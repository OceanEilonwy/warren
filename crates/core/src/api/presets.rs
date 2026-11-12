//! Presets: plain-text `key = value` files that describe a whole setup (objective, space, assumptions and
//! constraints). Shipped ones live in a directory on disk and are read on every request, so adding a file
//! adds a preset. The same format is what "Export" writes and "Import" reads.
//!
//! ```text
//! # comments start with #
//! name = Backyard starter
//! description = One line shown in the preset list
//! objective = max_meat            # or max_qol
//! width_m = 2                     # any space or assumption input, by its key
//! constraint.qol = min 1          # constraint.<key> = <min|eq|max> <value>
//! constraint.meat = min 250
//! ```

use std::collections::HashMap;
use std::path::Path;

use super::meta::{ASSUMPTIONS, Kind, Op, Request, SPACE, allowed_ops, ctl, out_ui, var_ui};
use crate::engine::{Objective, Out, Var};

/// Largest preset text accepted (bytes).
pub const MAX_PRESET_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PresetInfo {
    /// File name without `.txt`; only letters, digits, `-` and `_`.
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug)]
pub struct Parsed {
    pub request: Request,
    pub name: Option<String>,
    pub description: Option<String>,
    /// Lines or values that were ignored, for showing to the user.
    pub warnings: Vec<String>,
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Where presets come from: a directory read on every call (the server, so dropping a file in adds a
/// preset), or the files in `presets/` built into the binary (the browser/WASM build).
#[derive(Clone, Debug)]
pub enum Source {
    Dir(std::path::PathBuf),
    Embedded,
}

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_presets.rs"));
}

impl Source {
    pub fn list(&self) -> Vec<PresetInfo> {
        match self {
            Source::Dir(d) => list(d),
            Source::Embedded => {
                let mut out: Vec<PresetInfo> = embedded::PRESETS.iter().map(|(id, text)| info(id, &parse(text))).collect();
                out.sort_by_key(|a| a.name.to_lowercase());
                out
            }
        }
    }

    pub fn load(&self, id: &str) -> Result<Parsed, String> {
        match self {
            Source::Dir(d) => load(d, id),
            Source::Embedded => {
                if !valid_id(id) {
                    return Err(bad_id(id));
                }
                embedded::PRESETS.iter().find(|(i, _)| *i == id).map(|(_, t)| parse(t)).ok_or_else(|| format!("No preset called “{id}”"))
            }
        }
    }
}

fn info(id: &str, p: &Parsed) -> PresetInfo {
    PresetInfo {
        name: p.name.clone().unwrap_or_else(|| id.replace(['-', '_'], " ")),
        description: p.description.clone().unwrap_or_default(),
        id: id.to_string(),
    }
}

fn bad_id(id: &str) -> String {
    format!("“{}” is not a valid preset name", id.chars().take(40).collect::<String>())
}

/// Presets in `dir`, sorted by name. A missing or unreadable directory is just an empty list.
pub fn list(dir: &Path) -> Vec<PresetInfo> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PresetInfo> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("txt") {
                return None;
            }
            let id = path.file_stem()?.to_str()?.to_string();
            if !valid_id(&id) {
                return None;
            }
            let text = read_limited(&path)?;
            Some(info(&id, &parse(&text)))
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

fn read_limited(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() as usize > MAX_PRESET_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Load one preset by id. The id must be a plain file name, so nothing outside `dir` can be read.
pub fn load(dir: &Path, id: &str) -> Result<Parsed, String> {
    if !valid_id(id) {
        return Err(bad_id(id));
    }
    let text = read_limited(&dir.join(format!("{id}.txt"))).ok_or_else(|| format!("No preset called “{id}”"))?;
    Ok(parse(&text))
}

fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.to_string() }
}

pub fn parse(text: &str) -> Parsed {
    let mut warnings = Vec::new();
    let mut name = None;
    let mut description = None;
    let mut req = Request::default();
    req.vars.clear();
    req.outs.clear();
    let mut form: HashMap<String, String> = HashMap::new();
    if text.len() > MAX_PRESET_BYTES {
        warnings.push(format!("The file is larger than {} KB, so nothing was read", MAX_PRESET_BYTES / 1024));
        return Parsed { request: Request::default(), name, description, warnings };
    }
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        // `description = a # b` is allowed to contain a hash only if the whole line is a description: keep simple
        if line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            warnings.push(format!("line {n}: expected “key = value”"));
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "name" => name = Some(v.chars().take(80).collect()),
            "description" => description = Some(v.chars().take(200).collect()),
            "objective" => match Objective::from_key(v) {
                Some(o) => req.objective = o,
                None => warnings.push(format!("line {n}: unknown objective “{v}” (use max_meat or max_qol)")),
            },
            k if k.starts_with("constraint.") => constraint(&mut req, &k["constraint.".len()..], v, n, &mut warnings),
            k => match ctl(k) {
                None => warnings.push(format!("line {n}: unknown setting “{k}”")),
                Some(c) => {
                    let ok = match c.kind {
                        Kind::Bool => matches!(v, "true" | "false" | "1" | "0" | "yes" | "no" | "on" | "off"),
                        Kind::Choice(cs) => v.parse::<f64>().is_ok_and(|x| cs.iter().any(|(y, _)| (x - y).abs() < 1e-9)),
                        Kind::Range { min, max, .. } => match v.parse::<f64>() {
                            Ok(x) if x.is_finite() => {
                                if x < min - 1e-9 || x > max + 1e-9 {
                                    warnings.push(format!("line {n}: {k} = {v} is outside {}–{}, so it was clamped", num(min), num(max)));
                                }
                                true
                            }
                            _ => false,
                        },
                    };
                    if ok {
                        form.insert(k.to_string(), v.to_string());
                    } else {
                        warnings.push(format!("line {n}: bad value “{v}” for {k}"));
                    }
                }
            },
        }
    }
    super::meta::apply_form(&mut req.params, &form);
    Parsed { request: req, name, description, warnings }
}

