//! The solver: choose every free optimisable input to best meet the objective while respecting the
//! constraints. Pinned inputs are left alone; free levers are searched by coordinate search (every
//! candidate in a step scored in parallel). Quality of life is handled separately: it sits at its
//! lowest allowed value unless the objective is to maximise it, in which case the largest feasible
//! value is found by bisection.

use rayon::prelude::*;

use super::constraints::{Bound, Objective, Problem, Solution, Subject, Var, Violation};
use super::layout::compute_layout;
use super::params::Params;
use super::progress::Progress;
use super::summary::{MetricKey, Summary, simulate_runs_tracked};

/// Plans where more than this share of rabbits are butchered early for lack of room are rejected.
pub const MAX_FORCED_SHARE: f64 = 0.03;
const QOL_STEP: f64 = 0.05;
const QOL_MAX: f64 = 3.0;

#[derive(Clone, Copy, Debug)]
pub struct OptConfig {
    /// Randomised runs per candidate.
    pub eval_runs: usize,
    /// Max years simulated per candidate.
    pub eval_years: u32,
    /// Full search rounds (stops early when a round finds nothing better).
    pub rounds: usize,
}

impl Default for OptConfig {
    fn default() -> Self {
        OptConfig { eval_runs: 48, eval_years: 5, rounds: 3 }
    }
}

/// Lexicographic: first fewest rule violations, then best objective.
#[derive(Clone, Copy, Debug)]
struct Score {
    violation: f64,
    objective: f64,
}

fn better(a: Score, b: Score, eps: f64) -> bool {
    if (a.violation - b.violation).abs() > 1e-9 {
        return a.violation < b.violation;
    }
    a.objective > b.objective + eps
}

/// Larger than any other violation, so a plan that houses no herd never beats one that does.
const NO_HERD_PENALTY: f64 = 1000.0;

/// A plan with no doe or buck cage produces nothing, so it must never look "feasible" (or cheap).
fn has_herd(s: &Summary) -> bool {
    s.layout.feasible && s.eff_does > 0
}

/// Candidates are judged on a small sample, so the search aims a little under `MAX_FORCED_SHARE`; otherwise
/// a plan that just passes on the sample can come out just over the limit in the full report.
const FORCED_SEARCH_TARGET: f64 = 0.025;

fn forced_excess(s: &Summary) -> f64 {
    (s.forced_share() - FORCED_SEARCH_TARGET).max(0.0) * 10.0 + if has_herd(s) { 0.0 } else { NO_HERD_PENALTY }
}

/// Candidates are scored on short, small samples, so aim a little inside each result limit; the final
/// report re-checks against the exact limit with the full number of runs.
const SEARCH_MARGIN: f64 = 0.03;

fn tightened(b: &Bound) -> Bound {
    if b.exact().is_some() {
        return *b;
    }
    Bound { min: b.min.map(|m| m * (1.0 + SEARCH_MARGIN)), max: b.max.map(|m| m * (1.0 - SEARCH_MARGIN)) }
}

fn out_violation(problem: &Problem, s: &Summary) -> f64 {
    problem.outs.iter().map(|(o, b)| tightened(b).violation(o.of(s))).sum()
}

/// Levers are always searched for the most meat; floor space is what the "maximise floor space"
/// objective adds on top (see `solve_max_qol`).
fn evaluate(p: &Params, problem: &Problem, cfg: &OptConfig, prog: &Progress) -> Score {
    let mut q = p.clone();
    q.years = q.years.min(cfg.eval_years);
    let s = simulate_runs_tracked(&q, cfg.eval_runs, prog);
    Score { violation: out_violation(problem, &s) + forced_excess(&s), objective: s.metric(MetricKey::MeatKgPerYr).mean }
}

// ---------------- quality of life & headcount ----------------

/// Per-type QoL for a given scalar `s` (None = lowest allowed): each type takes `s` clamped to its bounds.
fn qol_values(problem: &Problem, s: Option<f64>) -> [f64; 4] {
    let mut out = [1.0; 4];
    for (i, v) in Var::QOL_TYPES.iter().enumerate() {
        let b = problem.effective_bound(*v).unwrap_or_default();
        let lo = b.min.unwrap_or(1.0).max(1.0);
        let hi = b.max.unwrap_or(QOL_MAX).max(lo);
        out[i] = match (b.exact(), s) {
            (Some(x), _) => x,
            (None, None) => lo,
            (None, Some(s)) => s.clamp(lo, hi),
        };
    }
    out
}

