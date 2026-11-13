//! The planner's API, independent of transport. Every call is `method` + JSON body -> JSON reply, so the
//! HTTP server (`POST /api/<method>`) and the WASM build (`call(method, body)`) expose exactly the same
//! thing and share all the logic here.
//!
//! | method           | body                                   | reply                                  |
//! |------------------|----------------------------------------|----------------------------------------|
//! | `meta`           | `{}`                                   | [`Meta`]: inputs, constraints, defaults |
//! | `presets`        | `{}`                                   | `[PresetInfo]`                         |
//! | `preset_load`    | `{"id"}`                               | [`Loaded`]                             |
//! | `preset_parse`   | `{"text"}`                             | [`Loaded`]                             |
//! | `preset_export`  | `{"request", "name"?}`                 | `{"text"}`                             |
//! | `solve`          | `{"request"}`                          | [`SolveView`]                          |
//! | `cost`           | `{"request", "key"}`                   | `{"text"}`                             |
//! | `add_constraint` | `{"request", "key"}`                   | `{"request"}`                          |
//! | `set_objective`  | `{"request", "objective"}`             | `{"request"}`                          |
//!
//! Every `request` is a [`WireRequest`]; it is checked and clamped before use, and replies carry the
//! normalised version back.

pub mod meta;
pub mod presets;
pub mod report;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engine::{
    CageSnap, Layout, MetricKey, Objective, OptConfig, Out, Params, Problem, Solution, Var, sim::counted_from_day, solve as run_solver,
    summary::Series, summary::Stat,
};
pub use meta::{Meta, Request, WireConstraint, WireRequest};
use presets::{PresetInfo, Source};
use report::Check;

/// A failed call: `status` follows HTTP (400 bad input, 404 unknown method or preset).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}

impl ApiError {
    fn bad(m: impl Into<String>) -> Self {
        ApiError { status: 400, message: m.into() }
    }
    fn not_found(m: impl Into<String>) -> Self {
        ApiError { status: 404, message: m.into() }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.status)
    }
}

/// A preset that was loaded or parsed: the request it describes and anything that was ignored.
#[derive(Clone, Debug, Serialize)]
pub struct Loaded {
    pub request: WireRequest,
    pub name: Option<String>,
    pub description: Option<String>,
    pub warnings: Vec<String>,
    /// "Loaded “X”." plus up to six ignored lines, ready to show.
    pub note: String,
}

