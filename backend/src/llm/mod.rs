//! The only place that talks to model providers. genai (rust-genai) handles Gemini, Anthropic
//! and the others; the model name chooses the provider. `mock` is an offline stand-in for
//! development and tests, never a substitute for a real model.

pub mod mock;

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use futures::future::BoxFuture;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, ContentPart, MessageContent, ReasoningEffort, ToolCall};
use genai::embed::EmbedOptions;
use serde_json::{Value, json};

use crate::settings::{ConversationSettings, Reasoning};

/// Width of the `vector(768)` columns.
pub const EMBED_DIMENSIONS: usize = 768;

/// One model call, reduced to what the agent loop needs.
pub struct ModelReply {
    pub text: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    /// The assistant turn to append before the tool responses. It keeps provider metadata
    /// such as Gemini thought signatures, so tool use continues correctly.
    pub assistant_message: Option<ChatMessage>,
    pub usage: Value,
}

pub trait ModelGateway: Send + Sync {
    fn model_name(&self) -> &str;
    fn chat(&self, request: ChatRequest) -> BoxFuture<'_, Result<ModelReply>>;
}

pub struct GenaiGateway {
    client: genai::Client,
    model: String,
    options: ChatOptions,
}

impl ModelGateway for GenaiGateway {
    fn model_name(&self) -> &str {
        &self.model
    }

    fn chat(&self, request: ChatRequest) -> BoxFuture<'_, Result<ModelReply>> {
        Box::pin(async move {
            let response = self.client.exec_chat(self.model.as_str(), request, Some(&self.options)).await?;
            let usage = serde_json::to_value(&response.usage).unwrap_or_default();
            let text = response.first_text().map(str::to_string);
            let tool_calls: Vec<ToolCall> = response.tool_calls().into_iter().cloned().collect();
            let assistant_message = response.into_assistant_message_for_tool_use();
            Ok(ModelReply { text, tool_calls, assistant_message, usage })
        })
    }
}

pub struct Embedding {
    pub vector: Vec<f32>,
    pub prompt_tokens: i64,
}

pub struct Transcript {
    pub text: String,
    pub usage: Value,
}

/// Model access for the whole app. The model names come from the admin settings on each call,
/// so a change on the management site applies without a restart.
#[derive(Clone)]
pub struct Ai {
    /// API keys come from the provider's usual variable, e.g. GEMINI_API_KEY or ANTHROPIC_API_KEY.
    client: genai::Client,
}

impl Ai {
    pub fn new() -> Result<Self> {
        Ok(Self { client: genai::Client::new()? })
    }

    pub fn gateway(&self, model: &str) -> Box<dyn ModelGateway> {
        self.gateway_with(model, ChatOptions::default().with_temperature(0.2).with_max_tokens(1200))
    }

    /// The agent's gateway: thinking depth, temperature and output limit come from the admin settings.
    pub fn chat_gateway(&self, model: &str, conversation: &ConversationSettings) -> Box<dyn ModelGateway> {
        self.gateway_with(model, chat_options(conversation))
    }

    fn gateway_with(&self, model: &str, options: ChatOptions) -> Box<dyn ModelGateway> {
        if model == "mock" {
            Box::new(mock::MockGateway)
        } else {
            Box::new(GenaiGateway { client: self.client.clone(), model: model.to_string(), options })
        }
    }

    /// One plain completion with no tools (memory curation, summaries).
    pub async fn complete(&self, model: &str, system: &str, user: &str) -> Result<(String, Value)> {
        let request = ChatRequest::new(vec![ChatMessage::user(user.to_string())]).with_system(system.to_string());
        let reply = self.gateway(model).chat(request).await?;
        Ok((reply.text.unwrap_or_default(), reply.usage))
    }

    /// A 768-dimension embedding of `text`.
    pub async fn embed(&self, model: &str, text: &str) -> Result<Embedding> {
        if model == "mock" {
            return Ok(Embedding { vector: mock::embed(text), prompt_tokens: 0 });
        }
        let options = EmbedOptions::new().with_dimensions(EMBED_DIMENSIONS);
        let response = self.client.embed(model, text.to_string(), Some(&options)).await?;
        let vector = response.first_vector().cloned().context("no embedding in the response")?;
        if vector.len() != EMBED_DIMENSIONS {
            bail!("{model} returned {} dimensions, expected {EMBED_DIMENSIONS}", vector.len());
        }
        let reported = i64::from(response.usage.prompt_tokens.unwrap_or(0));
        // Some providers do not report embedding tokens; estimate so the budget still moves.
        let prompt_tokens = if reported > 0 { reported } else { (text.chars().count() as i64 / 3).max(1) };
        Ok(Embedding { vector, prompt_tokens })
    }

    /// Speech to text with a model that accepts audio input.
    pub async fn transcribe(&self, model: &str, audio: &[u8], mime: &str, language_hint: &str) -> Result<Transcript> {
        if model == "mock" {
            return Ok(Transcript { text: mock::TRANSCRIPT.to_string(), usage: json!({}) });
        }
        let data = base64::engine::general_purpose::STANDARD.encode(audio);
        let content = MessageContent::from_parts(vec![
            ContentPart::from_binary_base64(mime.to_string(), data, None),
            ContentPart::from_text(format!(
                "Transcribe this recording word for word. The speaker is most likely using {language_hint}. \
                 Write Chinese in Traditional characters as used in Taiwan. Return only the transcript, with no notes. \
                 If there is no speech, return an empty line."
            )),
        ]);
        let request = ChatRequest::new(vec![ChatMessage::user(content)]);
        let reply = self.gateway(model).chat(request).await?;
        Ok(Transcript { text: reply.text.unwrap_or_default().trim().to_string(), usage: reply.usage })
    }
}

/// Unset values are left to the provider: `Auto` thinking and no temperature send nothing.
fn chat_options(c: &ConversationSettings) -> ChatOptions {
    let mut options = ChatOptions::default().with_max_tokens(c.max_output_tokens);
    if let Some(t) = c.temperature {
        options = options.with_temperature(t);
    }
    let effort = match c.reasoning {
        Reasoning::Auto => None,
        Reasoning::Low => Some(ReasoningEffort::Low),
        Reasoning::Medium => Some(ReasoningEffort::Medium),
        Reasoning::High => Some(ReasoningEffort::High),
    };
    if let Some(effort) = effort {
        options = options.with_reasoning_effort(effort);
    }
    options
}

/// `[0.1,0.2,...]`, the text form pgvector accepts with `::vector`.
pub fn vector_literal(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 10 + 2);
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{x:.6}"));
    }
    s.push(']');
    s
}
