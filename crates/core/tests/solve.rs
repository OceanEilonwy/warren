use rabbitsim::engine::*;

fn base() -> Params {
    Params { runs: 60, years: 4, ..Params::default() }
}

fn cfg() -> OptConfig {
    OptConfig::default()
}

fn meat(s: &Solution) -> f64 {
    s.summary.metric(MetricKey::MeatKgPerYr).mean
}

fn default_problem(obj: Objective) -> Problem {
    Problem::new(base(), obj).with_var(Var::Qol, Bound::at_least(1.0)).with_var(Var::MaxHeadcount, Bound::at_most(0.0))
}

#[test]
fn unconstrained_max_meat_uses_the_lowest_floor_space() {
    let s = solve(&Problem::new(base(), Objective::MaxMeat), cfg());
    for v in Var::QOL_TYPES {
        assert_eq!(v.get(&s.params), 1.0, "{v:?}");
    }
    assert!(s.feasible());
    assert!(meat(&s) > 170.0, "meat {}", meat(&s));
    assert_eq!(s.objective_value, meat(&s));
}

#[test]
fn qol_lower_bound_is_the_value_used() {
    let p = default_problem(Objective::MaxMeat).with_var(Var::Qol, Bound::at_least(1.4));
    let s = solve(&p, cfg());
    for v in Var::QOL_TYPES {
        assert!((v.get(&s.params) - 1.4).abs() < 1e-9);
    }
    let free = solve(&default_problem(Objective::MaxMeat), cfg());
    assert!(meat(&s) < meat(&free), "{} vs {}", meat(&s), meat(&free));
}

#[test]
fn per_type_constraint_overrides_the_master() {
    let p = default_problem(Objective::MaxMeat).with_var(Var::Qol, Bound::at_least(1.2)).with_var(Var::QolBuck, Bound::at_least(2.0));
    let s = solve(&p, cfg());
    assert!((s.params.qol_buck - 2.0).abs() < 1e-9);
    assert!((s.params.qol_doe - 1.2).abs() < 1e-9);
    assert!((s.params.qol_young - 1.2).abs() < 1e-9);
}

#[test]
fn pinned_levers_are_respected_exactly() {
    let p = default_problem(Objective::MaxMeat)
        .with_var(Var::Does, Bound::exactly(3.0))
        .with_var(Var::Bucks, Bound::exactly(2.0))
        .with_var(Var::WeanDays, Bound::exactly(49.0))
        .with_var(Var::RebreedDays, Bound::exactly(28.0))
        .with_var(Var::ButcherRule, Bound::exactly(0.0))
        .with_var(Var::ButcherDays, Bound::exactly(84.0))
        .with_var(Var::GrowerCageW, Bound::exactly(0.75))
        .with_var(Var::Tiers, Bound::exactly(2.0));
    let s = solve(&p, cfg());
    let q = &s.params;
    assert_eq!((q.does, q.bucks, q.wean_days, q.rebreed_days, q.butcher_days, q.tiers), (3, 2, 49, 28, 84, 2));
    assert_eq!(q.butcher_rule, ButcherRule::Age);
    assert_eq!(q.grower_cage_w, 0.75);
    assert!(s.violations.is_empty(), "{:?}", s.violations);
}

#[test]
fn limits_are_respected_and_free_levers_use_the_room() {
    let p = default_problem(Objective::MaxMeat)
        .with_var(Var::Does, Bound::at_most(3.0))
        .with_var(Var::RebreedDays, Bound::at_least(28.0))
        .with_var(Var::WeanDays, Bound::at_least(42.0))
        .with_var(Var::ButcherDays, Bound::at_most(84.0));
    let s = solve(&p, cfg());
    let q = &s.params;
    assert!(q.does <= 3 && q.does >= 1);
    assert!(q.rebreed_days >= 28);
    assert!(q.wean_days >= 42);
    assert!(q.butcher_days <= 84);
    assert!(s.violations.is_empty(), "{:?}", s.violations);
    let free = solve(&default_problem(Objective::MaxMeat), cfg());
    assert!(meat(&s) < meat(&free));
}

#[test]
fn constraining_never_beats_the_unconstrained_optimum() {
    let free = meat(&solve(&default_problem(Objective::MaxMeat), cfg()));
    for p in [
        default_problem(Objective::MaxMeat).with_var(Var::Does, Bound::exactly(2.0)),
        default_problem(Objective::MaxMeat).with_var(Var::MaxHeadcount, Bound::at_most(30.0)),
        default_problem(Objective::MaxMeat).with_var(Var::Tiers, Bound::exactly(1.0)),
    ] {
        let m = meat(&solve(&p, cfg()));
        assert!(m <= free * 1.03, "{m} vs {free}");
    }
}

#[test]
fn headcount_cap_is_applied_and_costs_meat() {
    let s = solve(&default_problem(Objective::MaxMeat).with_var(Var::MaxHeadcount, Bound::at_most(30.0)), cfg());
    assert_eq!(s.params.max_headcount, 30);
    assert!(s.summary.metric(MetricKey::PeakHead).p90 <= 36.0);
}

#[test]
fn output_constraints_are_met_when_possible() {
    // a cap on the peak number of rabbits, and a minimum number butchered
    let p =
        default_problem(Objective::MaxMeat).with_out(Out::PeakHead, Bound::at_most(50.0)).with_out(Out::FryersPerYr, Bound::at_least(90.0));
    let s = solve(&p, cfg());
    assert!(s.feasible(), "{:?}", s.violations);
    assert!(s.summary.metric(MetricKey::PeakHead).mean <= 50.0);
    assert!(s.summary.metric(MetricKey::FryersPerYr).mean >= 90.0);
    let free = meat(&solve(&default_problem(Objective::MaxMeat), cfg()));
    assert!(meat(&s) < free, "limits should cost something: {} vs {free}", meat(&s));
}

