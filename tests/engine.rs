use rabbitsim::engine::*;

fn base() -> Params {
    Params { tiers: 2, runs: 60, years: 4, ..Params::default() }
}

fn meat(p: &Params) -> f64 {
    simulate(p).metric(MetricKey::MeatKgPerYr).mean
}

#[test]
fn runs_are_reproducible_for_a_seed_and_differ_between_seeds() {
    let p = base();
    let a = simulate(&p);
    let b = simulate(&p);
    assert_eq!(a.metric(MetricKey::MeatKgPerYr), b.metric(MetricKey::MeatKgPerYr));
    assert_eq!(a.meat.mean, b.meat.mean);
    let c = simulate(&Params { seed: 99, ..p });
    assert_ne!(a.metric(MetricKey::MeatKgPerYr).mean, c.metric(MetricKey::MeatKgPerYr).mean);
}

#[test]
fn defaults_produce_plausible_output() {
    let s = simulate(&base());
    let m = s.metric(MetricKey::MeatKgPerYr);
    assert!(m.mean > 100.0 && m.mean < 600.0, "meat {}", m.mean);
    assert!(m.p10 <= m.mean && m.mean <= m.p90);
    let litters = s.metric(MetricKey::LittersPerDoeYr).mean;
    assert!((3.0..=12.0).contains(&litters), "litters/doe/yr {litters}");
    let age = s.metric(MetricKey::ButcherAgeWk).mean;
    assert!((8.0..=14.0).contains(&age), "butcher age {age}");
    let cost = s.metric(MetricKey::CostPerKg).mean;
    assert!((3.0..=30.0).contains(&cost), "$/kg {cost}");
    assert!(s.metric(MetricKey::PelletKgPerYr).mean > s.metric(MetricKey::HayKgPerYr).mean);
}

#[test]
fn weekly_series_cover_every_week() {
    let p = base();
    let s = simulate(&p);
    let weeks = (p.years as usize * 365 / 7).max(1);
    for series in [&s.head, &s.growers, &s.cages, &s.meat, &s.cost] {
        assert_eq!(series.mean.len(), weeks);
        assert_eq!(series.p10.len(), weeks);
        assert!(series.p10.iter().zip(&series.p90).all(|(a, b)| a <= b));
        assert!(series.mean.iter().all(|x| x.is_finite() && *x >= 0.0));
    }
}

#[test]
fn cages_in_use_never_exceed_cages_available() {
    let s = simulate(&base());
    let n = s.layout.n_cages() as f64;
    assert!(s.cages.p90.iter().all(|&c| c <= n + 1e-9));
    assert!(s.metric(MetricKey::CageUse).mean <= 1.0);
}

#[test]
fn snapshot_matches_layout_and_respects_cage_areas() {
    let p = base();
    let s = simulate(&p);
    assert_eq!(s.snapshot.len(), s.layout.n_cages());
    for (snap, cage) in s.snapshot.iter().zip(&s.layout.cages) {
        match snap.role {
            CageRole::Doe | CageRole::DoeLitter => assert_eq!(cage.kind, CageKind::Doe),
            CageRole::Buck => assert_eq!(cage.kind, CageKind::Buck),
            CageRole::Growers => {
                // never more growers than the Code area allows (youngest rule: 0.07 m² each)
                let max = (cage.area / (code::GROWER_5_TO_12WK * p.qol_young)).floor() as u32;
                assert!(snap.count <= max, "{} growers in {:.2} m²", snap.count, cage.area);
            }
            CageRole::Replacement => assert_eq!(snap.count, 1),
            CageRole::Empty => assert_eq!(snap.count, 0),
        }
    }
}

#[test]
fn growers_always_meet_code_area_over_many_seeds() {
    // The simulator packs by area every day; verify against the final snapshot of many runs.
    let p = Params { years: 3, ..base() };
    let l = compute_layout(&p);
    for seed in 1..40 {
        let r = simulate_run(&p, &l, seed);
        for (snap, cage) in r.snapshot.iter().zip(&l.cages) {
            if snap.role == CageRole::Growers {
                assert!(snap.count as f64 * code::GROWER_5_TO_12WK <= cage.area + 1e-9);
            }
        }
    }
}

#[test]
fn infeasible_space_gives_empty_summary_not_a_panic() {
    let s = simulate(&Params { width_m: 0.5, depth_m: 0.4, ..Params::default() });
    assert!(!s.layout.feasible);
    assert_eq!(s.metric(MetricKey::MeatKgPerYr).mean, 0.0);
    assert!(s.snapshot.is_empty());
    let s = simulate(&Params { runs: 0, ..Params::default() });
    assert_eq!(s.metric(MetricKey::MeatKgPerYr).mean, 0.0);
}

