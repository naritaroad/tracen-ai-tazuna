//! The bounded agent loop: model -> tools -> model, at most `conversation.max_steps` model calls.
//! Every tool call is checked, run on the server and recorded for the trace panel.

pub mod prompt;

use std::time::Duration;

use genai::chat::{ChatMessage, ChatRequest, ToolResponse};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::avatar::cues::{self, Context as CueContext};
use crate::avatar::{AvatarState, intent, reply_intent};
use crate::catalog::{academy_now, info_topics};
use crate::locale::Locale;
use crate::llm::Ai;
use crate::memory;
use crate::settings::{self, AiSettings, UsageRecord};
use crate::tools::{self, ToolContext};

pub const HISTORY_LIMIT: i64 = 20;
/// Deep thinking on a long reply can take a while; one call still may not hang the run.
const MODEL_TIMEOUT: Duration = Duration::from_secs(90);

/// At least two minutes, plus room for every extra step an admin allows.
fn run_timeout(max_steps: u32) -> Duration {
    Duration::from_secs((45 * u64::from(max_steps)).clamp(120, 600))
}

/// One event for the SSE stream (docs/api/stream-events.md).
#[derive(Debug, Clone)]
pub struct StreamEvent {
    pub kind: &'static str,
    pub data: Value,
}

pub type EventSink = UnboundedSender<StreamEvent>;

fn emit(sink: &EventSink, kind: &'static str, data: Value) {
    // The client may have gone; the run still finishes and is saved.
    let _ = sink.send(StreamEvent { kind, data });
}

pub struct RunInput<'a> {
    pub db: &'a PgPool,
    pub ai: &'a Ai,
    pub settings: &'a AiSettings,
    pub user: &'a CurrentUser,
    pub conversation_id: Uuid,
    pub locale: Locale,
}

#[derive(sqlx::FromRow)]
struct HistoryRow {
    role: String,
    content: String,
}

/// What the run produced; `message_id` is `None` when it failed.
pub struct RunResult {
    pub run_id: Uuid,
    pub message_id: Option<Uuid>,
}

async fn history(db: &PgPool, conversation_id: Uuid) -> sqlx::Result<Vec<ChatMessage>> {
    let mut rows = sqlx::query_as::<_, HistoryRow>(
        "SELECT role, content FROM messages WHERE conversation_id = $1 AND role IN ('user', 'assistant', 'director')
         ORDER BY created_at DESC LIMIT $2",
    )
    .bind(conversation_id)
    .bind(HISTORY_LIMIT)
    .fetch_all(db)
    .await?;
    rows.reverse();
    Ok(rows
        .into_iter()
        .map(|r| match r.role.as_str() {
            "user" => ChatMessage::user(r.content),
            "director" => ChatMessage::assistant(format!("[Reply from the Director's Office, written by a person]\n{}", r.content)),
            _ => ChatMessage::assistant(r.content),
        })
        .collect())
}

/// Runs the agent for the newest user message and returns the saved assistant message id.
pub async fn run(input: RunInput<'_>, sink: &EventSink) -> anyhow::Result<RunResult> {
    let model = input.settings.chat_model.as_str();
    let run_id: Uuid = sqlx::query_scalar("INSERT INTO agent_runs (conversation_id, model, locale) VALUES ($1, $2, $3) RETURNING id")
        .bind(input.conversation_id)
        .bind(model)
        .bind(input.locale.code())
        .fetch_one(input.db)
        .await?;
    emit(sink, "run.started", json!({ "run_id": run_id, "model": model, "locale": input.locale.code() }));
    emit(sink, "avatar.intent", intent(AvatarState::Thinking, "run.started"));

    let outcome = if settings::check_budget(input.db, input.settings).await.is_err() {
        Ok(Err(Failure { steps: 0, code: "budget" }))
    } else {
        tokio::time::timeout(run_timeout(input.settings.conversation.max_steps), run_loop(&input, run_id, sink)).await
    };
    let (status, steps, error_code, message_id, usage) = match outcome {
        Ok(Ok(done)) => ("completed", done.steps, None, Some(done.message_id), done.usage),
        Ok(Err(failure)) => ("failed", failure.steps, Some(failure.code), None, json!({})),
        Err(_) => ("failed", input.settings.conversation.max_steps as i32, Some("timeout"), None, json!({})),
    };

    sqlx::query("UPDATE agent_runs SET status = $2, steps = $3, error_code = $4, usage = $5, finished_at = now() WHERE id = $1")
        .bind(run_id)
        .bind(status)
        .bind(steps)
        .bind(error_code)
        .bind(&usage)
        .execute(input.db)
        .await?;

    match error_code {
        None => emit(sink, "run.completed", json!({ "run_id": run_id, "steps": steps, "usage": usage })),
        Some(code) => {
            // A notice keeps the conversation readable after a failure.
            sqlx::query("INSERT INTO messages (conversation_id, role, content, run_id) VALUES ($1, 'notice', $2, $3)")
                .bind(input.conversation_id)
                .bind(format!("run.failed.{code}"))
                .bind(run_id)
                .execute(input.db)
                .await?;
            emit(sink, "avatar.intent", intent(AvatarState::Error, "run.failed"));
            emit(sink, "run.failed", json!({ "run_id": run_id, "code": code, "message": failure_text(code, input.locale) }));
        }
    }
    Ok(RunResult { run_id, message_id })
}

