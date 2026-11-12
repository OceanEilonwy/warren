//! axum application: htmx-driven, stateless (all state lives in the form), CPU work on blocking threads.

pub mod charts;
pub mod controls;
pub mod presets;
pub mod views;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use maud::{Markup, html};

use crate::engine::{MetricKey, Objective, OptConfig, Out, Problem, Solution, solve};
use controls::{Request, parse_request};
use views::View;

/// Solved problems, so cost chips and slider drags don't repeat work the server has already done.
pub struct AppState {
    cache: Mutex<HashMap<String, Arc<Solution>>>,
    /// Where preset files are read from (on every request, so adding a file adds a preset).
    pub presets_dir: PathBuf,
}

impl Default for AppState {
    fn default() -> Self {
        AppState::new(default_presets_dir())
    }
}

/// `$PRESETS_DIR`, else the `presets/` folder next to the source tree.
pub fn default_presets_dir() -> PathBuf {
    std::env::var_os("PRESETS_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/presets")))
}

impl AppState {
    const CACHE_LIMIT: usize = 64;

    pub fn new(presets_dir: PathBuf) -> Self {
        AppState { cache: Mutex::new(HashMap::new()), presets_dir }
    }

    fn presets(&self) -> Vec<presets::PresetInfo> {
        presets::list(&self.presets_dir)
    }

    pub fn solve_cached(&self, problem: &Problem) -> Arc<Solution> {
        let key = serde_json::to_string(problem).unwrap_or_default();
        if let Some(s) = self.cache.lock().unwrap().get(&key) {
            return s.clone();
        }
        let s = Arc::new(solve(problem, OptConfig::default()));
        let mut c = self.cache.lock().unwrap();
        if c.len() >= Self::CACHE_LIMIT {
            c.clear();
        }
        c.insert(key, s.clone());
        s
    }
}

pub fn router() -> Router {
    router_with(default_presets_dir())
}

pub fn router_with(presets_dir: PathBuf) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/preset/export", get(export_preset))
        .route("/preset/load", post(load_preset))
        .route("/results", get(results))
        .route("/app", get(app))
        .route("/cost", get(cost))
        .route("/favicon.ico", get(|| async { StatusCode::NO_CONTENT }))
        .route("/htmx.min.js", get(|| async { asset("text/javascript", include_str!("../../assets/htmx.min.js")) }))
        .route("/style.css", get(|| async { asset("text/css", include_str!("../../assets/style.css")) }))
        .with_state(Arc::new(AppState::new(presets_dir)))
}

fn asset(content_type: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "public, max-age=3600")], body).into_response()
}

/// Run CPU-heavy work off the async runtime (rayon uses every core inside).
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.expect("solver thread panicked")
}

async fn solved(st: Arc<AppState>, req: &Request) -> Arc<Solution> {
    let problem = req.problem();
    blocking(move || st.solve_cached(&problem)).await
}

/// The shell only: no calculation, so it appears at once and the browser then asks for the plan.
async fn index(State(st): State<Arc<AppState>>) -> Markup {
    views::page(&Request::default(), &st.presets())
}

async fn results(State(st): State<Arc<AppState>>, Query(q): Query<HashMap<String, String>>) -> Markup {
    let req = parse_request(&q);
    let sol = solved(st, &req).await;
    views::results_fragment(&View { req: &req, sol: &sol, presets: &[], note: None })
}

/// Whole-app swaps: add / remove a constraint, change objective, restore defaults.
async fn app(State(st): State<Arc<AppState>>, Query(q): Query<HashMap<String, String>>) -> Markup {
    let mut req = parse_request(&q);
    let mut note: Option<String> = None;
    match q.get("action").map(String::as_str) {
        Some("preset") => {
            if let Some(id) = q.get("preset").filter(|p| !p.is_empty()) {
                match presets::load(&st.presets_dir, id) {
                    Ok(p) => {
                        note = Some(preset_note(p.name.as_deref().unwrap_or(id), &p.warnings));
                        req = p.request;
                    }
                    Err(e) => note = Some(e),
                }
            }
        }
        Some("add") => {
            if let Some(key) = q.get("add").filter(|k| !k.is_empty()) {
                let current = solved(st.clone(), &req).await;
                req.add_from(key, &current);
            }
        }
        Some("remove") => {
            if let Some(key) = q.get("key") {
                req.remove(key);
            }
        }
        Some("defaults") => req = Request::default(),
        // "Maximise floor space" needs something to trade against: suggest meat and herd-size minimums.
        Some("objective") if req.objective == Objective::MaxQol && !(req.has(Out::MeatKgPerYr.key()) && req.has(Out::PeakHead.key())) => {
            let mut best = req.clone();
            best.objective = Objective::MaxMeat;
            let s = solved(st.clone(), &best).await;
            req.suggest_floors(s.summary.metric(MetricKey::MeatKgPerYr).mean, s.summary.metric(MetricKey::PeakHead).mean);
        }
        _ => {}
    }
    let presets = st.presets();
    let sol = solved(st, &req).await;
    views::app_swap(&View { req: &req, sol: &sol, presets: &presets, note: note.as_deref() })
}

/// "Loaded X" plus anything in the file that was ignored.
fn preset_note(name: &str, warnings: &[String]) -> String {
    let mut s = format!("Loaded “{name}”.");
    for w in warnings.iter().take(6) {
        s.push_str(&format!("\nIgnored: {w}"));
    }
    if warnings.len() > 6 {
        s.push_str(&format!("\n…and {} more.", warnings.len() - 6));
    }
    s
}

/// Download the current setup (the form's values, as the query string) as a preset file.
async fn export_preset(Query(q): Query<HashMap<String, String>>) -> Response {
    let req = parse_request(&q);
    let body = presets::to_text(&req, "My plan");
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"rabbit-plan.txt\""),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}

/// Replace the whole setup with an uploaded preset file's contents.
async fn load_preset(State(st): State<Arc<AppState>>, Form(f): Form<HashMap<String, String>>) -> Markup {
    let text = f.get("text").map(String::as_str).unwrap_or("");
    let p = presets::parse(text);
    let note = if text.trim().is_empty() {
        "That file was empty, so nothing changed.".to_string()
    } else {
        preset_note(p.name.as_deref().unwrap_or("imported preset"), &p.warnings)
    };
    let req = if text.trim().is_empty() { Request::default() } else { p.request };
    let presets = st.presets();
    let sol = solved(st, &req).await;
    views::app_swap(&View { req: &req, sol: &sol, presets: &presets, note: Some(&note) })
}

/// What one constraint costs: the objective with and without it.
async fn cost(State(st): State<Arc<AppState>>, Query(q): Query<HashMap<String, String>>) -> Markup {
    let req = parse_request(&q);
    let Some(key) = q.get("drop").cloned() else { return html! {} };
    let mut without = req.clone();
    without.remove(&key);
    let objective = req.objective;
    let (full, other) = blocking(move || (st.solve_cached(&req.problem()), st.solve_cached(&without.problem()))).await;
    html! { (views::cost_text(objective, &full, &other)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_returns_the_same_solution_for_the_same_problem() {
        let st = AppState::default();
        let mut p = Request::default().problem();
        p.base.runs = 20;
        let a = st.solve_cached(&p);
        let b = st.solve_cached(&p);
        assert!(Arc::ptr_eq(&a, &b));
    }
}
