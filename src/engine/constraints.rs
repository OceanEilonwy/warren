//! What the user can ask for: an objective, limits on optimisable inputs, and limits on results.

use serde::{Deserialize, Serialize};

use super::params::{ButcherRule, Params};
use super::summary::{MetricKey, Summary};

/// An optimisable input. Each one is either left free (the solver chooses) or constrained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Var {
    /// Shorthand: applies to every per-type QoL that has no constraint of its own.
    Qol,
    QolDoe,
    QolBuck,
    QolYoung,
    QolOld,
    MaxHeadcount,
    Tiers,
    GrowerCageW,
    Does,
    Bucks,
    WeanDays,
    RebreedDays,
    ButcherDays,
    ButcherRule,
}

impl Var {
    pub const ALL: [Var; 14] = [
        Var::Qol,
        Var::QolDoe,
        Var::QolBuck,
        Var::QolYoung,
        Var::QolOld,
        Var::MaxHeadcount,
        Var::Tiers,
        Var::GrowerCageW,
        Var::Does,
        Var::Bucks,
        Var::WeanDays,
        Var::RebreedDays,
        Var::ButcherDays,
        Var::ButcherRule,
    ];
    /// The four real per-type welfare multipliers.
    pub const QOL_TYPES: [Var; 4] = [Var::QolDoe, Var::QolBuck, Var::QolYoung, Var::QolOld];

    pub fn key(self) -> &'static str {
        match self {
            Var::Qol => "qol",
            Var::QolDoe => "qol_doe",
            Var::QolBuck => "qol_buck",
            Var::QolYoung => "qol_young",
            Var::QolOld => "qol_old",
            Var::MaxHeadcount => "max_headcount",
            Var::Tiers => "tiers",
            Var::GrowerCageW => "grower_cage_w",
            Var::Does => "does",
            Var::Bucks => "bucks",
            Var::WeanDays => "wean_days",
            Var::RebreedDays => "rebreed_days",
            Var::ButcherDays => "butcher_days",
            Var::ButcherRule => "butcher_rule",
        }
    }

    pub fn from_key(k: &str) -> Option<Var> {
        Var::ALL.into_iter().find(|v| v.key() == k)
    }

    /// Searched by the solver when free (as opposed to welfare multipliers and the headcount cap).
    pub fn is_lever(self) -> bool {
        matches!(
            self,
            Var::Tiers | Var::GrowerCageW | Var::Does | Var::Bucks | Var::WeanDays | Var::RebreedDays | Var::ButcherDays | Var::ButcherRule
        )
    }

    pub fn is_qol(self) -> bool {
        matches!(self, Var::Qol | Var::QolDoe | Var::QolBuck | Var::QolYoung | Var::QolOld)
    }

    pub fn get(self, p: &Params) -> f64 {
        match self {
            Var::Qol | Var::QolDoe => p.qol_doe,
            Var::QolBuck => p.qol_buck,
            Var::QolYoung => p.qol_young,
            Var::QolOld => p.qol_old,
            Var::MaxHeadcount => p.max_headcount as f64,
            Var::Tiers => p.tiers as f64,
            Var::GrowerCageW => p.grower_cage_w,
            Var::Does => p.does as f64,
            Var::Bucks => p.bucks as f64,
            Var::WeanDays => p.wean_days as f64,
            Var::RebreedDays => p.rebreed_days as f64,
            Var::ButcherDays => p.butcher_days as f64,
            Var::ButcherRule => (p.butcher_rule == ButcherRule::Room) as u8 as f64,
        }
    }

    pub fn set(self, p: &mut Params, v: f64) {
        match self {
            Var::Qol => p.set_all_qol(v),
            Var::QolDoe => p.qol_doe = v,
            Var::QolBuck => p.qol_buck = v,
            Var::QolYoung => p.qol_young = v,
            Var::QolOld => p.qol_old = v,
            Var::MaxHeadcount => p.max_headcount = v.round().max(0.0) as u32,
            Var::Tiers => p.tiers = v.round().max(1.0) as u32,
            Var::GrowerCageW => p.grower_cage_w = v,
            Var::Does => p.does = v.round().max(1.0) as u32,
            Var::Bucks => p.bucks = v.round().max(1.0) as u32,
            Var::WeanDays => p.wean_days = v.round() as u32,
            Var::RebreedDays => p.rebreed_days = v.round() as u32,
            Var::ButcherDays => p.butcher_days = v.round() as u32,
            Var::ButcherRule => p.butcher_rule = if v > 0.5 { ButcherRule::Room } else { ButcherRule::Age },
        }
    }
}

/// A result the user can limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Out {
    MeatKgPerYr,
    FryersPerYr,
    PeakHead,
}