fn constraint(req: &mut Request, key: &str, value: &str, n: usize, warnings: &mut Vec<String>) {
    let mut parts = value.split_whitespace();
    let (op_s, num_s) = match (parts.next(), parts.next(), parts.next()) {
        (Some(a), Some(b), None) => (Some(a), b),
        (Some(a), None, None) => (None, a),
        _ => {
            warnings.push(format!("line {n}: constraint.{key} should look like “min 250”"));
            return;
        }
    };
    let op = match op_s {
        None => None,
        Some("min") | Some(">=") => Some(Op::AtLeast),
        Some("eq") | Some("=") | Some("==") => Some(Op::Exactly),
        Some("max") | Some("<=") => Some(Op::AtMost),
        Some(o) => {
            warnings.push(format!("line {n}: unknown operator “{o}” (use min, eq or max)"));
            return;
        }
    };
    let Ok(x) = num_s.parse::<f64>() else {
        warnings.push(format!("line {n}: “{num_s}” is not a number"));
        return;
    };
    if !x.is_finite() {
        warnings.push(format!("line {n}: “{num_s}” is not a number"));
        return;
    }
    if let Some(var) = Var::from_key(key) {
        let u = var_ui(var);
        let allowed = allowed_ops(var);
        let fallback = if allowed.contains(&Op::Exactly) { Op::Exactly } else { allowed[0] };
        let op = op.filter(|o| allowed.contains(o)).unwrap_or(fallback);
        let (min, max) = u.range().map_or((0.0, 1.0), |(a, b, _)| (a, b));
        req.vars.retain(|(v, _)| *v != var);
        req.vars.push((var, op.bound(x.clamp(min, max))));
    } else if let Some(out) = Out::from_key(key) {
        let u = out_ui(out);
        req.outs.retain(|(o, _)| *o != out);
        req.outs.push((out, op.unwrap_or(Op::Exactly).bound(x.clamp(u.min, u.max))));
    } else {
        warnings.push(format!("line {n}: unknown constraint “{key}”"));
    }
}