#[test]
fn more_floor_space_per_rabbit_never_helps_meat() {
    let lo = Params { does: 3, ..base() };
    let hi = Params { does: 3, qol_doe: 1.6, qol_buck: 1.6, qol_young: 1.6, qol_old: 1.6, ..base() };
    assert!(meat(&hi) <= meat(&lo) * 1.02, "{} vs {}", meat(&hi), meat(&lo));
}

#[test]
fn more_does_in_a_big_space_gives_more_meat() {
    let big = Params { width_m: 4.0, depth_m: 1.5, height_m: 1.35, tiers: 2, runs: 60, ..Params::default() };
    let few = meat(&Params { does: 2, ..big.clone() });
    let many = meat(&Params { does: 8, bucks: 2, ..big });
    assert!(many > few * 1.5, "{many} vs {few}");
}

#[test]
fn feed_cost_scales_with_price() {
    let a = simulate(&base()).metric(MetricKey::FeedCostPerYr).mean;
    let b = simulate(&Params { pellet_price: 3.0, hay_price: 1.6, ..base() }).metric(MetricKey::FeedCostPerYr).mean;
    assert!((b / a - 2.0).abs() < 0.02, "ratio {}", b / a);
}

#[test]
fn headcount_cap_is_respected_and_forces_early_butchering() {
    let capped = simulate(&Params { max_headcount: 25, butcher_rule: ButcherRule::Room, ..base() });
    assert!(capped.metric(MetricKey::PeakHead).p90 <= 25.0 + 6.0, "peak {}", capped.metric(MetricKey::PeakHead).p90);
    let free = simulate(&Params { butcher_rule: ButcherRule::Room, ..base() });
    assert!(capped.metric(MetricKey::MeatKgPerYr).mean < free.metric(MetricKey::MeatKgPerYr).mean);
}

#[test]
fn buying_replacements_costs_money_keeping_own_does_not() {
    let own = simulate(&base()).metric(MetricKey::BuyCostPerYr).mean;
    let buy = simulate(&Params { replacement: Replacement::Buy, ..base() }).metric(MetricKey::BuyCostPerYr).mean;
    assert!(buy > own);
}

#[test]
fn one_sided_access_loses_most_of_the_output() {
    let both = meat(&Params { does: 3, ..base() });
    let one = meat(&Params { does: 3, access_both_sides: false, ..base() });
    assert!(one < both * 0.7, "{one} vs {both}");
}

#[test]
fn optimiser_beats_the_defaults_and_stays_within_the_space() {
    let start = Params { runs: 60, ..Params::default() };
    let best = optimise(&start, OptConfig::default());
    let l = compute_layout(&best);
    assert!(l.feasible);
    assert!(best.tiers as usize <= l.max_tiers.max(1));
    let (a, b) = (meat(&start), meat(&best));
    assert!(b > a, "optimised {b} vs defaults {a}");
    let s = simulate(&best);
    assert!(s.forced_share() < 0.06, "forced share {}", s.forced_share());
    // space and welfare settings are never touched
    assert_eq!(best.width_m, start.width_m);
    assert_eq!(best.qol_doe, start.qol_doe);
    assert_eq!(best.pellet_price, start.pellet_price);
}

#[test]
fn optimiser_respects_the_headcount_cap() {
    let start = Params { max_headcount: 30, runs: 60, ..Params::default() };
    let best = optimise(&start, OptConfig::default());
    assert_eq!(best.max_headcount, 30);
    let s = simulate(&best);
    assert!(s.metric(MetricKey::PeakHead).p90 <= 30.0 + 6.0, "peak {}", s.metric(MetricKey::PeakHead).p90);
}

#[test]
fn optimised_meat_falls_as_welfare_rises() {
    let at = |q: f64| {
        let mut p = Params { runs: 60, ..Params::default() };
        p.set_all_qol(q);
        meat(&optimise(&p, OptConfig::default()))
    };
    let (m1, m15, m2) = (at(1.0), at(1.5), at(2.0));
    assert!(m1 > m15 && m15 > m2, "{m1} {m15} {m2}");
}

#[test]
fn optimiser_terminates_on_an_infeasible_space() {
    let start = Params { width_m: 0.5, depth_m: 0.4, runs: 10, ..Params::default() };
    let _ = optimise(&start, OptConfig::default());
}

