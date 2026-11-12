//! Wording shared by every front end: Code of Practice checks, what a constraint costs, and why a plan
//! misses a constraint. Kept here (not in the UI) because it reads engine internals.

use super::meta::{Request, out_ui, var_ui};
use crate::engine::{Bound, MetricKey, Objective, Solution, Subject, code};

pub fn fmt_num(v: f64, dp: usize) -> String {
    let s = format!("{:.*}", dp, v.abs());
    let (int, frac) = s.split_once('.').map_or((s.as_str(), None), |(a, b)| (a, Some(b)));
    let mut out = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    if v < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') { format!("-{out}") } else { out }
}

fn trim1(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// What a constraint costs, in the objective's own terms: `full` is the plan with it, `without` without.
pub fn cost_text(objective: Objective, full: &Solution, without: &Solution) -> String {
    if !full.feasible() && without.feasible() {
        return "Conflicts with your other constraints — removing it would make the plan work.".into();
    }
    let gain = objective.improvement(full.objective_value, without.objective_value);
    if gain < objective.epsilon() {
        return "Not limiting — the plan would be the same without it.".into();
    }
    match objective {
        Objective::MaxMeat => {
            let pct = if full.objective_value > 0.0 { gain / full.objective_value * 100.0 } else { 0.0 };
            format!("Costs {} kg/yr of meat ({}%)", fmt_num(gain, 0), fmt_num(pct, 0))
        }
        Objective::MaxQol => format!("Costs {:.0}% of extra floor space", gain * 100.0),
    }
}

/// One Code of Practice check.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Check {
    pub ok: bool,
    pub text: String,
}

/// Every check, passed or not.
pub fn code_checks(sol: &Solution) -> Vec<Check> {
    let s = &sol.summary;
    let l = &s.layout;
    let p = &sol.params;
    let w = if s.runs > 0 { s.eff_wean } else { p.wean_days.min(56) };
    let doe_min = (if w > 35 { code::DOE_LITTER_8WK } else { code::DOE_LITTER_5WK }) * p.qol_doe;
    let forced = s.metric(MetricKey::ForcedPerYr).mean;
    let share = s.forced_share();
    let mut c = vec![
        (
            l.n_doe > 0 && l.doe_area + 1e-9 >= doe_min,
            format!("Doe + litter cages are {:.2} m² (Code: {doe_min:.2} m² for litters to {} wk)", l.doe_area, trim1(w as f64 / 7.0)),
        ),
        (
            l.n_buck > 0 && l.buck_area + 1e-9 >= code::BREEDER * p.qol_buck,
            format!("Buck cages are {:.2} m² (adult breeder minimum {:.2} m²)", l.buck_area, code::BREEDER * p.qol_buck),
        ),
        (
            p.cage_height_cm >= code::MIN_HEIGHT_CM,
            format!("Cage height {} cm (minimum 45 cm, and enough to sit up with ears erect)", p.cage_height_cm),
        ),
        (
            true,
            format!(
                "Growers get {:.2} m² each (5–12 wk) and {:.2} m² each (12 wk+), checked every simulated day",
                code::GROWER_5_TO_12WK * p.qol_young,
                code::OVER_12WK_GROUP * p.qol_old
            ),
        ),
        (true, "Replacement bucks housed alone from 10 weeks; replacement does from 12 weeks".to_string()),
    ];
    if s.runs > 0 && s.eff_does < p.does as usize {
        c.push((false, format!("Only {} of {} doe cages fit in this space", s.eff_does, p.does)));
    }
    if forced > 0.5 {
        if share > 0.03 {
            c.push((
                false,
                format!(
                    "{:.0}% of rabbits ({forced:.0}/yr) have to be butchered before the planned age because the cages are full",
                    share * 100.0
                ),
            ));
        } else {
            c.push((true, format!("Occasionally (about {forced:.0}/yr) a rabbit is butchered a little early because the cages are full")));
        }
    }
    for i in &l.issues {
        c.push((false, i.clone()));
    }
    c.into_iter().map(|(ok, text)| Check { ok, text }).collect()
}

fn bound_text(b: &Bound, fmt: impl Fn(f64) -> String) -> String {
    match (b.exact(), b.min, b.max) {
        (Some(x), _, _) => format!("exactly {}", fmt(x)),
        (None, Some(m), None) => format!("at least {}", fmt(m)),
        (None, None, Some(m)) => format!("at most {}", fmt(m)),
        (None, Some(a), Some(c)) => format!("{}–{}", fmt(a), fmt(c)),
        _ => "any".into(),
    }
}

/// Each constraint the plan misses, as "Label: wanted …, got …".
pub fn violation_texts(sol: &Solution) -> Vec<String> {
    sol.violations
        .iter()
        .map(|x| match x.subject {
            Subject::Var(var) => {
                let u = var_ui(var);
                format!("{}: wanted {}, got {}", u.label, bound_text(&x.bound, |n| u.value_text(n)), u.value_text(x.actual))
            }
            Subject::Out(out) => {
                let u = out_ui(out);
                format!("{}: wanted {}, got {}", u.label, bound_text(&x.bound, |n| u.value_text(n)), u.value_text(x.actual))
            }
        })
        .collect()
}

/// "Maximise floor space" with nothing that limits it: the UI says so.
pub fn floor_space_unlimited(req: &Request) -> bool {
    req.objective == Objective::MaxQol && !req.limits_floor_space()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_formatting() {
        assert_eq!(fmt_num(1234567.891, 1), "1,234,567.9");
        assert_eq!(fmt_num(12.0, 0), "12");
        assert_eq!(fmt_num(-1234.6, 0), "-1,235");
        assert_eq!(fmt_num(-0.0001, 1), "0.0");
    }
}
