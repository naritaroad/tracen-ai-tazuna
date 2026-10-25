//! Speech (docs/character/voice-contract.md). Microphone input is turned into text here; the trainer checks
//! the text before sending it. Spoken replies are reserved: the endpoints exist but do nothing yet.

use std::time::Instant;

use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::locale::Locale;
use crate::settings::{self, UsageRecord};
use crate::state::AppState;

const MAX_AUDIO_BYTES: usize = 10 * 1024 * 1024;
const AUDIO_TYPES: [&str; 8] = ["audio/webm", "audio/ogg", "audio/mp4", "audio/m4a", "audio/mpeg", "audio/mp3", "audio/wav", "audio/aac"];

/// `audio/webm;codecs=opus` -> `audio/webm`; `audio/x-wav` -> `audio/wav`.
fn base_mime(content_type: &str) -> String {
    let base = content_type.split(';').next().unwrap_or_default().trim().to_ascii_lowercase();
    match base.as_str() {
        "audio/x-wav" | "audio/wave" => "audio/wav".into(),
        "audio/x-m4a" => "audio/m4a".into(),
        "video/webm" => "audio/webm".into(),
        _ => base,
    }
}

fn language_name(locale: Locale) -> &'static str {
    locale.pick("Traditional Chinese (Taiwan)", "Japanese", "English")
}

async fn transcribe(State(state): State<AppState>, user: CurrentUser, mut form: Multipart) -> AppResult<Json<Value>> {
    let mut audio: Option<(Vec<u8>, String)> = None;
    let mut locale = Locale::ZhTw;
    while let Some(field) = form.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
        match field.name() {
            Some("audio") => {
                let mime = base_mime(field.content_type().unwrap_or_default());
                let bytes = field.bytes().await.map_err(|e| AppError::BadRequest(e.to_string()))?;
                audio = Some((bytes.to_vec(), mime));
            }
            Some("locale") => {
                let text = field.text().await.unwrap_or_default();
                locale = Locale::parse(&text).unwrap_or(locale);
            }
            _ => {}
        }
    }
    let (bytes, mime) = audio.ok_or_else(|| AppError::BadRequest("the audio field is missing".into()))?;
    if bytes.is_empty() || bytes.len() > MAX_AUDIO_BYTES {
        return Err(AppError::BadRequest("audio must be 1 byte to 10 MB".into()));
    }
    if !AUDIO_TYPES.contains(&mime.as_str()) {
        return Err(AppError::BadRequest(format!("unsupported audio type {mime}")));
    }

    let settings = settings::load(&state.db, &state.config).await?;
    if settings::check_budget(&state.db, &settings).await.is_err() {
        return Err(AppError::Conflict("the monthly AI budget is used up".into()));
    }
    let started = Instant::now();
    // The recording is sent to the engine and dropped; only the returned text is kept by the client.
    let transcript = state.ai.transcribe(&settings.transcribe_model, &bytes, &mime, language_name(locale)).await.map_err(|e| {
        tracing::warn!(error = ?e, "transcription failed");
        AppError::Internal(anyhow::anyhow!("transcription failed"))
    })?;
    let (p, c) = settings::tokens(&transcript.usage);
    settings::record_usage(
        &state.db,
        &settings,
        UsageRecord { purpose: "transcribe", model: &settings.transcribe_model, prompt_tokens: p, completion_tokens: c, user_id: Some(user.id), run_id: None },
    )
    .await;
    Ok(Json(json!({
        "text": transcript.text,
        "locale": locale.code(),
        "engine": settings.transcribe_model,
        "duration_ms": started.elapsed().as_millis() as u64,
    })))
}

/// Reserved: voices for spoken replies.
async fn voices(_user: CurrentUser) -> Json<Value> {
    Json(json!({ "voices": [] }))
}

/// Reserved: speak a saved assistant message.
async fn synthesize(_user: CurrentUser) -> AppError {
    AppError::NotImplemented
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/transcribe", post(transcribe).layer(DefaultBodyLimit::max(MAX_AUDIO_BYTES + 64 * 1024)))
        .route("/voices", get(voices))
        .route("/synthesize", post(synthesize))
}

#[cfg(test)]
mod tests {
    use super::base_mime;

    #[test]
    fn normalises_recorder_mime_types() {
        assert_eq!(base_mime("audio/webm;codecs=opus"), "audio/webm");
        assert_eq!(base_mime("audio/x-wav"), "audio/wav");
        assert_eq!(base_mime("Audio/MP4"), "audio/mp4");
    }
}