#[test]
fn a_tight_cap_stops_matings_rather_than_overshooting() {
    for cap in [20u32, 30, 45] {
        let s = simulate(&Params { max_headcount: cap, butcher_rule: ButcherRule::Room, ..base() });
        let peak = s.metric(MetricKey::PeakHead);
        assert!(peak.mean <= cap as f64 + 2.0, "cap {cap}: mean peak {}", peak.mean);
        assert!(peak.p90 <= cap as f64 + 6.0, "cap {cap}: p90 peak {}", peak.p90);
        assert!(s.metric(MetricKey::MeatKgPerYr).mean > 0.0);
    }
}

#[test]
fn dressed_weight_scales_meat_and_feed_and_the_default_is_unchanged() {
    let at = |kg: f64| simulate(&Params { dressed_weight_kg: kg, ..base() });
    let (light, normal, heavy) = (at(0.8), at(1.0), at(1.25));
    assert_eq!(normal.metric(MetricKey::MeatKgPerYr), simulate(&base()).metric(MetricKey::MeatKgPerYr), "1.0 kg is the default");
    let m = |s: &Summary| s.metric(MetricKey::MeatKgPerYr).mean;
    let f = |s: &Summary| s.metric(MetricKey::PelletKgPerYr).mean;
    assert!(m(&heavy) > m(&normal) * 1.15 && m(&heavy) < m(&normal) * 1.35, "{} vs {}", m(&heavy), m(&normal));
    assert!(m(&light) < m(&normal) * 0.9 && m(&light) > m(&normal) * 0.7, "{} vs {}", m(&light), m(&normal));
    // feed follows metabolic weight (kg^0.75): it grows, but by less than the meat does
    assert!(f(&heavy) > f(&normal) * 1.08 && f(&heavy) < m(&heavy) / m(&normal) * f(&normal), "{} vs {}", f(&heavy), f(&normal));
    // so heavier rabbits are cheaper to feed per kg of meat
    let c = |s: &Summary| s.metric(MetricKey::CostPerKg).mean;
    assert!(c(&heavy) < c(&normal) && c(&normal) < c(&light));
}

#[test]
fn dressed_weight_input_is_the_average_dressed_weight_of_a_12_week_rabbit() {
    // plenty of room, butcher everyone at exactly 12 weeks: the average carcass is what was asked for
    let roomy = Params {
        width_m: 6.0,
        depth_m: 3.0,
        height_m: 2.0,
        does: 4,
        butcher_rule: ButcherRule::Age,
        butcher_days: 84,
        runs: 80,
        years: 4,
        ..Params::default()
    };
    for kg in [1.1, 1.43, 1.9] {
        let s = simulate(&Params { dressed_weight_kg: kg, ..roomy.clone() });
        assert!(s.metric(MetricKey::ForcedPerYr).mean < 1.0, "needs no early culls to be a fair test");
        let avg = s.metric(MetricKey::FryerKgPerYr).mean / s.metric(MetricKey::FryersPerYr).mean;
        assert!((avg / kg - 1.0).abs() < 0.03, "asked for {kg} kg, average dressed {avg:.3} kg");
    }
}

#[test]
fn a_long_start_up_still_leaves_a_full_year_to_count() {
    // 2 years with the longest start-up used to annualise a single day (P10/P90 of 0)
    let s = simulate(&Params { years: 2, warmup_days: 728, runs: 60, ..Params::default() });
    let m = s.metric(MetricKey::MeatKgPerYr);
    assert!(m.p10 > 0.0 && m.p90 < 2.0 * m.mean, "meat {m:?}");
}

#[test]
fn rabbits_butchered_includes_the_ones_butchered_early() {
    // a tight cap with the age rule forces early culls; they are butchered rabbits too
    let s = simulate(&Params { max_headcount: 20, runs: 60, ..Params::default() });
    let (n, forced, kg) =
        (s.metric(MetricKey::FryersPerYr).mean, s.metric(MetricKey::ForcedPerYr).mean, s.metric(MetricKey::FryerKgPerYr).mean);
    assert!(forced <= n, "forced {forced} is a subset of butchered {n}");
    assert!(kg / n < 1.2, "kg per butchered rabbit {:.2} must count every rabbit in the kg", kg / n);
    assert!((s.forced_share() - forced / n).abs() < 1e-9);
}

