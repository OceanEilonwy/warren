//! The planner's API in the browser. JavaScript (in a Web Worker) calls `initThreadPool(n)` once, then
//! `call(method, body)` exactly as it would `POST /api/<method>`; rayon spreads each solve over `n`
//! further workers. Needs a cross-origin isolated page (COOP/COEP) for shared memory.

use std::sync::OnceLock;

use rabbitsim::api::{Api, presets::Source};
use wasm_bindgen::prelude::*;

pub use wasm_bindgen_rayon::init_thread_pool;

fn api() -> &'static Api {
    static API: OnceLock<Api> = OnceLock::new();
    API.get_or_init(|| Api::new(Source::Embedded))
}

/// Run one API method. Errors come back as `{"status": n, "message": "..."}` in the exception.
#[wasm_bindgen]
pub fn call(method: &str, body: &str) -> Result<String, JsValue> {
    api().call(method, body).map_err(|e| JsValue::from_str(&serde_json::json!({ "status": e.status, "message": e.message }).to_string()))
}