fn with_welfare(problem: &Problem, qol: [f64; 4]) -> Params {
    let mut p = problem.base.clone();
    for (v, x) in Var::QOL_TYPES.iter().zip(qol) {
        v.set(&mut p, x);
    }
    let cap = problem.var_bound(Var::MaxHeadcount).map_or(0.0, |b| b.exact().or(b.max).unwrap_or(0.0));
    Var::MaxHeadcount.set(&mut p, cap);
    p
}

// ---------------- lever search ----------------

fn raw_domain(v: Var, max_tiers: u32) -> Vec<f64> {
    match v {
        Var::Tiers => (1..=max_tiers.max(1)).map(f64::from).collect(),
        Var::GrowerCageW => vec![0.4, 0.5, 0.6, 0.75, 0.9, 1.0, 1.25, 1.5, 2.0],
        Var::Does => (1..=24).map(f64::from).collect(),
        Var::Bucks => vec![1.0, 2.0, 3.0],
        Var::WeanDays => vec![28.0, 35.0, 42.0, 49.0, 56.0],
        Var::RebreedDays => vec![11.0, 14.0, 21.0, 28.0, 35.0, 42.0, 56.0],
        Var::ButcherDays => vec![63.0, 70.0, 77.0, 84.0, 91.0, 98.0, 112.0, 126.0, 140.0],
        Var::ButcherRule => vec![0.0, 1.0],
        _ => Vec::new(),
    }
}

/// Candidate values for a lever that satisfy its constraint (pinned levers have exactly one).
fn domain(problem: &Problem, v: Var, max_tiers: u32) -> Vec<f64> {
    let Some(b) = problem.var_bound(v) else { return raw_domain(v, max_tiers) };
    if let Some(x) = b.exact() {
        return vec![x];
    }
    let d: Vec<f64> = raw_domain(v, max_tiers).into_iter().filter(|x| b.contains(*x)).collect();
    if !d.is_empty() {
        return d;
    }
    // bounds exclude every stock candidate: use the limit itself
    vec![b.min.or(b.max).unwrap_or(0.0)]
}

fn nearest(d: &[f64], target: f64) -> f64 {
    d.iter().copied().min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs())).unwrap_or(target)
}

/// Where a lever search starts (defaults where allowed, as many tiers as fit), and the tier limit.
fn search_start(problem: &Problem, welfare: Params) -> (Params, u32) {
    let max_tiers = compute_layout(&Params { tiers: 99, ..welfare.clone() }).max_tiers.max(1) as u32;
    let dom = |v: Var| domain(problem, v, max_tiers);
    let mut best = welfare;
    let defaults = Params::default();
    for v in [Var::GrowerCageW, Var::Does, Var::Bucks, Var::WeanDays, Var::RebreedDays, Var::ButcherDays, Var::ButcherRule] {
        v.set(&mut best, nearest(&dom(v), v.get(&defaults)));
    }
    Var::Tiers.set(&mut best, dom(Var::Tiers).last().copied().unwrap_or(1.0));
    (best, max_tiers)
}

/// Does that fit beside `bucks` buck cages in `best`'s layout (all of them if none are known to fit).
fn does_for(problem: &Problem, best: &Params, bucks: f64, max_tiers: u32) -> Vec<f64> {
    let fit = compute_layout(&Params { does: 99, bucks: bucks as u32, ..best.clone() }).n_doe as f64;
    let all = domain(problem, Var::Does, max_tiers);
    let does: Vec<f64> = all.iter().copied().filter(|d| *d <= fit).collect();
    if does.is_empty() { all } else { does }
}

/// Run-years to score one candidate.
fn eval_units(p: &Params, cfg: &OptConfig) -> u64 {
    cfg.eval_runs as u64 * u64::from(p.years.min(cfg.eval_years))
}

/// Candidates scored in one search round from `best` (later rounds can differ a little).
fn round_size(problem: &Problem, best: &Params, max_tiers: u32) -> u64 {
    let dom = |v: Var| domain(problem, v, max_tiers).len();
    let herd: usize = domain(problem, Var::Bucks, max_tiers).into_iter().map(|b| does_for(problem, best, b, max_tiers).len()).sum();
    let levers: usize = [Var::GrowerCageW, Var::Tiers, Var::RebreedDays, Var::WeanDays].into_iter().map(dom).sum();
    (herd + levers + dom(Var::ButcherRule) * dom(Var::ButcherDays)) as u64
}