fn failure_text(code: &str, locale: Locale) -> &'static str {
    match code {
        "max_steps" => locale.pick(
            "這個問題處理步驟太多，我先停下來了。請把問題拆小一點，或交由理事長室處理。",
            "手順が多くなりすぎたため、いったん止めました。質問を分けるか、理事長室へご相談ください。",
            "This took too many steps, so I stopped. Please ask a smaller question, or contact the Director's Office.",
        ),
        "budget" => locale.pick(
            "本月的 AI 使用額度已用完，請改用手動頁面，或聯絡理事長室。",
            "今月の AI 利用枠を使い切りました。手動ページをご利用いただくか、理事長室へご連絡ください。",
            "This month's AI budget is used up. Please use the manual pages, or contact the Director's Office.",
        ),
        "timeout" => locale.pick("回覆逾時，請稍後再試。", "応答がタイムアウトしました。しばらくしてからもう一度お試しください。", "The reply timed out. Please try again shortly."),
        _ => locale.pick("系統暫時無法回覆，請稍後再試，或使用手動頁面。", "ただいま応答できません。しばらくしてから再度お試しいただくか、手動ページをご利用ください。", "I cannot reply right now. Please try again later, or use the manual pages."),
    }
}

/// Gemini now and then writes a line break as the two characters `\n`; the trainer should see a line break.
fn tidy(text: &str) -> String {
    text.replace(r"\r\n", "\n").replace(r"\n", "\n")
}

struct Done {
    steps: i32,
    message_id: Uuid,
    usage: Value,
}

struct Failure {
    steps: i32,
    code: &'static str,
}

fn db_fail(steps: i32) -> impl Fn(sqlx::Error) -> Failure {
    move |e| {
        tracing::error!(error = %e, steps, "agent database error");
        Failure { steps, code: "internal" }
    }
}