impl Out {
    pub const ALL: [Out; 3] = [Out::MeatKgPerYr, Out::FryersPerYr, Out::PeakHead];

    pub fn key(self) -> &'static str {
        match self {
            Out::MeatKgPerYr => "meat",
            Out::FryersPerYr => "fryers",
            Out::PeakHead => "peak_head",
        }
    }

    pub fn from_key(k: &str) -> Option<Out> {
        Out::ALL.into_iter().find(|o| o.key() == k)
    }

    pub fn metric(self) -> MetricKey {
        match self {
            Out::MeatKgPerYr => MetricKey::MeatKgPerYr,
            Out::FryersPerYr => MetricKey::FryersPerYr,
            Out::PeakHead => MetricKey::PeakHead,
        }
    }

    pub fn of(self, s: &Summary) -> f64 {
        s.metric(self.metric()).mean
    }
}

/// Inclusive limits. `exact` (min == max) is an equality.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bound {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl Bound {
    pub fn exactly(v: f64) -> Bound {
        Bound { min: Some(v), max: Some(v) }
    }
    pub fn at_least(v: f64) -> Bound {
        Bound { min: Some(v), max: None }
    }
    pub fn at_most(v: f64) -> Bound {
        Bound { min: None, max: Some(v) }
    }
    pub fn exact(&self) -> Option<f64> {
        match (self.min, self.max) {
            (Some(a), Some(b)) if (a - b).abs() < 1e-12 => Some(a),
            _ => None,
        }
    }
    pub fn contains(&self, v: f64) -> bool {
        self.min.is_none_or(|m| v >= m - 1e-9) && self.max.is_none_or(|m| v <= m + 1e-9)
    }
    /// Relative amount by which `v` breaks the bound (0 if inside).
    pub fn violation(&self, v: f64) -> f64 {
        let (lim, amount) = match (self.min, self.max) {
            (Some(m), _) if v < m - 1e-9 => (m, m - v),
            (_, Some(m)) if v > m + 1e-9 => (m, v - m),
            _ => return 0.0,
        };
        amount / lim.abs().max(1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    /// Most dressed meat per year.
    MaxMeat,
    /// Most floor space per rabbit (average of the four QoL multipliers), subject to result limits.
    MaxQol,
}

impl Objective {
    pub const ALL: [Objective; 2] = [Objective::MaxMeat, Objective::MaxQol];

    pub fn key(self) -> &'static str {
        match self {
            Objective::MaxMeat => "max_meat",
            Objective::MaxQol => "max_qol",
        }
    }
    pub fn from_key(k: &str) -> Option<Objective> {
        Objective::ALL.into_iter().find(|o| o.key() == k)
    }
    /// Positive when moving from `from` to `to` is an improvement.
    pub fn improvement(self, from: f64, to: f64) -> f64 {
        match self {
            Objective::MaxMeat | Objective::MaxQol => to - from,
        }
    }
    /// Smallest difference that counts as better.
    pub fn epsilon(self) -> f64 {
        match self {
            Objective::MaxMeat => 0.5,
            Objective::MaxQol => 0.005,
        }
    }
}

/// Everything the solver needs: fixed inputs, objective, and constraints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Problem {
    /// Fixed inputs (space, prices, biology…). Optimisable fields are overwritten by the solver.
    pub base: Params,
    pub objective: Objective,
    pub vars: Vec<(Var, Bound)>,
    pub outs: Vec<(Out, Bound)>,
}

impl Problem {
    pub fn new(base: Params, objective: Objective) -> Problem {
        Problem { base, objective, vars: Vec::new(), outs: Vec::new() }
    }

    pub fn with_var(mut self, v: Var, b: Bound) -> Problem {
        self.vars.retain(|(x, _)| *x != v);
        self.vars.push((v, b));
        self
    }

    pub fn with_out(mut self, o: Out, b: Bound) -> Problem {
        self.outs.retain(|(x, _)| *x != o);
        self.outs.push((o, b));
        self
    }

    pub fn without_var(&self, v: Var) -> Problem {
        let mut p = self.clone();
        p.vars.retain(|(x, _)| *x != v);
        p
    }

    pub fn without_out(&self, o: Out) -> Problem {
        let mut p = self.clone();
        p.outs.retain(|(x, _)| *x != o);
        p
    }

    pub fn var_bound(&self, v: Var) -> Option<Bound> {
        self.vars.iter().find(|(x, _)| *x == v).map(|(_, b)| *b)
    }

    /// The bound that applies to `v`: its own, or for a per-type QoL the master `Qol` bound.
    pub fn effective_bound(&self, v: Var) -> Option<Bound> {
        self.var_bound(v).or_else(|| if v.is_qol() && v != Var::Qol { self.var_bound(Var::Qol) } else { None })
    }

