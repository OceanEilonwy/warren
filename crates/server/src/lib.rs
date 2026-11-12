//! HTTP front end: `POST /api/<method>` runs [`rabbitsim::api::Api::call`] (CPU work on blocking threads,
//! rayon inside), and everything else is the built web UI (`web/dist`).

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use rabbitsim::api::{Api, presets::Source};
use tower_http::services::{ServeDir, ServeFile};

/// Largest request body accepted (preset files are limited to 64 KB; requests are far smaller).
const MAX_BODY: usize = 256 * 1024;

/// `$PRESETS_DIR`, else the `presets/` folder at the repository root.
pub fn default_presets_dir() -> PathBuf {
    std::env::var_os("PRESETS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../presets")))
}

/// `$WEB_DIST`, else `web/dist` at the repository root (built with `npm run build` in `web/`).
pub fn default_web_dir() -> PathBuf {
    std::env::var_os("WEB_DIST").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/dist")))
}

pub fn router() -> Router {
    router_with(Api::new(Source::Dir(default_presets_dir())), default_web_dir())
}

pub fn router_with(api: Api, web_dir: PathBuf) -> Router {
    let index = web_dir.join("index.html");
    Router::new()
        .route("/api/{method}", post(call))
        .fallback_service(ServeDir::new(&web_dir).fallback(ServeFile::new(index)))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY))
        .with_state(Arc::new(api))
}

async fn call(State(api): State<Arc<Api>>, Path(method): Path<String>, body: Bytes) -> Response {
    let Ok(body) = String::from_utf8(body.to_vec()) else {
        return error(400, "request body is not UTF-8");
    };
    let res = tokio::task::spawn_blocking(move || api.call(&method, &body)).await;
    match res {
        Ok(Ok(json)) => ([(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, "no-store")], json).into_response(),
        Ok(Err(e)) => error(e.status, &e.message),
        Err(_) => error(500, "the solver failed"),
    }
}

fn error(status: u16, message: &str) -> Response {
    let code = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (code, [(header::CONTENT_TYPE, "application/json")], serde_json::json!({ "status": status, "message": message }).to_string())
        .into_response()
}