async fn run_loop(input: &RunInput<'_>, run_id: Uuid, sink: &EventSink) -> Result<Done, Failure> {
    let topics = info_topics(input.db).await.map_err(db_fail(0))?;
    let messages = history(input.db, input.conversation_id).await.map_err(db_fail(0))?;
    // Long-term memory: profile, recalled past events and the summary of older messages.
    let question = messages.last().and_then(|m| m.content.first_text()).unwrap_or_default().to_string();
    let profile = memory::prompt_context(input.db, input.ai, input.settings, input.user, input.conversation_id, &question)
        .await
        .map_err(db_fail(0))?;
    let conversation = &input.settings.conversation;
    let gateway = input.ai.chat_gateway(&input.settings.chat_model, conversation);
    let mut request = ChatRequest::new(messages)
        .with_system(prompt::system_prompt(input.locale, &profile, academy_now(), conversation))
        .with_tools(tools::specs(&topics));

    let ctx = ToolContext { db: input.db, user: input.user, conversation_id: input.conversation_id, run_id, locale: input.locale };
    let mut usage_total: Vec<Value> = Vec::new();
    let mut any_not_found = false;
    let mut handed_off = false;

    let max_steps = conversation.max_steps as i32;
    for step in 1..=max_steps {
        let reply = match tokio::time::timeout(MODEL_TIMEOUT, gateway.chat(request.clone())).await {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                tracing::warn!(error = ?e, step, "model call failed");
                return Err(Failure { steps: step, code: "model_error" });
            }
            Err(_) => return Err(Failure { steps: step, code: "timeout" }),
        };
        usage_total.push(reply.usage.clone());
        let (prompt_tokens, completion_tokens) = settings::tokens(&reply.usage);
        settings::record_usage(
            input.db,
            input.settings,
            UsageRecord { purpose: "chat", model: gateway.model_name(), prompt_tokens, completion_tokens, user_id: Some(input.user.id), run_id: Some(run_id) },
        )
        .await;

        if reply.tool_calls.is_empty() {
            // The mood line is for the character only; the trainer and the history never see it.
            let (text, mood) = cues::split_mood(&tidy(&reply.text.unwrap_or_default()));
            if text.is_empty() {
                return Err(Failure { steps: step, code: "model_error" });
            }
            let state = if handed_off { AvatarState::Handoff } else if any_not_found { AvatarState::Apologizing } else { AvatarState::Explaining };
            let cue = cues::infer(&CueContext { question: &question, reply: &text, handed_off, not_found: any_not_found }, mood);
            emit(sink, "avatar.intent", reply_intent(state, "message.completed", &cue));
            emit(sink, "message.delta", json!({ "text": text }));
            let message_id: Uuid = sqlx::query_scalar(
                "INSERT INTO messages (conversation_id, role, content, run_id) VALUES ($1, 'assistant', $2, $3) RETURNING id",
            )
            .bind(input.conversation_id)
            .bind(&text)
            .bind(run_id)
            .fetch_one(input.db)
            .await
            .map_err(db_fail(step))?;
            sqlx::query("UPDATE conversations SET updated_at = now() WHERE id = $1")
                .bind(input.conversation_id)
                .execute(input.db)
                .await
                .map_err(db_fail(step))?;
            emit(sink, "message.completed", json!({ "message_id": message_id, "content": text, "run_id": run_id }));
            return Ok(Done { steps: step, message_id, usage: json!(usage_total) });
        }

        emit(sink, "avatar.intent", intent(AvatarState::Working, "tool.started"));
        let assistant = reply.assistant_message.unwrap_or_else(|| ChatMessage::from(reply.tool_calls.clone()));
        request = request.append_message(assistant);

        for call in &reply.tool_calls {
            emit(sink, "tool.started", json!({ "call_id": call.call_id, "step": step, "name": call.fn_name, "arguments": call.fn_arguments }));
            let outcome = tools::dispatch(&ctx, &call.fn_name, &call.fn_arguments).await;
            if outcome.result.get("found").and_then(Value::as_bool) == Some(false) || outcome.result.get("complete").and_then(Value::as_bool) == Some(false) {
                any_not_found = true;
            }
            sqlx::query(
                "INSERT INTO tool_calls (run_id, step, call_id, tool_name, arguments, result, status, duration_ms)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(run_id)
            .bind(step)
            .bind(&call.call_id)
            .bind(&call.fn_name)
            .bind(&call.fn_arguments)
            .bind(&outcome.result)
            .bind(outcome.status.as_str())
            .bind(outcome.duration_ms)
            .execute(input.db)
            .await
            .map_err(db_fail(step))?;
            emit(
                sink,
                "tool.completed",
                json!({ "call_id": call.call_id, "step": step, "name": call.fn_name, "status": outcome.status.as_str(), "result": outcome.result, "duration_ms": outcome.duration_ms }),
            );
            if let Some(ticket) = &outcome.ticket {
                handed_off = true;
                emit(sink, "avatar.intent", intent(AvatarState::Handoff, "handoff.updated"));
                emit(sink, "handoff.updated", json!({ "ticket_no": ticket.ticket_no, "status": ticket.status, "conversation_status": "active" }));
            }
            request = request.append_message(ToolResponse::from_tool_call(call, outcome.result.to_string()));
        }
    }
    Err(Failure { steps: max_steps, code: "max_steps" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_line_breaks_become_real_ones() {
        assert_eq!(tidy("辛苦了♪\\n\n今天也加油"), "辛苦了♪\n\n今天也加油");
        assert_eq!(tidy(r"a\r\nb"), "a\nb");
    }

    #[test]
    fn run_time_grows_with_steps() {
        assert_eq!(run_timeout(1), Duration::from_secs(120));
        assert_eq!(run_timeout(6), Duration::from_secs(270));
        assert_eq!(run_timeout(12), Duration::from_secs(540));
    }
}