/// Run-years a lever search should take: the starting plan, every round, and one check afterwards.
fn search_units(problem: &Problem, welfare: &Params, cfg: &OptConfig) -> u64 {
    let (best, max_tiers) = search_start(problem, welfare.clone());
    (2 + cfg.rounds as u64 * round_size(problem, &best, max_tiers)) * eval_units(&best, cfg)
}

/// Run-years for the full report on a plan.
fn finish_units(p: &Params) -> u64 {
    u64::from(p.runs) * u64::from(p.years)
}

/// Coordinate search over the free levers, reported as the `lo .. lo + width` stretch of `prog`.
fn search_levers(problem: &Problem, welfare: Params, cfg: &OptConfig, prog: &Progress, lo: f64, width: f64) -> Params {
    let (mut best, max_tiers) = search_start(problem, welfare);
    let dom = |v: Var| domain(problem, v, max_tiers);
    let eps = Objective::MaxMeat.epsilon();
    let per_round = round_size(problem, &best, max_tiers) * eval_units(&best, cfg);
    prog.stage(lo, width, (2 + cfg.rounds as u64 * round_size(problem, &best, max_tiers)) * eval_units(&best, cfg));
    let mut best_score = evaluate(&best, problem, cfg, prog);

    let try_all = |best: &mut Params, best_score: &mut Score, cands: Vec<Params>| -> bool {
        if cands.is_empty() {
            return false;
        }
        let scores: Vec<Score> = cands.par_iter().map(|c| evaluate(c, problem, cfg, prog)).collect();
        let mut pick = None;
        let mut top = *best_score;
        for (i, s) in scores.iter().enumerate() {
            if better(*s, top, eps) {
                top = *s;
                pick = Some(i);
            }
        }
        match pick {
            Some(i) => {
                *best = cands[i].clone();
                *best_score = top;
                true
            }
            None => false,
        }
    };
    let single = |v: Var, values: Vec<f64>, best: &Params| -> Vec<Params> {
        values
            .into_iter()
            .map(|x| {
                let mut p = best.clone();
                v.set(&mut p, x);
                p
            })
            .collect()
    };

    for round in 0..cfg.rounds {
        let mut improved = false;
        // herd size
        for bucks in dom(Var::Bucks) {
            let cands = does_for(problem, &best, bucks, max_tiers)
                .into_iter()
                .map(|d| {
                    let mut p = best.clone();
                    Var::Bucks.set(&mut p, bucks);
                    Var::Does.set(&mut p, d);
                    p
                })
                .collect();
            improved |= try_all(&mut best, &mut best_score, cands);
        }
        for v in [Var::GrowerCageW, Var::Tiers, Var::RebreedDays, Var::WeanDays] {
            let cands = single(v, dom(v), &best);
            improved |= try_all(&mut best, &mut best_score, cands);
        }
        let mut cands = Vec::new();
        for rule in dom(Var::ButcherRule) {
            for days in dom(Var::ButcherDays) {
                let mut p = best.clone();
                Var::ButcherRule.set(&mut p, rule);
                Var::ButcherDays.set(&mut p, days);
                cands.push(p);
            }
        }
        improved |= try_all(&mut best, &mut best_score, cands);
        if !improved {
            prog.add((cfg.rounds - round - 1) as u64 * per_round); // the rounds we skip
            break;
        }
    }
    best
}

// ---------------- finishing ----------------

fn mean_qol(p: &Params) -> f64 {
    Var::QOL_TYPES.iter().map(|v| v.get(p)).sum::<f64>() / 4.0
}