#[test]
fn nursing_kit_feed_follows_the_herd_size() {
    // with does and a full grower stock the feed ratio tracks (kg ratio)^0.75 for every animal, kits included
    let p = Params { runs: 60, years: 3, ..Params::default() };
    let small = simulate(&Params { dressed_weight_kg: 1.0, ..p.clone() });
    let big = simulate(&Params { dressed_weight_kg: 2.0, ..p.clone() });
    let ratio = big.metric(MetricKey::PelletKgPerYr).mean / small.metric(MetricKey::PelletKgPerYr).mean;
    let meat_ratio = big.metric(MetricKey::MeatKgPerYr).mean / small.metric(MetricKey::MeatKgPerYr).mean;
    // same herd, so heads match closely; feed should rise by about 2^0.75 = 1.68
    assert!((ratio / 2f64.powf(0.75) - 1.0).abs() < 0.08, "pellet ratio {ratio:.3}, meat ratio {meat_ratio:.3}");
}

#[test]
fn breeders_retire_on_time_even_when_bred_back_to_back() {
    // 1-year working life, no deaths, 11-day re-mating: does used to be bred on until a mating failed
    let p = Params {
        runs: 100,
        years: 6,
        does: 4,
        bucks: 1,
        rebreed_days: 11,
        wean_days: 35,
        butcher_rule: ButcherRule::Room,
        butcher_days: 140,
        breeder_life_years: 1.0,
        breeder_mortality: 0.0,
        ..Params::default()
    };
    let s = simulate(&p);
    let scale = p.dressed_weight_kg / bio::reference_dressed_kg();
    let carcass = bio::live_weight(365.0, scale) * bio::dressing(365.0);
    let retired_per_yr = s.metric(MetricKey::CullKgPerYr).mean / carcass;
    assert!(retired_per_yr > 0.9 * 5.0, "5 breeders on a 1-year life should retire ~5 a year, got {retired_per_yr:.2}");
}

#[test]
fn replacements_are_reared_ahead_so_retirement_costs_little() {
    // bought-in replacements, 1-year life, no deaths: picking a replacement only once a breeder had gone
    // left every doe cage empty for ~100 days a year
    let p = Params {
        runs: 100,
        years: 6,
        does: 4,
        rebreed_days: 11,
        wean_days: 35,
        butcher_rule: ButcherRule::Room,
        butcher_days: 140,
        grower_cage_w: 1.25,
        breeder_mortality: 0.0,
        replacement: Replacement::Buy,
        ..Params::default()
    };
    let short = simulate(&Params { breeder_life_years: 1.0, ..p.clone() }).metric(MetricKey::MeatKgPerYr).mean;
    let long = simulate(&Params { breeder_life_years: 5.0, ..p.clone() }).metric(MetricKey::MeatKgPerYr).mean;
    assert!(short > 0.9 * long, "1-year life {short:.1} vs 5-year {long:.1}");
}

#[test]
fn nursing_conception_only_applies_to_does_mated_while_nursing() {
    // re-mating after weaning: nobody is nursing at mating, so the nursing rate changes nothing
    let late = Params { runs: 40, rebreed_days: 56, wean_days: 42, ..Params::default() };
    let a = simulate(&Params { nursing_conception_rate: 0.3, ..late.clone() });
    let b = simulate(&Params { nursing_conception_rate: 0.9, ..late.clone() });
    assert_eq!(a.metric(MetricKey::MeatKgPerYr), b.metric(MetricKey::MeatKgPerYr));
    // early re-mating: a lower nursing rate means fewer litters
    let early = Params { runs: 40, rebreed_days: 11, wean_days: 35, ..Params::default() };
    let lo = simulate(&Params { nursing_conception_rate: 0.3, ..early.clone() }).metric(MetricKey::LittersPerDoeYr).mean;
    let hi = simulate(&Params { nursing_conception_rate: 0.9, ..early.clone() }).metric(MetricKey::LittersPerDoeYr).mean;
    assert!(lo < 0.85 * hi, "litters/doe {lo:.2} vs {hi:.2}");
}

#[test]
fn growers_kept_longer_face_the_same_daily_risk() {
    // the room rule with a late latest age used to spread the 6-12 week loss thinly over every extra week
    let p = Params { runs: 100, does: 5, rebreed_days: 11, wean_days: 35, butcher_rule: ButcherRule::Room, ..Params::default() };
    let no_loss =
        |bd: u32| simulate(&Params { butcher_days: bd, post_wean_mortality: 0.0, ..p.clone() }).metric(MetricKey::MeatKgPerYr).mean;
    let loss = |bd: u32| simulate(&Params { butcher_days: bd, post_wean_mortality: 0.1, ..p.clone() }).metric(MetricKey::MeatKgPerYr).mean;
    let cost_short = 1.0 - loss(84) / no_loss(84);
    let cost_long = 1.0 - loss(140) / no_loss(140);
    assert!(cost_long > 0.8 * cost_short, "loss costs {:.1}% at 84 d but only {:.1}% at 140 d", cost_short * 100.0, cost_long * 100.0);
}
