//! Many randomised runs -> mean / P10 / P90, run in parallel across all cores.

use rayon::prelude::*;

use super::layout::{Layout, compute_layout};
use super::params::Params;
use super::sim::{CageSnap, RunResult, effective, simulate_run};

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct Stat {
    pub mean: f64,
    pub p10: f64,
    pub p90: f64,
}

pub fn stat(xs: &[f64]) -> Stat {
    if xs.is_empty() {
        return Stat::default();
    }
    let mut s = xs.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let q = |f: f64| s[((f * (s.len() - 1) as f64).round() as usize).min(s.len() - 1)];
    Stat { mean: xs.iter().sum::<f64>() / xs.len() as f64, p10: q(0.1), p90: q(0.9) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricKey {
    MeatKgPerYr,
    FryersPerYr,
    FryerKgPerYr,
    CullKgPerYr,
    ForcedPerYr,
    PelletKgPerYr,
    HayKgPerYr,
    FeedCostPerYr,
    BuyCostPerYr,
    CostPerKg,
    PeakHead,
    CageUse,
    LittersPerDoeYr,
    ButcherAgeWk,
}

impl MetricKey {
    /// Name used in API responses.
    pub fn key(self) -> &'static str {
        match self {
            MetricKey::MeatKgPerYr => "meat_kg_per_yr",
            MetricKey::FryersPerYr => "fryers_per_yr",
            MetricKey::FryerKgPerYr => "fryer_kg_per_yr",
            MetricKey::CullKgPerYr => "cull_kg_per_yr",
            MetricKey::ForcedPerYr => "forced_per_yr",
            MetricKey::PelletKgPerYr => "pellet_kg_per_yr",
            MetricKey::HayKgPerYr => "hay_kg_per_yr",
            MetricKey::FeedCostPerYr => "feed_cost_per_yr",
            MetricKey::BuyCostPerYr => "buy_cost_per_yr",
            MetricKey::CostPerKg => "cost_per_kg",
            MetricKey::PeakHead => "peak_head",
            MetricKey::CageUse => "cage_use",
            MetricKey::LittersPerDoeYr => "litters_per_doe_yr",
            MetricKey::ButcherAgeWk => "butcher_age_wk",
        }
    }

    pub const ALL: [MetricKey; 14] = [
        MetricKey::MeatKgPerYr,
        MetricKey::FryersPerYr,
        MetricKey::FryerKgPerYr,
        MetricKey::CullKgPerYr,
        MetricKey::ForcedPerYr,
        MetricKey::PelletKgPerYr,
        MetricKey::HayKgPerYr,
        MetricKey::FeedCostPerYr,
        MetricKey::BuyCostPerYr,
        MetricKey::CostPerKg,
        MetricKey::PeakHead,
        MetricKey::CageUse,
        MetricKey::LittersPerDoeYr,
        MetricKey::ButcherAgeWk,
    ];

    pub fn of(self, r: &RunResult) -> f64 {
        match self {
            MetricKey::MeatKgPerYr => r.meat_kg_per_yr,
            MetricKey::FryersPerYr => r.fryers_per_yr,
            MetricKey::FryerKgPerYr => r.fryer_kg_per_yr,
            MetricKey::CullKgPerYr => r.cull_kg_per_yr,
            MetricKey::ForcedPerYr => r.forced_per_yr,
            MetricKey::PelletKgPerYr => r.pellet_kg_per_yr,
            MetricKey::HayKgPerYr => r.hay_kg_per_yr,
            MetricKey::FeedCostPerYr => r.feed_cost_per_yr,
            MetricKey::BuyCostPerYr => r.buy_cost_per_yr,
            MetricKey::CostPerKg => (r.feed_cost_per_yr + r.buy_cost_per_yr) / r.meat_kg_per_yr.max(0.001),
            MetricKey::PeakHead => r.peak_head,
            MetricKey::CageUse => r.cage_use,
            MetricKey::LittersPerDoeYr => r.litters_per_doe_yr,
            MetricKey::ButcherAgeWk => r.butcher_age_wk,
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Series {
    pub mean: Vec<f64>,
    pub p10: Vec<f64>,
    pub p90: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct Summary {
    pub layout: Layout,
    pub eff_wean: u32,
    pub eff_does: usize,
    metrics: [Stat; 14],
    pub head: Series,
    pub growers: Series,
    pub cages: Series,
    pub meat: Series,
    pub cost: Series,
    /// The last day of a typical run (see `typical_snapshot`).
    pub snapshot: Vec<CageSnap>,
    /// The busiest day of a run with a typical peak (see `peak_snapshot`).
    pub peak_snapshot: Vec<CageSnap>,
    pub runs: usize,
}

impl Summary {
    pub fn metric(&self, k: MetricKey) -> Stat {
        self.metrics[MetricKey::ALL.iter().position(|x| *x == k).unwrap()]
    }

    /// Share of butchered rabbits that went before the planned age because cages were full.
    pub fn forced_share(&self) -> f64 {
        // forced culls are already included in the butchered count
        self.metric(MetricKey::ForcedPerYr).mean / self.metric(MetricKey::FryersPerYr).mean.max(1.0)
    }
}

fn series(rs: &[RunResult], pick: impl Fn(&RunResult) -> &Vec<f64>) -> Series {
    let n = pick(&rs[0]).len();
    let mut out = Series::default();
    for w in 0..n {
        let col: Vec<f64> = rs.iter().map(|r| pick(r)[w]).collect();
        let s = stat(&col);
        out.mean.push(s.mean);
        out.p10.push(s.p10);
        out.p90.push(s.p90);
    }
    out
}

/// The last-day cage snapshot of a typical run: the one whose final headcount is the median (lowest seed on
/// ties). Taking run 0 as-is can show an unlucky run, e.g. a one-doe herd that happens to be empty that day.
fn typical_snapshot(rs: &[RunResult]) -> Vec<CageSnap> {
    let head = |r: &RunResult| r.snapshot.iter().map(|c| c.count).sum::<u32>();
    let mut order: Vec<usize> = (0..rs.len()).collect();
    order.sort_by_key(|&i| (head(&rs[i]), i));
    order.get(rs.len() / 2).map_or_else(Vec::new, |&i| rs[i].snapshot.clone())
}

/// The busiest-day snapshot of the run whose peak headcount is the median (lowest seed on ties).
fn peak_snapshot(rs: &[RunResult]) -> Vec<CageSnap> {
    let mut order: Vec<usize> = (0..rs.len()).collect();
    order.sort_by(|&a, &b| rs[a].peak_head.total_cmp(&rs[b].peak_head).then(a.cmp(&b)));
    order.get(rs.len() / 2).map_or_else(Vec::new, |&i| rs[i].peak_snapshot.clone())
}

pub fn simulate(p: &Params) -> Summary {
    simulate_runs(p, p.runs as usize)
}

pub fn simulate_runs(p: &Params, runs: usize) -> Summary {
    let layout = compute_layout(p);
    let eff = effective(p, &layout);
    if !layout.feasible || eff.does == 0 || runs == 0 {
        return Summary {
            layout,
            eff_wean: eff.wean,
            eff_does: eff.does,
            metrics: [Stat::default(); 14],
            head: Series::default(),
            growers: Series::default(),
            cages: Series::default(),
            meat: Series::default(),
            cost: Series::default(),
            snapshot: Vec::new(),
            peak_snapshot: Vec::new(),
            runs: 0,
        };
    }
    let rs: Vec<RunResult> = (0..runs as u64).into_par_iter().map(|i| simulate_run(p, &layout, p.seed + i)).collect();
    let mut metrics = [Stat::default(); 14];
    for (i, k) in MetricKey::ALL.iter().enumerate() {
        metrics[i] = stat(&rs.iter().map(|r| k.of(r)).collect::<Vec<_>>());
    }
    Summary {
        eff_wean: eff.wean,
        eff_does: eff.does,
        metrics,
        head: series(&rs, |r| &r.weekly.head),
        growers: series(&rs, |r| &r.weekly.growers),
        cages: series(&rs, |r| &r.weekly.cages),
        meat: series(&rs, |r| &r.weekly.meat),
        cost: series(&rs, |r| &r.weekly.cost),
        snapshot: typical_snapshot(&rs),
        peak_snapshot: peak_snapshot(&rs),
        runs,
        layout,
    }
}
