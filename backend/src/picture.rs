//! The trainer's profile picture, shown on the account button and in "My profile". The browser
//! crops and shrinks it before upload; the server only checks the type by its first bytes and the size.

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Same cap as the `user_avatars.image` check.
const MAX_BYTES: usize = 512 * 1024;

/// The image type from its signature; the declared type is not trusted.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[derive(sqlx::FromRow)]
struct Picture {
    content_type: String,
    image: Vec<u8>,
}

async fn get_picture(State(state): State<AppState>, user: CurrentUser) -> AppResult<Response> {
    let row = sqlx::query_as::<_, Picture>("SELECT content_type, image FROM user_avatars WHERE user_id = $1")
        .bind(user.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound("avatar"))?;
    let mut response = Response::new(Body::from(row.image));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(&row.content_type).map_err(|e| anyhow::anyhow!(e))?);
    // The URL carries the version (`?v=`), so a changed picture is a new URL.
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=86400"));
    Ok(response)
}

async fn put_picture(State(state): State<AppState>, user: CurrentUser, mut form: Multipart) -> AppResult<Json<Value>> {
    let mut image: Option<Vec<u8>> = None;
    while let Some(field) = form.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
        if field.name() == Some("image") {
            image = Some(field.bytes().await.map_err(|e| AppError::BadRequest(e.to_string()))?.to_vec());
        }
    }
    let bytes = image.ok_or_else(|| AppError::BadRequest("the image field is missing".into()))?;
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(AppError::BadRequest("the picture must be 1 byte to 512 KB".into()));
    }
    let content_type = sniff(&bytes).ok_or_else(|| AppError::BadRequest("the picture must be PNG, JPEG or WebP".into()))?;
    let version: i64 = sqlx::query_scalar(
        "INSERT INTO user_avatars (user_id, content_type, image) VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE SET content_type = EXCLUDED.content_type, image = EXCLUDED.image, updated_at = now()
         RETURNING (extract(epoch FROM updated_at) * 1000)::bigint",
    )
    .bind(user.id)
    .bind(content_type)
    .bind(bytes)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({ "avatar_version": version })))
}

async fn delete_picture(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM user_avatars WHERE user_id = $1").bind(user.id).execute(&state.db).await?;
    Ok(Json(json!({ "avatar_version": null })))
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/avatar", get(get_picture).put(put_picture).delete(delete_picture).layer(DefaultBodyLimit::max(MAX_BYTES + 64 * 1024)))
}

#[cfg(test)]
mod tests {
    use super::sniff;

    #[test]
    fn sniff_reads_the_signature() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(sniff(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"<svg xmlns="), None);
        assert_eq!(sniff(b""), None);
    }
}