/// The text written by "Export": everything needed to rebuild `req`.
pub fn to_text(req: &Request, name: &str) -> String {
    let mut s = String::new();
    s.push_str("# NZ White meat rabbit planner preset\n");
    s.push_str("# One `key = value` per line. Constraints: `constraint.<name> = <min|eq|max> <value>`.\n");
    s.push_str(&format!("name = {name}\nobjective = {}\n", req.objective.key()));
    for g in std::iter::once(&SPACE).chain(ASSUMPTIONS.iter()) {
        s.push_str(&format!("\n# {}\n", g.title));
        for c in g.ctls {
            let v = (c.get)(&req.params);
            let text = match c.kind {
                Kind::Bool => (if v > 0.5 { "true" } else { "false" }).to_string(),
                _ => num(v),
            };
            s.push_str(&format!("{} = {}\n", c.key, text));
        }
    }
    s.push_str("\n# Constraints\n");
    for (v, b) in &req.vars {
        s.push_str(&format!("constraint.{} = {} {}\n", v.key(), Op::of(b).key(), num(super::meta::bound_value(b))));
    }
    for (o, b) in &req.outs {
        s.push_str(&format!("constraint.{} = {} {}\n", o.key(), Op::of(b).key(), num(super::meta::bound_value(b))));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::meta::all_ctls;
    use crate::engine::{Bound, Params};

    #[test]
    fn ids_are_plain_file_names() {
        for ok in ["backyard", "a-b_c9", "X"] {
            assert!(valid_id(ok), "{ok}");
        }
        for bad in ["", "../etc/passwd", "a/b", "a.b", "a b", "%2e%2e", "ü", &"x".repeat(65)] {
            assert!(!valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn export_then_parse_round_trips_everything() {
        let mut req = Request { objective: Objective::MaxQol, ..Request::default() };
        req.params.width_m = 3.25;
        req.params.access_both_sides = false;
        req.params.pre_wean_mortality = 0.15;
        req.params.dressed_weight_kg = 1.61;
        req.params.runs = 700;
        req.vars.push((Var::Does, Bound::at_least(4.0)));
        req.vars.push((Var::QolDoe, Bound::exactly(1.5)));
        req.outs.push((Out::MeatKgPerYr, Bound::at_least(200.0)));
        req.outs.push((Out::PeakHead, Bound::at_most(80.0)));
        let text = to_text(&req, "My plan");
        let p = parse(&text);
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
        assert_eq!(p.name.as_deref(), Some("My plan"));
        assert_eq!(p.request.objective, req.objective);
        assert_eq!(p.request.params, req.params);
        let sorted = |mut v: Vec<(Var, Bound)>| {
            v.sort_by_key(|(x, _)| x.key());
            v
        };
        assert_eq!(sorted(p.request.vars.clone()), sorted(req.vars.clone()));
        assert_eq!(p.request.outs.len(), 2);
        assert!(p.request.outs.iter().all(|x| req.outs.contains(x)));
    }

    #[test]
    fn defaults_round_trip_too() {
        let req = Request::default();
        let p = parse(&to_text(&req, "x"));
        assert!(p.warnings.is_empty());
        assert_eq!(p.request, req);
    }

    #[test]
    fn comments_blank_lines_and_spacing_are_fine() {
        let p = parse("# hi\n\n  width_m=4   # trailing comment\nobjective =   max_meat\nconstraint.does = >= 3\nconstraint.meat = 200\n");
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
        assert_eq!(p.request.params.width_m, 4.0);
        assert_eq!(p.request.vars, vec![(Var::Does, Bound::at_least(3.0))]);
        assert_eq!(p.request.outs, vec![(Out::MeatKgPerYr, Bound::exactly(200.0))], "no operator means exactly");
    }

    #[test]
    fn a_preset_with_no_constraint_lines_has_no_constraints() {
        let p = parse("width_m = 2\n");
        assert!(p.request.vars.is_empty() && p.request.outs.is_empty());
    }

    #[test]
    fn problems_are_reported_and_never_fatal() {
        let p = parse(
            "nonsense\nwidth_m = wide\nbogus = 1\nobjective = fastest\nconstraint.does = sideways 3\nconstraint.does = min x\nconstraint.nope = min 1\nconstraint.does = 1 2 3\ndepth_m = 99\naccess_both_sides = maybe\n",
        );
        let w = p.warnings.join("\n");
        for needle in [
            "expected “key = value”",
            "bad value “wide”",
            "unknown setting “bogus”",
            "unknown objective",
            "unknown operator",
            "not a number",
            "unknown constraint “nope”",
            "should look like",
            "clamped",
            "bad value “maybe”",
        ] {
            assert!(w.contains(needle), "missing {needle:?} in:\n{w}");
        }
        assert_eq!(p.request.params.width_m, Params::default().width_m, "bad value ignored");
        assert_eq!(p.request.params.depth_m, 3.0, "out of range clamped to the slider max");
    }

    #[test]
    fn herd_cap_is_always_at_most_and_values_are_clamped() {
        let p = parse("constraint.max_headcount = min 40\nconstraint.does = min 999\nconstraint.meat = max -5\n");
        assert_eq!(p.request.vars[0], (Var::MaxHeadcount, Bound::at_most(40.0)));
        assert_eq!(p.request.vars[1], (Var::Does, Bound::at_least(24.0)));
        assert_eq!(p.request.outs[0], (Out::MeatKgPerYr, Bound::at_most(0.0)));
    }

    #[test]
    fn later_lines_win_and_oversized_text_is_refused() {
        let p = parse("constraint.does = min 3\nconstraint.does = max 5\n");
        assert_eq!(p.request.vars, vec![(Var::Does, Bound::at_most(5.0))]);
        let big = format!("width_m = 4\n{}", "# pad\n".repeat(MAX_PRESET_BYTES / 4));
        let p = parse(&big);
        assert!(p.warnings.iter().any(|w| w.contains("larger than")));
        assert_eq!(p.request.params.width_m, Params::default().width_m);
    }

    #[test]
    fn every_input_key_is_written_and_read() {
        let text = to_text(&Request::default(), "x");
        for c in all_ctls() {
            assert!(text.contains(&format!("\n{} = ", c.key)), "export is missing {}", c.key);
        }
    }

    #[test]
    fn listing_and_loading_from_a_directory() {
        let dir = std::env::temp_dir().join(format!("rabbit-presets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("zeta.txt"), "name = Zeta\ndescription = last\nwidth_m = 5\n").unwrap();
        std::fs::write(dir.join("alpha-one.txt"), "width_m = 1\n").unwrap();
        std::fs::write(dir.join("ignored.md"), "x").unwrap();
        std::fs::write(dir.join("bad name.txt"), "x").unwrap();
        let l = list(&dir);
        assert_eq!(l.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["alpha-one", "zeta"]);
        assert_eq!(l[0].name, "alpha one", "no name line: made from the file name");
        assert_eq!(l[1].description, "last");
        assert_eq!(load(&dir, "zeta").unwrap().request.params.width_m, 5.0);
        assert!(load(&dir, "../zeta").is_err() && load(&dir, "missing").is_err() && load(&dir, "").is_err());
        assert!(list(&dir.join("nope")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
