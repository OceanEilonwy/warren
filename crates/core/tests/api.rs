//! The transport-free API: what both the HTTP server and the WASM build expose.

use rabbitsim::api::{Api, json, presets::Source};
use rabbitsim::engine::OptConfig;
use serde_json::{Value, json as j};

fn presets_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../presets"))
}

fn api() -> Api {
    Api::new(Source::Dir(presets_dir())).with_config(OptConfig { eval_runs: 16, eval_years: 3, rounds: 2 })
}

/// The default request with cheap simulation settings.
fn base() -> Value {
    let mut r = json(&api().call("meta", "").unwrap())["defaults"].clone();
    r["params"]["runs"] = j!(24);
    r["params"]["years"] = j!(3);
    r
}

fn call(api: &Api, method: &str, body: Value) -> Value {
    json(&api.call(method, &body.to_string()).unwrap_or_else(|e| panic!("{method}: {e}")))
}

fn keys(req: &Value) -> Vec<String> {
    req["constraints"].as_array().unwrap().iter().map(|c| c["key"].as_str().unwrap().to_string()).collect()
}

fn meat(v: &Value) -> f64 {
    v["metrics"]["meat_kg_per_yr"]["mean"].as_f64().unwrap()
}

#[test]
fn meta_describes_every_input_constraint_and_objective() {
    let m = call(&api(), "meta", j!({}));
    assert_eq!(m["space"]["title"], "Space");
    let space: Vec<&str> = m["space"]["ctls"].as_array().unwrap().iter().map(|c| c["key"].as_str().unwrap()).collect();
    assert_eq!(space, ["width_m", "depth_m", "height_m", "access_both_sides"]);
    assert!(m["assumptions"].as_array().unwrap().len() >= 5);
    let pct = m["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["ctls"].as_array().unwrap())
        .find(|c| c["key"] == "pre_wean_mortality")
        .unwrap();
    assert_eq!(pct["fmt"], "pct");
    assert_eq!(pct["kind"]["type"], "range");
    let vars = m["vars"].as_array().unwrap();
    assert_eq!(vars.len(), 14);
    let cap = vars.iter().find(|v| v["key"] == "max_headcount").unwrap();
    assert_eq!(cap["ops"], j!(["max"]), "the herd cap is a ceiling only");
    let rule = vars.iter().find(|v| v["key"] == "butcher_rule").unwrap();
    assert_eq!(rule["kind"]["type"], "choice");
    assert_eq!(m["outs"].as_array().unwrap().len(), 3);
    assert_eq!(m["objectives"].as_array().unwrap().len(), 2);
    assert!(keys(&m["defaults"]).is_empty(), "nothing is constrained to start with");
    assert_eq!(m["defaults"]["objective"], "max_meat");
    // tiers is an optimisable input, not part of the space
    assert!(!space.contains(&"tiers") && vars.iter().any(|v| v["key"] == "tiers"));
}

#[test]
fn solve_returns_a_plan_with_results_layout_and_series() {
    let v = call(&api(), "solve", j!({ "request": base() }));
    assert!(meat(&v) > 100.0, "meat {}", meat(&v));
    assert_eq!(v["objective_value"].as_f64().unwrap(), meat(&v));
    assert!(v["feasible"].as_bool().unwrap());
    let cages = v["layout"]["cages"].as_array().unwrap().len();
    assert!(cages > 0);
    assert_eq!(v["snapshot"].as_array().unwrap().len(), cages);
    let weeks = v["series"]["head"]["mean"].as_array().unwrap().len();
    assert_eq!(weeks, 156);
    assert!(v["warm_weeks"].as_u64().unwrap() > 0 && (v["warm_weeks"].as_u64().unwrap() as usize) < weeks);
    assert!(!v["checks"].as_array().unwrap().is_empty());
    for k in ["meat_kg_per_yr", "fryers_per_yr", "cost_per_kg", "cage_use", "peak_head"] {
        assert!(v["metrics"][k]["p10"].as_f64().is_some(), "{k}");
    }
    assert_eq!(v["request"], base(), "the normalised request comes back");
}

#[test]
fn constraints_change_the_plan_and_are_reported_when_missed() {
    let a = api();
    let mut r = base();
    r["constraints"] = j!([{ "key": "qol", "op": "min", "value": 2.0 }]);
    let roomy = call(&a, "solve", j!({ "request": r }));
    assert_eq!(roomy["params"]["qol_doe"], 2.0);
    let plain = call(&a, "solve", j!({ "request": base() }));
    assert!(meat(&roomy) < meat(&plain));
    let mut r = base();
    r["constraints"] = j!([{ "key": "meat", "op": "min", "value": 900 }]);
    let v = call(&a, "solve", j!({ "request": r }));
    assert!(!v["feasible"].as_bool().unwrap());
    let why = v["violations"].as_array().unwrap();
    assert!(why.iter().any(|t| t.as_str().unwrap().starts_with("Dressed meat: wanted at least 900")), "{why:?}");
}

#[test]
fn adding_a_constraint_anchors_it_on_the_current_plan() {
    let a = api();
    let v = call(&a, "solve", j!({ "request": base() }));
    let r = call(&a, "add_constraint", j!({ "request": base(), "key": "does" }))["request"].clone();
    assert_eq!(keys(&r), ["does"]);
    let c = r["constraints"].as_array().unwrap().iter().find(|c| c["key"] == "does").unwrap().clone();
    assert_eq!(c["op"], "eq");
    assert_eq!(c["value"].as_f64(), v["params"]["does"].as_f64(), "pinned at what the plan already uses");
    let r = call(&a, "add_constraint", j!({ "request": base(), "key": "meat" }))["request"].clone();
    let c = r["constraints"].as_array().unwrap().iter().find(|c| c["key"] == "meat").unwrap().clone();
    assert_eq!(c["op"], "min");
    assert_eq!(c["value"].as_f64().unwrap(), meat(&v).floor());
    assert_eq!(a.call("add_constraint", &j!({ "request": base(), "key": "nope" }).to_string()).unwrap_err().status, 400);
}

#[test]
fn switching_to_max_floor_space_suggests_meat_and_herd_minimums() {
    let a = api();
    let r = call(&a, "set_objective", j!({ "request": base(), "objective": "max_qol" }))["request"].clone();
    assert_eq!(r["objective"], "max_qol");
    assert!(keys(&r).contains(&"meat".to_string()) && keys(&r).contains(&"peak_head".to_string()));
    let v = call(&a, "solve", j!({ "request": r }));
    assert!(!v["floor_space_unlimited"].as_bool().unwrap());
    // nothing that limits room: the plan says so
    let mut r = base();
    r["objective"] = j!("max_qol");
    assert!(call(&a, "solve", j!({ "request": r }))["floor_space_unlimited"].as_bool().unwrap());
    // back to max meat leaves the constraints alone
    let back = call(&a, "set_objective", j!({ "request": base(), "objective": "max_meat" }))["request"].clone();
    assert!(keys(&back).is_empty());
}

#[test]
fn cost_says_what_a_constraint_costs() {
    let a = api();
    let mut r = base();
    r["constraints"] = j!([{ "key": "qol", "op": "min", "value": 1.75 }, { "key": "max_headcount", "op": "max", "value": 0 }]);
    let t = call(&a, "cost", j!({ "request": r.clone(), "key": "qol" }))["text"].as_str().unwrap().to_string();
    assert!(t.starts_with("Costs ") && t.contains("kg/yr"), "{t}");
    let t = call(&a, "cost", j!({ "request": r.clone(), "key": "max_headcount" }))["text"].as_str().unwrap().to_string();
    assert!(t.starts_with("Not limiting"), "{t}");
    assert_eq!(a.call("cost", &j!({ "request": r, "key": "does" }).to_string()).unwrap_err().status, 400, "not in the request");
}

#[test]
fn bad_input_is_an_error_or_clamped_never_a_panic() {
    let a = api();
    assert_eq!(a.call("nope", "{}").unwrap_err().status, 404);
    assert_eq!(a.call("solve", "not json").unwrap_err().status, 400);
    assert_eq!(a.call("cost", "{}").unwrap_err().status, 400, "missing key");
    let mut r = base();
    r["params"]["width_m"] = j!(1e9);
    r["params"]["depth_m"] = j!(-5);
    r["objective"] = j!("whatever");
    r["constraints"] = j!([{ "key": "does", "op": "?", "value": 999 }, { "key": "zzz", "op": "min", "value": 1 }]);
    let v = call(&a, "solve", j!({ "request": r }));
    assert_eq!(v["request"]["params"]["width_m"], 6.0);
    assert_eq!(v["request"]["params"]["depth_m"], 0.4);
    assert_eq!(v["request"]["objective"], "max_meat");
    assert_eq!(v["request"]["constraints"], j!([{ "key": "does", "op": "eq", "value": 24.0 }]));
    // an empty body means "the defaults"
    assert!(a.call("solve", "").is_ok());
}

#[test]
fn an_infeasible_space_solves_to_an_empty_plan() {
    let mut r = base();
    r["params"]["width_m"] = j!(0.5);
    r["params"]["depth_m"] = j!(0.4);
    let v = call(&api(), "solve", j!({ "request": r }));
    assert_eq!(v["runs"], 0);
    assert_eq!(v["eff_does"], 0);
    assert!(!v["layout"]["issues"].as_array().unwrap().is_empty());
}

#[test]
fn presets_list_load_export_and_import() {
    let a = api();
    let list = call(&a, "presets", j!({}));
    let list = list.as_array().unwrap();
    assert!(list.len() >= 5);
    assert!(list.iter().all(|p| !p["name"].as_str().unwrap().is_empty() && !p["description"].as_str().unwrap().is_empty()));
    let id = list[0]["id"].as_str().unwrap();
    let loaded = call(&a, "preset_load", j!({ "id": id }));
    assert!(loaded["warnings"].as_array().unwrap().is_empty());
    assert!(loaded["note"].as_str().unwrap().starts_with("Loaded “"));
    // export, then import: the same setup
    let text = call(&a, "preset_export", j!({ "request": loaded["request"], "name": "Round trip" }))["text"].as_str().unwrap().to_string();
    assert!(text.contains("name = Round trip"));
    let back = call(&a, "preset_parse", j!({ "text": text }));
    assert_eq!(back["request"], loaded["request"]);
    assert!(back["warnings"].as_array().unwrap().is_empty());
    // bad files explain what was ignored
    let bad = call(&a, "preset_parse", j!({ "text": "width_m = 99\nnonsense\nconstraint.does = sideways 3\n" }));
    let w: Vec<&str> = bad["warnings"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert_eq!(w.len(), 3, "{w:?}");
    assert!(bad["note"].as_str().unwrap().contains("Ignored: line 1"));
    assert_eq!(a.call("preset_parse", r#"{"text":"  "}"#).unwrap_err().status, 400);
    // ids cannot escape the directory
    for id in ["../Cargo", "a/b", "", "nope"] {
        assert_eq!(a.call("preset_load", &j!({ "id": id }).to_string()).unwrap_err().status, 404, "{id}");
    }
}

#[test]
fn embedded_presets_match_the_files_on_disk() {
    let disk = Api::new(Source::Dir(presets_dir()));
    let built_in = Api::new(Source::Embedded);
    assert_eq!(disk.preset_list(), built_in.preset_list());
    for p in built_in.preset_list() {
        assert_eq!(call(&disk, "preset_load", j!({ "id": p.id })), call(&built_in, "preset_load", j!({ "id": p.id })), "{}", p.id);
    }
}

#[test]
fn every_shipped_preset_has_a_workable_plan() {
    let a = api();
    for p in a.preset_list() {
        let loaded = call(&a, "preset_load", j!({ "id": p.id }));
        let mut r = loaded["request"].clone();
        r["params"]["runs"] = j!(24);
        r["params"]["years"] = j!(3);
        let v = call(&a, "solve", j!({ "request": r }));
        assert!(v["feasible"].as_bool().unwrap(), "{}: {:?}", p.id, v["violations"]);
    }
}

#[test]
fn repeated_solves_hit_the_cache() {
    let a = api();
    let body = j!({ "request": base() }).to_string();
    a.call("solve", &body).unwrap();
    let t = std::time::Instant::now();
    a.call("solve", &body).unwrap();
    assert!(t.elapsed().as_millis() < 50, "second identical solve took {:?}", t.elapsed());
}
