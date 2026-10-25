//! The official academy crest. It is a separate file (Cygames emblem, see NOTICE.md),
//! so the route answers 404 when the file is missing and the site keeps its own horseshoe mark.

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum::routing::get;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Only a file that opens as an SVG document is served.
fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    head.trim_start_matches('\u{feff}').trim_start().starts_with('<') && head.contains("<svg")
}

async fn crest(State(state): State<AppState>) -> AppResult<Response> {
    let path = state.config.brand_crest_file.as_ref().ok_or(AppError::NotFound("crest"))?;
    let bytes = tokio::fs::read(path).await.map_err(|_| AppError::NotFound("crest"))?;
    if !looks_like_svg(&bytes) {
        return Err(AppError::NotFound("crest"));
    }
    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"));
    // Opened directly, the SVG still may not run scripts or load anything.
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    Ok(response)
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/brand/crest.svg", get(crest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_svg_documents_pass() {
        assert!(looks_like_svg(b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"));
        assert!(looks_like_svg(b"\xef\xbb\xbf<?xml version=\"1.0\"?>\n<svg></svg>"));
        assert!(!looks_like_svg(b"\x89PNG\r\n\x1a\n"));
        assert!(!looks_like_svg(b"<html><body>svg</body></html>"));
    }
}