/// A solved plan: everything the results page shows.
#[derive(Clone, Debug, Serialize)]
pub struct SolveView {
    /// The request as understood (checked and clamped).
    pub request: WireRequest,
    /// Every input of the chosen plan, including the ones the solver picked.
    pub params: Params,
    /// Meat kg/yr or the mean floor-space multiplier, depending on the objective.
    pub objective_value: f64,
    pub feasible: bool,
    /// Share of rabbits butchered early for lack of room is within the accepted limit.
    pub fits: bool,
    pub forced_share: f64,
    /// Constraints the plan misses, as ready-to-show sentences.
    pub violations: Vec<String>,
    /// Code of Practice checks.
    pub checks: Vec<Check>,
    /// "Maximise floor space" with nothing that limits floor space.
    pub floor_space_unlimited: bool,
    /// Mean / P10 / P90 of every result, by `MetricKey::key`.
    pub metrics: HashMap<&'static str, Stat>,
    pub layout: Layout,
    /// What is in each cage on a typical day: the last day of the run with the median final headcount
    /// (same order as `layout.cages`).
    pub snapshot: Vec<CageSnap>,
    /// What is in each cage on the busiest day of the run with the median peak headcount.
    pub peak_snapshot: Vec<CageSnap>,
    /// Weekly series across runs.
    pub series: SeriesView,
    pub eff_wean: u32,
    pub eff_does: usize,
    pub runs: usize,
    /// Weeks at the start excluded from annual figures (shaded on charts).
    pub warm_weeks: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeriesView {
    pub head: Series,
    pub growers: Series,
    pub cages: Series,
    pub meat: Series,
    pub cost: Series,
}

/// The API with its solution cache. One per process (server) or per WASM instance.
pub struct Api {
    cache: Mutex<HashMap<String, Arc<Solution>>>,
    presets: Source,
    cfg: OptConfig,
}

#[derive(Deserialize)]
struct ReqBody {
    #[serde(default)]
    request: Option<WireRequest>,
}

#[derive(Deserialize)]
struct KeyBody {
    #[serde(default)]
    request: Option<WireRequest>,
    key: String,
}

#[derive(Deserialize)]
struct ObjectiveBody {
    #[serde(default)]
    request: Option<WireRequest>,
    objective: String,
}

#[derive(Deserialize)]
struct ExportBody {
    #[serde(default)]
    request: Option<WireRequest>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct IdBody {
    id: String,
}

#[derive(Deserialize)]
struct TextBody {
    text: String,
}

#[derive(Serialize)]
struct RequestReply {
    request: WireRequest,
}

#[derive(Serialize)]
struct TextReply {
    text: String,
}

fn parse<T: for<'de> Deserialize<'de>>(body: &str) -> Result<T, ApiError> {
    let body = if body.trim().is_empty() { "{}" } else { body };
    serde_json::from_str(body).map_err(|e| ApiError::bad(format!("bad request body: {e}")))
}

fn reply<T: Serialize>(v: &T) -> Result<String, ApiError> {
    serde_json::to_string(v).map_err(|e| ApiError { status: 500, message: format!("could not encode reply: {e}") })
}

fn request_of(w: Option<WireRequest>) -> Request {
    w.map_or_else(Request::default, |w| Request::from_wire(&w))
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

impl Api {
    const CACHE_LIMIT: usize = 64;

    pub fn new(presets: Source) -> Self {
        Api { cache: Mutex::new(HashMap::new()), presets, cfg: OptConfig::default() }
    }

    /// Use a cheaper solver (tests).
    pub fn with_config(mut self, cfg: OptConfig) -> Self {
        self.cfg = cfg;
        self
    }

    /// Solve, reusing an earlier answer to the same problem (cost lines and slider drags repeat work).
    pub fn solve_cached(&self, problem: &Problem) -> Arc<Solution> {
        let key = serde_json::to_string(problem).unwrap_or_default();
        if let Some(s) = self.cache.lock().unwrap().get(&key) {
            return s.clone();
        }
        let s = Arc::new(run_solver(problem, self.cfg));
        let mut c = self.cache.lock().unwrap();
        if c.len() >= Self::CACHE_LIMIT {
            c.clear();
        }
        c.insert(key, s.clone());
        s
    }

    /// Run one API method.
    pub fn call(&self, method: &str, body: &str) -> Result<String, ApiError> {
        match method {
            "meta" => reply(&meta::meta()),
            "presets" => reply(&self.presets.list()),
            "preset_load" => {
                let b: IdBody = parse(body)?;
                let p = self.presets.load(&b.id).map_err(ApiError::not_found)?;
                let name = p.name.clone().unwrap_or_else(|| b.id.clone());
                reply(&self.loaded(p, &name))
            }
            "preset_parse" => {
                let b: TextBody = parse(body)?;
                if b.text.trim().is_empty() {
                    return Err(ApiError::bad("That file was empty, so nothing changed."));
                }
                let p = presets::parse(&b.text);
                let name = p.name.clone().unwrap_or_else(|| "imported preset".into());
                reply(&self.loaded(p, &name))
            }
            "preset_export" => {
                let b: ExportBody = parse(body)?;
                let name = b.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "My plan".into());
                reply(&TextReply { text: presets::to_text(&request_of(b.request), name.trim()) })
            }
            "solve" => {
                let b: ReqBody = parse(body)?;
                reply(&self.solve(&request_of(b.request)))
            }
            "cost" => {
                let b: KeyBody = parse(body)?;
                reply(&TextReply { text: self.cost(&request_of(b.request), &b.key)? })
            }
            "add_constraint" => {
                let b: KeyBody = parse(body)?;
                let mut req = request_of(b.request);
                if Var::from_key(&b.key).is_none() && Out::from_key(&b.key).is_none() {
                    return Err(ApiError::bad(format!("unknown constraint “{}”", b.key)));
                }
                let current = self.solve_cached(&req.problem());
                req.add_from(&b.key, &current);
                reply(&RequestReply { request: req.to_wire() })
            }
            "set_objective" => {
                let b: ObjectiveBody = parse(body)?;
                let mut req = request_of(b.request);
                req.objective =
                    Objective::from_key(&b.objective).ok_or_else(|| ApiError::bad(format!("unknown objective “{}”", b.objective)))?;
                // "Maximise floor space" needs something to trade against: suggest meat and herd-size minimums.
                if req.objective == Objective::MaxQol && !(req.has(Out::MeatKgPerYr.key()) && req.has(Out::PeakHead.key())) {
                    let mut best = req.clone();
                    best.objective = Objective::MaxMeat;
                    let s = self.solve_cached(&best.problem());
                    req.suggest_floors(s.summary.metric(MetricKey::MeatKgPerYr).mean, s.summary.metric(MetricKey::PeakHead).mean);
                }
                reply(&RequestReply { request: req.to_wire() })
            }
            _ => Err(ApiError::not_found(format!("no API method “{}”", method.chars().take(40).collect::<String>()))),
        }
    }

    fn loaded(&self, p: presets::Parsed, name: &str) -> Loaded {
        Loaded {
            note: preset_note(name, &p.warnings),
            request: p.request.to_wire(),
            name: p.name,
            description: p.description,
            warnings: p.warnings,
        }
    }

    /// Preset list (convenience for callers that don't want JSON).
    pub fn preset_list(&self) -> Vec<PresetInfo> {
        self.presets.list()
    }

    pub fn solve(&self, req: &Request) -> SolveView {
        let sol = self.solve_cached(&req.problem());
        let s = &sol.summary;
        let p = &sol.params;
        SolveView {
            request: req.to_wire(),
            params: p.clone(),
            objective_value: sol.objective_value,
            feasible: sol.feasible(),
            fits: sol.fits,
            forced_share: s.forced_share(),
            violations: report::violation_texts(&sol),
            checks: report::code_checks(&sol),
            floor_space_unlimited: report::floor_space_unlimited(req),
            metrics: MetricKey::ALL.iter().map(|k| (k.key(), s.metric(*k))).collect(),
            layout: s.layout.clone(),
            snapshot: s.snapshot.clone(),
            peak_snapshot: s.peak_snapshot.clone(),
            series: SeriesView {
                head: s.head.clone(),
                growers: s.growers.clone(),
                cages: s.cages.clone(),
                meat: s.meat.clone(),
                cost: s.cost.clone(),
            },
            eff_wean: s.eff_wean,
            eff_does: s.eff_does,
            runs: s.runs,
            warm_weeks: (counted_from_day(p) / 7) as usize,
        }
    }

    /// What the constraint `key` costs: the plan with and without it.
    pub fn cost(&self, req: &Request, key: &str) -> Result<String, ApiError> {
        if !req.has(key) {
            return Err(ApiError::bad(format!("no constraint “{}” in the request", key.chars().take(40).collect::<String>())));
        }
        let mut without = req.clone();
        without.remove(key);
        let full = self.solve_cached(&req.problem());
        let other = self.solve_cached(&without.problem());
        Ok(report::cost_text(req.objective, &full, &other))
    }
}

/// Parse a reply back into JSON (tests and Rust callers).
pub fn json(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or(Value::Null)
}