#[test]
fn impossible_output_constraints_are_reported() {
    let p = default_problem(Objective::MaxMeat).with_out(Out::MeatKgPerYr, Bound::at_least(5000.0));
    let s = solve(&p, cfg());
    assert!(!s.feasible());
    assert!(s.violations.iter().any(|v| v.subject == Subject::Out(Out::MeatKgPerYr)));
    assert!(meat(&s) > 150.0, "still returns the closest plan: {}", meat(&s));
}

#[test]
fn pins_that_do_not_fit_are_reported() {
    let p = default_problem(Objective::MaxMeat).with_var(Var::Does, Bound::exactly(20.0));
    let s = solve(&p, cfg());
    assert!(s.violations.iter().any(|v| v.subject == Subject::Var(Var::Does)), "{:?}", s.violations);
}

#[test]
fn max_qol_trades_floor_space_against_a_meat_target() {
    let at = |kg: f64| {
        let p = default_problem(Objective::MaxQol).with_out(Out::MeatKgPerYr, Bound::at_least(kg));
        solve(&p, cfg())
    };
    let easy = at(110.0);
    let hard = at(155.0);
    assert!(easy.feasible() && hard.feasible(), "{:?} {:?}", easy.violations, hard.violations);
    assert!(meat(&easy) >= 110.0 * 0.98 && meat(&hard) >= 155.0 * 0.98);
    assert!(easy.objective_value > hard.objective_value + 0.1, "QoL {} vs {}", easy.objective_value, hard.objective_value);
    assert!(hard.objective_value >= 1.0);
}

#[test]
fn max_qol_without_a_trade_off_goes_as_high_as_the_cages_fit() {
    // in 2 m of width a doe cage stops fitting a little above double the Code minimum
    let s = solve(&default_problem(Objective::MaxQol), cfg());
    assert!(s.objective_value > 1.8 && s.objective_value < 2.2, "{}", s.objective_value);
}

#[test]
fn max_qol_that_cannot_be_met_reports_the_closest_plan() {
    let p = default_problem(Objective::MaxQol).with_out(Out::MeatKgPerYr, Bound::at_least(5000.0));
    let s = solve(&p, cfg());
    assert!(!s.feasible());
    assert!(s.objective_value <= 1.0 + 1e-9);
}

#[test]
fn max_qol_respects_a_pinned_type() {
    let p =
        default_problem(Objective::MaxQol).with_var(Var::QolBuck, Bound::exactly(1.0)).with_out(Out::MeatKgPerYr, Bound::at_least(150.0));
    let s = solve(&p, cfg());
    assert_eq!(s.params.qol_buck, 1.0);
    assert!(s.params.qol_doe > 1.0);
}

#[test]
fn removing_a_binding_constraint_improves_the_objective_and_a_slack_one_does_not() {
    let tight = default_problem(Objective::MaxMeat).with_var(Var::Does, Bound::exactly(2.0));
    let (with, without) = (solve(&tight, cfg()), solve(&tight.without_var(Var::Does), cfg()));
    assert!(meat(&without) > meat(&with) + 20.0);
    let slack = default_problem(Objective::MaxMeat).with_var(Var::Does, Bound::at_most(24.0));
    let (a, b) = (solve(&slack, cfg()), solve(&slack.without_var(Var::Does), cfg()));
    assert!((meat(&a) - meat(&b)).abs() < 12.0, "{} vs {}", meat(&a), meat(&b));
}

#[test]
fn solving_is_deterministic() {
    let p = default_problem(Objective::MaxMeat).with_var(Var::QolDoe, Bound::at_least(1.3));
    let (a, b) = (solve(&p, cfg()), solve(&p, cfg()));
    assert_eq!(a.params, b.params);
    assert_eq!(meat(&a), meat(&b));
}

#[test]
fn unsolvable_space_terminates_for_every_objective() {
    for o in Objective::ALL {
        let p = Problem::new(Params { width_m: 0.5, depth_m: 0.4, runs: 10, ..Params::default() }, o);
        let s = solve(&p, cfg());
        assert!(!s.summary.layout.feasible);
    }
}

#[test]
fn bounds_that_exclude_every_stock_value_still_work() {
    let p =
        default_problem(Objective::MaxMeat).with_var(Var::GrowerCageW, Bound::at_least(2.5)).with_var(Var::WeanDays, Bound::at_most(30.0));
    let s = solve(&p, cfg());
    assert!(s.params.grower_cage_w >= 2.5 && s.params.wean_days <= 30);
}

#[test]
fn an_empty_layout_is_never_feasible() {
    // QoL so high that no doe cage fits in 2 m: the plan produces nothing.
    let p = default_problem(Objective::MaxMeat).with_var(Var::Qol, Bound::at_least(3.0));
    let s = solve(&p, cfg());
    assert!(!s.feasible());
    // and the max-QoL search stops where cages still fit
    let q = solve(&default_problem(Objective::MaxQol), cfg());
    assert!(q.feasible() && q.summary.layout.feasible);
    assert!(q.objective_value > 1.5 && q.objective_value < 3.0, "{}", q.objective_value);
    assert!(meat(&q) > 0.0);
}