fn finish(problem: &Problem, params: Params, prog: &Progress, lo: f64, width: f64) -> Solution {
    prog.stage(lo, width, finish_units(&params));
    let summary = simulate_runs_tracked(&params, params.runs as usize, prog);
    let mut violations = Vec::new();
    for (o, b) in &problem.outs {
        let v = o.of(&summary);
        if b.violation(v) > 1e-9 {
            violations.push(Violation { subject: Subject::Out(*o), actual: v, bound: *b });
        }
    }
    let actual = |v: Var| match v {
        Var::Does => summary.eff_does as f64,
        Var::Bucks => summary.layout.n_buck as f64,
        Var::Tiers => summary.layout.tiers as f64,
        _ => v.get(&params),
    };
    for v in Var::ALL {
        let Some(b) = (if v == Var::Qol { None } else { problem.effective_bound(v) }) else { continue };
        if v == Var::MaxHeadcount {
            continue; // a cap is a limit we respect, not a target
        }
        let a = actual(v);
        if !b.contains(a) {
            violations.push(Violation { subject: Subject::Var(v), actual: a, bound: b });
        }
    }
    let objective_value = match problem.objective {
        Objective::MaxMeat => summary.metric(MetricKey::MeatKgPerYr).mean,
        Objective::MaxQol => mean_qol(&params),
    };
    let fits = has_herd(&summary) && summary.forced_share() <= MAX_FORCED_SHARE + 0.02;
    Solution { params, summary, objective_value, violations, fits }
}

pub fn solve(problem: &Problem, cfg: OptConfig) -> Solution {
    solve_tracked(problem, cfg, &Progress::default())
}

/// [`solve`], reporting how far it has got in `prog`.
pub fn solve_tracked(problem: &Problem, cfg: OptConfig, prog: &Progress) -> Solution {
    match problem.objective {
        Objective::MaxMeat => {
            let welfare = with_welfare(problem, qol_values(problem, None));
            let search = search_units(problem, &welfare, &cfg) as f64;
            let share = search / (search + finish_units(&welfare) as f64);
            let p = search_levers(problem, welfare, &cfg, prog, 0.0, share);
            finish(problem, p, prog, share, 1.0 - share)
        }
        Objective::MaxQol => solve_max_qol(problem, cfg, prog),
    }
}

/// Largest QoL scalar for which the result limits can still be met, found by bisection on a grid.
fn solve_max_qol(problem: &Problem, cfg: OptConfig, prog: &Progress) -> Solution {
    let steps = ((QOL_MAX - 1.0) / QOL_STEP).round() as usize;
    let s_of = |i: usize| 1.0 + QOL_STEP * i as f64;
    let fast = OptConfig { rounds: 2, ..cfg };
    // Plan the bar: a full search, a fast one at the top, the bisection, a full confirming search and
    // the report, sized from the lowest QoL (each search then measures its own work).
    let low = with_welfare(problem, qol_values(problem, Some(s_of(0))));
    let (full_u, fast_u) = (search_units(problem, &low, &cfg) as f64, search_units(problem, &low, &fast) as f64);
    let bisect = (usize::BITS - steps.max(1).leading_zeros()) as f64; // ≈ log2(steps), rounded up
    let total = 2.0 * full_u + (1.0 + bisect) * fast_u + finish_units(&low) as f64;
    let (full_w, fast_w) = (full_u / total, fast_u / total);
    let mut at = 0.0;
    let mut attempt = |i: usize, cfg: &OptConfig| -> (Params, bool) {
        let w = if cfg.rounds == fast.rounds { fast_w } else { full_w };
        let welfare = with_welfare(problem, qol_values(problem, Some(s_of(i))));
        let p = search_levers(problem, welfare, cfg, prog, at, w);
        let sc = evaluate(&p, problem, cfg, prog);
        at += w;
        (p, sc.violation <= 1e-9)
    };
    let (p0, ok0) = attempt(0, &cfg);
    if !ok0 {
        // even the lowest QoL can't meet the limits: report the closest plan
        return finish(problem, p0, prog, 1.0 - finish_units(&low) as f64 / total, finish_units(&low) as f64 / total);
    }
    let (mut lo, mut hi) = (0usize, steps);
    if attempt(hi, &fast).1 {
        lo = hi;
    } else {
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if attempt(mid, &fast).1 { lo = mid } else { hi = mid }
        }
    }
    // confirm with the full search; step down if the longer run disagrees
    let report = finish_units(&low) as f64 / total;
    let mut i = lo;
    loop {
        let (p, _) = attempt(i, &cfg);
        let sol = finish(problem, p, prog, 1.0 - report, report);
        if sol.feasible() || i == 0 {
            return sol;
        }
        i -= 1;
    }
}

/// Convenience: optimise layout and timing for the most meat with welfare and the headcount cap
/// held at the values in `start`.
pub fn optimise(start: &Params, cfg: OptConfig) -> Params {
    solve(&Problem::pinning_welfare(start.clone()), cfg).params
}
