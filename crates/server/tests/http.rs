//! HTTP wiring: `POST /api/<method>`, errors as JSON with the right status, and the web UI as files.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use rabbitsim::api::{Api, presets::Source};
use rabbitsim::engine::OptConfig;
use tower::ServiceExt;

fn app(presets: std::path::PathBuf, web: std::path::PathBuf) -> axum::Router {
    let api = Api::new(Source::Dir(presets)).with_config(OptConfig { eval_runs: 8, eval_years: 3, rounds: 1 });
    rabbitsim_server::router_with(api, web)
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("warren-http-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

async fn send(app: &axum::Router, method: &str, uri: &str, body: &str) -> (StatusCode, String, String) {
    let req = Request::builder().method(method).uri(uri).body(Body::from(body.to_string())).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let ct = res.headers().get(header::CONTENT_TYPE).map(|v| v.to_str().unwrap().to_string()).unwrap_or_default();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, ct, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn api_methods_answer_json() {
    let app = app(rabbitsim_server::default_presets_dir(), tmp("web-a"));
    let (s, ct, body) = send(&app, "POST", "/api/meta", "").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(ct, "application/json");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["space"]["title"], "Space");
    let (s, _, body) = send(&app, "POST", "/api/solve", r#"{"request":{"params":{"runs":10,"years":2}}}"#).await;
    assert_eq!(s, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(v["metrics"]["meat_kg_per_yr"]["mean"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn errors_carry_status_and_message() {
    let app = app(rabbitsim_server::default_presets_dir(), tmp("web-b"));
    let (s, ct, body) = send(&app, "POST", "/api/nope", "{}").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(ct, "application/json");
    assert!(body.contains("no API method"));
    let (s, _, body) = send(&app, "POST", "/api/solve", "{not json").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(body.contains("bad request body"));
    let (s, _, _) = send(&app, "POST", "/api/preset_load", r#"{"id":"../../etc/passwd"}"#).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _, _) = send(&app, "GET", "/api/meta", "").await;
    assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED);
    let big = format!(r#"{{"text":"{}"}}"#, "x".repeat(300 * 1024));
    let (s, _, _) = send(&app, "POST", "/api/preset_parse", &big).await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn presets_are_read_from_disk_on_every_request() {
    let dir = tmp("presets");
    let app = app(dir.clone(), tmp("web-c"));
    let (_, _, body) = send(&app, "POST", "/api/presets", "").await;
    assert_eq!(body, "[]");
    std::fs::write(dir.join("tiny.txt"), "name = Tiny\ndescription = A test\nwidth_m = 1\n").unwrap();
    let (_, _, body) = send(&app, "POST", "/api/presets", "").await;
    assert!(body.contains(r#""id":"tiny""#) && body.contains("Tiny"));
    let (s, _, body) = send(&app, "POST", "/api/preset_load", r#"{"id":"tiny"}"#).await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains(r#""width_m":1.0"#), "{body}");
}

#[tokio::test]
async fn the_web_ui_is_served_with_an_index_fallback() {
    let web = tmp("web-d");
    std::fs::write(web.join("index.html"), "<!doctype html><title>warren</title>").unwrap();
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(web.join("assets/app.js"), "console.log(1)").unwrap();
    let app = app(rabbitsim_server::default_presets_dir(), web);
    let (s, ct, body) = send(&app, "GET", "/", "").await;
    assert_eq!(s, StatusCode::OK);
    assert!(ct.starts_with("text/html") && body.contains("warren"));
    let (s, ct, _) = send(&app, "GET", "/assets/app.js", "").await;
    assert_eq!(s, StatusCode::OK);
    assert!(ct.contains("javascript"), "{ct}");
    let (s, _, body) = send(&app, "GET", "/some/client/route", "").await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains("warren"), "unknown paths fall back to the app");
}