    /// Everything pinned or limited by hand, as a convenience for tests and callers
    /// that want the old "optimise only the layout and timings" behaviour.
    pub fn pinning_welfare(base: Params) -> Problem {
        let mut p = Problem::new(base.clone(), Objective::MaxMeat);
        for v in Var::QOL_TYPES {
            p = p.with_var(v, Bound::exactly(v.get(&base)));
        }
        p.with_var(Var::MaxHeadcount, Bound::at_most(base.max_headcount as f64))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
    Var(Var),
    Out(Out),
}

#[derive(Clone, Copy, Debug)]
pub struct Violation {
    pub subject: Subject,
    pub actual: f64,
    pub bound: Bound,
}

#[derive(Clone, Debug)]
pub struct Solution {
    pub params: Params,
    pub summary: Summary,
    /// Meat kg/yr, $/kg, or the mean QoL multiplier, depending on the objective.
    pub objective_value: f64,
    pub violations: Vec<Violation>,
    /// Share of rabbits butchered early for lack of room is within the accepted limit.
    pub fits: bool,
}

impl Solution {
    pub fn feasible(&self) -> bool {
        self.violations.is_empty() && self.fits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        for v in Var::ALL {
            assert_eq!(Var::from_key(v.key()), Some(v));
        }
        for o in Out::ALL {
            assert_eq!(Out::from_key(o.key()), Some(o));
        }
        for o in Objective::ALL {
            assert_eq!(Objective::from_key(o.key()), Some(o));
        }
        assert_eq!(Var::from_key("nope"), None);
    }

    #[test]
    fn var_get_set_round_trip_on_defaults() {
        for v in Var::ALL {
            let mut p = Params::default();
            let x = v.get(&p);
            v.set(&mut p, x);
            assert_eq!(p, Params::default(), "{v:?}");
        }
    }

    #[test]
    fn master_qol_sets_all_types() {
        let mut p = Params::default();
        Var::Qol.set(&mut p, 1.7);
        assert!(Var::QOL_TYPES.iter().all(|v| v.get(&p) == 1.7));
    }

    #[test]
    fn bound_semantics() {
        let b = Bound::at_least(5.0);
        assert!(b.contains(5.0) && b.contains(9.0) && !b.contains(4.9));
        assert_eq!(b.violation(4.0), 0.2);
        assert_eq!(b.violation(6.0), 0.0);
        let b = Bound::at_most(10.0);
        assert!(b.contains(10.0) && !b.contains(10.5));
        assert!((b.violation(12.0) - 0.2).abs() < 1e-12);
        let b = Bound::exactly(3.0);
        assert_eq!(b.exact(), Some(3.0));
        assert!(b.contains(3.0) && !b.contains(3.1) && !b.contains(2.9));
        assert_eq!(Bound::default().exact(), None);
        assert!(Bound::default().contains(1e9));
        // tiny limits use an absolute scale of 1
        assert!((Bound::at_least(0.1).violation(0.0) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn effective_bound_falls_back_to_master_qol_for_types_only() {
        let p = Problem::new(Params::default(), Objective::MaxMeat).with_var(Var::Qol, Bound::at_least(1.5));
        assert_eq!(p.effective_bound(Var::QolDoe), Some(Bound::at_least(1.5)));
        assert_eq!(p.effective_bound(Var::Does), None);
        let p = p.with_var(Var::QolDoe, Bound::exactly(2.0));
        assert_eq!(p.effective_bound(Var::QolDoe), Some(Bound::exactly(2.0)));
        assert_eq!(p.effective_bound(Var::QolBuck), Some(Bound::at_least(1.5)));
    }

    #[test]
    fn with_and_without_replace_and_remove() {
        let p = Problem::new(Params::default(), Objective::MaxMeat)
            .with_var(Var::Does, Bound::exactly(3.0))
            .with_var(Var::Does, Bound::exactly(4.0))
            .with_out(Out::MeatKgPerYr, Bound::at_least(200.0));
        assert_eq!(p.vars.len(), 1);
        assert_eq!(p.var_bound(Var::Does), Some(Bound::exactly(4.0)));
        assert!(p.without_var(Var::Does).vars.is_empty());
        assert!(p.without_out(Out::MeatKgPerYr).outs.is_empty());
        assert_eq!(p.outs.len(), 1, "without_* leaves the original untouched");
    }

    #[test]
    fn objective_improvement_direction() {
        assert!(Objective::MaxMeat.improvement(100.0, 120.0) > 0.0);
        assert!(Objective::MaxQol.improvement(1.0, 1.5) > 0.0);
    }
}
