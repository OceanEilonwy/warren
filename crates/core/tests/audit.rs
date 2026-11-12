//! Sanity audit of every input the user can touch: slider ranges, every constraint with every
//! operator, and the extremes of every fixed input. Nothing may panic, produce NaN, or silently
//! ignore a constraint.

use rabbitsim::api::meta::{Kind, OUT_UI, Op, VAR_UI, all_ctls, allowed_ops, out_ui, var_ui};
use rabbitsim::engine::*;

fn small() -> Params {
    Params { runs: 16, years: 3, ..Params::default() }
}

fn cfg() -> OptConfig {
    OptConfig { eval_runs: 16, eval_years: 3, rounds: 2 }
}

fn base_problem(obj: Objective) -> Problem {
    Problem::new(small(), obj).with_var(Var::Qol, Bound::at_least(1.0)).with_var(Var::MaxHeadcount, Bound::at_most(0.0))
}

/// Value the plan actually has for an input (what a constraint on it is judged against).
fn actual(sol: &Solution, v: Var) -> f64 {
    match v {
        Var::Does => sol.summary.eff_does as f64,
        Var::Bucks => sol.summary.layout.n_buck as f64,
        Var::Tiers => sol.summary.layout.tiers as f64,
        v => v.get(&sol.params),
    }
}

fn metrics_are_sane(s: &Summary, what: &str) {
    for k in MetricKey::ALL {
        let m = s.metric(k);
        assert!(m.mean.is_finite() && m.p10.is_finite() && m.p90.is_finite(), "{what}: {k:?} not finite");
        assert!(m.mean >= -1e-9, "{what}: {k:?} negative: {}", m.mean);
    }
    assert!(s.metric(MetricKey::CageUse).mean <= 1.0 + 1e-9, "{what}: cage use over 100%");
    for series in [&s.head, &s.growers, &s.cages, &s.meat, &s.cost] {
        assert!(series.mean.iter().all(|x| x.is_finite() && *x >= -1e-9), "{what}: weekly series not sane");
    }
}

// ---------------------------------------------------------------- slider ranges

#[test]
fn every_slider_has_a_sane_range_and_reachable_ends() {
    let mut checked = 0;
    let reachable = |min: f64, max: f64, step: f64, what: &str| {
        assert!(min < max && step > 0.0, "{what}: bad range {min}..{max} step {step}");
        let n = (max - min) / step;
        assert!((n - n.round()).abs() < 1e-6, "{what}: {min}..{max} is not a whole number of {step} steps (max unreachable)");
    };
    for c in all_ctls() {
        if let Kind::Range { min, max, step } = c.kind {
            reachable(min, max, step, c.key);
            checked += 1;
            let d = (c.get)(&Params::default());
            assert!(d >= min && d <= max, "{}: default {d} outside {min}..{max}", c.key);
            let off = (d - min) / step;
            assert!((off - off.round()).abs() < 1e-6, "{}: default {d} is not on the slider grid ({min} + n×{step})", c.key);
            for v in [min, d, max] {
                let t = c.value_text(v);
                assert!(!t.is_empty() && !t.contains("NaN"), "{}: bad label {t:?}", c.key);
            }
        }
    }
    for u in VAR_UI.iter() {
        if let Some((min, max, step)) = u.range() {
            reachable(min, max, step, u.var.key());
            checked += 1;
            for v in [min, max] {
                let t = u.value_text(v);
                assert!(!t.is_empty() && !t.contains("NaN"), "{}: bad label {t:?}", u.var.key());
            }
        }
    }
    for u in OUT_UI.iter() {
        reachable(u.min, u.max, u.step, u.out.key());
        checked += 1;
        assert!(!u.value_text(u.min).is_empty() && !u.value_text(u.max).contains("NaN"));
    }
    assert!(checked > 40);
}

// ---------------------------------------------------------------- fixed inputs

#[test]
fn extremes_of_every_fixed_input_simulate_cleanly() {
    for c in all_ctls() {
        let candidates: Vec<f64> = match c.kind {
            Kind::Range { min, max, .. } => vec![min, max],
            Kind::Bool => vec![0.0, 1.0],
            Kind::Choice(cs) => cs.iter().map(|(v, _)| *v).collect(),
        };
        for v in candidates {
            let mut p = small();
            (c.set)(&mut p, v);
            let what = format!("{} = {v}", c.key);
            metrics_are_sane(&simulate(&p), &what);
        }
    }
}

/// A named change to the base parameters.
type Tweak = (&'static str, Box<dyn Fn(&mut Params)>);

#[test]
fn the_solver_survives_extreme_spaces_and_assumptions() {
    let extremes: Vec<Tweak> = vec![
        (
            "tiny space",
            Box::new(|p| {
                p.width_m = 0.5;
                p.depth_m = 0.4;
                p.height_m = 0.4
            }),
        ),
        (
            "huge space",
            Box::new(|p| {
                p.width_m = 6.0;
                p.depth_m = 3.0;
                p.height_m = 3.0
            }),
        ),
        ("one side only", Box::new(|p| p.access_both_sides = false)),
        ("tall cages", Box::new(|p| p.cage_height_cm = 90.0)),
        ("shallow reach", Box::new(|p| p.max_reach_m = 0.4)),
        ("tiny litters", Box::new(|p| p.litter_mean = 4.0)),
        (
            "harsh mortality",
            Box::new(|p| {
                p.pre_wean_mortality = 0.4;
                p.post_wean_mortality = 0.2;
                p.breeder_mortality = 0.3
            }),
        ),
        ("poor conception", Box::new(|p| p.conception_rate = 0.4)),
        (
            "buy replacements dear",
            Box::new(|p| {
                p.replacement = Replacement::Buy;
                p.breeder_price = 150.0
            }),
        ),
        ("late breeding", Box::new(|p| p.breed_age_days = 240)),
        ("short working life", Box::new(|p| p.breeder_life_years = 1.0)),
        (
            "long warm-up",
            Box::new(|p| {
                p.warmup_days = 730;
                p.years = 2
            }),
        ),
    ];
    for obj in Objective::ALL {
        for (name, f) in &extremes {
            let mut p = small();
            f(&mut p);
            let sol = solve(&Problem::new(p, obj).with_var(Var::Qol, Bound::at_least(1.0)), cfg());
            metrics_are_sane(&sol.summary, &format!("{name} / {obj:?}"));
            assert!(sol.objective_value.is_finite(), "{name} / {obj:?}");
        }
    }
}

// ---------------------------------------------------------------- constraints

/// A bound for `var` at a fraction of its slider range with the given operator.
fn bound_at(var: Var, op: Op, frac: f64) -> Bound {
    let (min, max, step) = var_ui(var).range().unwrap_or((0.0, 1.0, 1.0));
    let raw = min + (max - min) * frac;
    let v = (min + ((raw - min) / step).round() * step).clamp(min, max);
    op.bound(v)
}

#[test]
fn every_input_constraint_is_honoured_or_reported_under_max_meat() {
    let mut cases = 0;
    for u in VAR_UI.iter() {
        if u.var == Var::MaxHeadcount {
            continue; // checked separately: a cap has its own semantics
        }
        let ops = allowed_ops(u.var);
        for op in ops {
            for frac in [0.0, 0.5, 1.0] {
                let b = bound_at(u.var, *op, frac);
                let sol = solve(&base_problem(Objective::MaxMeat).with_var(u.var, b), cfg());
                metrics_are_sane(&sol.summary, &format!("{:?} {:?}", u.var, b));
                cases += 1;
                if sol.feasible() {
                    // a plan reported as meeting the constraints must really meet them
                    let members: Vec<Var> = if u.var == Var::Qol { Var::QOL_TYPES.to_vec() } else { vec![u.var] };
                    for m in members {
                        let a = actual(&sol, m);
                        assert!(b.contains(a), "{:?} {:?}: plan has {a}", u.var, b);
                    }
                    if matches!(op, Op::Exactly) && u.var != Var::Qol {
                        assert!((actual(&sol, u.var) - b.exact().unwrap()).abs() < 1e-6, "{:?} pinned but plan differs", u.var);
                    }
                } else {
                    // never a silent failure: it is either reported or the herd cannot fit at all
                    assert!(!sol.violations.is_empty() || !sol.fits, "{:?} {:?}: infeasible but nothing reported", u.var, b);
                }
            }
        }
    }
    assert!(cases > 60);
}

#[test]
fn input_constraints_also_hold_under_max_floor_space() {
    for u in VAR_UI.iter().filter(|u| !matches!(u.var, Var::Qol | Var::MaxHeadcount)) {
        for op in allowed_ops(u.var) {
            let b = bound_at(u.var, *op, 0.5);
            let sol = solve(&base_problem(Objective::MaxQol).with_var(u.var, b).with_out(Out::MeatKgPerYr, Bound::at_least(100.0)), cfg());
            metrics_are_sane(&sol.summary, &format!("{:?} {:?} / max floor space", u.var, b));
            if sol.feasible() {
                assert!(b.contains(actual(&sol, u.var)), "{:?} {:?}: plan has {}", u.var, b, actual(&sol, u.var));
                assert!(sol.summary.metric(MetricKey::MeatKgPerYr).mean >= 100.0 - 1e-9);
            }
        }
    }
}

#[test]
fn every_result_constraint_is_honoured_or_reported() {
    for u in OUT_UI.iter() {
        for op in [Op::AtLeast, Op::AtMost] {
            for frac in [0.05, 0.2, 0.5] {
                let v = (u.min + (u.max - u.min) * frac / u.step).round() * u.step;
                let b = op.bound(v.clamp(u.min, u.max));
                for obj in Objective::ALL {
                    let sol = solve(&base_problem(obj).with_out(u.out, b), cfg());
                    metrics_are_sane(&sol.summary, &format!("{:?} {:?} / {obj:?}", u.out, b));
                    let got = u.out.of(&sol.summary);
                    if sol.feasible() {
                        assert!(b.contains(got), "{:?} {:?} / {obj:?}: plan has {got}", u.out, b);
                    } else {
                        assert!(!sol.violations.is_empty() || !sol.fits, "{:?} {:?}: infeasible but silent", u.out, b);
                    }
                }
            }
        }
    }
}

#[test]
fn the_herd_cap_limits_the_herd_and_a_minimum_herd_is_a_result() {
    for cap in [20.0, 35.0, 60.0] {
        let sol = solve(&base_problem(Objective::MaxMeat).with_var(Var::MaxHeadcount, Bound::at_most(cap)), cfg());
        assert_eq!(sol.params.max_headcount, cap as u32);
        assert!(
            sol.summary.metric(MetricKey::PeakHead).mean <= cap + 3.0,
            "cap {cap}: peak {}",
            sol.summary.metric(MetricKey::PeakHead).mean
        );
    }
    // "at least" on the cap is not a thing: asking for a bigger herd is the peak-rabbits result
    let sol = solve(&base_problem(Objective::MaxQol).with_out(Out::PeakHead, Bound::at_least(60.0)), cfg());
    assert!(sol.feasible(), "{:?}", sol.violations);
    assert!(sol.summary.metric(MetricKey::PeakHead).mean >= 60.0);
    let free = solve(&base_problem(Objective::MaxQol), cfg());
    assert!(
        sol.objective_value < free.objective_value,
        "a bigger herd leaves less room: {} vs {}",
        sol.objective_value,
        free.objective_value
    );
}

#[test]
fn contradictory_constraints_are_reported_not_hidden() {
    let sol = solve(
        &base_problem(Objective::MaxMeat).with_var(Var::Does, Bound::at_most(2.0)).with_out(Out::MeatKgPerYr, Bound::at_least(400.0)),
        cfg(),
    );
    assert!(!sol.feasible());
    assert!(sol.violations.iter().any(|v| v.subject == Subject::Out(Out::MeatKgPerYr)));
    // at least > at most on one input
    let b = Bound { min: Some(10.0), max: Some(3.0) };
    let sol = solve(&base_problem(Objective::MaxMeat).with_var(Var::Does, b), cfg());
    metrics_are_sane(&sol.summary, "contradictory bound");
}

#[test]
fn labels_and_help_are_present_for_every_constraint() {
    for u in VAR_UI.iter() {
        assert!(!u.label.is_empty() && !u.group.is_empty());
    }
    for u in OUT_UI.iter() {
        assert!(!u.label.is_empty() && out_ui(u.out).out == u.out);
    }
    for c in all_ctls() {
        assert!(!c.label.is_empty() && !c.key.is_empty());
    }
}
