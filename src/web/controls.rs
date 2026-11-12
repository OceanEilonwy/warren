//! Single source of truth for every slider: label, range, formatting, and typed get/set
//! accessors on `Params`. Drives form rendering, request parsing and the value labels.

use std::collections::HashMap;

use crate::engine::{Bound, Objective, Out, Params, Replacement, Solution, Var};

#[derive(Clone, Copy)]
pub enum Kind {
    Range { min: f64, max: f64, step: f64 },
    Bool,
    Choice(&'static [(f64, &'static str)]),
}

#[derive(Clone, Copy)]
pub enum Fmt {
    Metres,
    Cm,
    WeeksDays,
    Days,
    Pct,
    /// A whole number that is already a percentage (e.g. 10 for 10%).
    PctPoints,
    Money,
    MoneyPerKg,
    Int,
    Years,
    GPerDay,
    Qol,
    CapOrNone,
    Plain,
    Kg,
}

fn trim1(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

impl Fmt {
    /// Text shown before and after the number in an entry box ("$" … "/kg"). Inputs are typed in the
    /// same units the label uses, except `Pct`, which stores a fraction but is typed as a percentage
    /// (see `scale`), and `Qol`, typed as a multiple of the Code minimum.
    pub fn affixes(self) -> (&'static str, &'static str) {
        match self {
            Fmt::Metres => ("", "m"),
            Fmt::Cm => ("", "cm"),
            Fmt::WeeksDays | Fmt::Days => ("", "d"),
            Fmt::Pct | Fmt::PctPoints => ("", "%"),
            Fmt::Money => ("$", ""),
            Fmt::MoneyPerKg => ("$", "/kg"),
            Fmt::Int | Fmt::Plain => ("", ""),
            Fmt::Years => ("", "yr"),
            Fmt::GPerDay => ("", "g/day"),
            Fmt::Qol => ("×", ""),
            Fmt::CapOrNone => ("", "rabbits"),
            Fmt::Kg => ("", "kg"),
        }
    }

    /// Multiplier from the stored value to the number typed in the box.
    pub fn scale(self) -> f64 {
        if matches!(self, Fmt::Pct) { 100.0 } else { 1.0 }
    }

    pub fn render(self, v: f64) -> String {
        match self {
            Fmt::Metres => format!("{v:.2} m"),
            Fmt::Cm => format!("{v:.0} cm"),
            Fmt::WeeksDays => format!("{} wk ({v:.0} d)", trim1(v / 7.0)),
            Fmt::Days => format!("{v:.0} d"),
            Fmt::Pct => format!("{:.0}%", v * 100.0),
            Fmt::PctPoints => format!("{v:.0}%"),
            Fmt::Money => format!("${v:.0}"),
            Fmt::MoneyPerKg => format!("${v:.2}/kg"),
            Fmt::Int => format!("{v:.0}"),
            Fmt::Years => format!("{} yr", trim1(v)),
            Fmt::GPerDay => format!("{v:.0} g/day"),
            Fmt::Qol => {
                if (v - 1.0).abs() < 1e-9 {
                    "minimum".into()
                } else {
                    format!("+{:.0}%", (v - 1.0) * 100.0)
                }
            }
            Fmt::CapOrNone => {
                if v == 0.0 {
                    "no cap".into()
                } else {
                    format!("{v:.0}")
                }
            }
            Fmt::Plain => trim1(v),
            Fmt::Kg => format!("{v:.1} kg"),
        }
    }
}

pub struct Ctl {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub fmt: Fmt,
    pub help: &'static str,
    pub get: fn(&Params) -> f64,
    pub set: fn(&mut Params, f64),
}

pub struct Group {
    pub title: &'static str,
    pub open: bool,
    pub note: &'static str,
    pub ctls: &'static [Ctl],
}

impl Ctl {
    pub fn value_text(&self, v: f64) -> String {
        match self.kind {
            Kind::Bool => {
                if v > 0.5 {
                    "yes".into()
                } else {
                    "no".into()
                }
            }
            Kind::Choice(cs) => cs.iter().find(|(x, _)| (*x - v).abs() < 1e-9).map(|c| c.1.to_string()).unwrap_or_default(),
            Kind::Range { .. } => self.fmt.render(v),
        }
    }

    /// Fraction (0..1) of the way along a range slider.
    pub fn fraction(&self, v: f64) -> f64 {
        match self.kind {
            Kind::Range { min, max, .. } => ((v - min) / (max - min)).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// Read a form value into `p`, ignoring anything malformed and clamping to the slider range.
    fn apply(&self, p: &mut Params, raw: &str) {
        let v = match self.kind {
            Kind::Bool => match raw {
                "true" | "1" | "on" | "yes" => 1.0,
                "false" | "0" | "off" | "no" => 0.0,
                _ => return,
            },
            Kind::Choice(cs) => match raw.parse::<f64>() {
                Ok(v) if cs.iter().any(|(x, _)| (*x - v).abs() < 1e-9) => v,
                _ => return,
            },
            Kind::Range { min, max, .. } => match raw.parse::<f64>() {
                Ok(v) if v.is_finite() => v.clamp(min, max),
                _ => return,
            },
        };
        (self.set)(p, v);
    }
}

macro_rules! num {
    ($key:ident, $t:ty, $label:expr, $min:expr, $max:expr, $step:expr, $fmt:expr, $help:expr) => {
        Ctl {
            key: stringify!($key),
            label: $label,
            kind: Kind::Range { min: $min, max: $max, step: $step },
            fmt: $fmt,
            help: $help,
            get: |p| p.$key as f64,
            set: |p, v| p.$key = if stringify!($t) == "f64" { v as $t } else { v.round() as $t },
        }
    };
}

const REPLACE_CHOICES: &[(f64, &str)] = &[(0.0, "Keep own"), (1.0, "Buy in")];

/// Fixed inputs that are always shown: the space you have.
pub static SPACE: Group = Group {
    title: "Space",
    open: true,
    note: "",
    ctls: &[
        num!(width_m, f64, "Width", 0.5, 6.0, 0.05, Fmt::Metres, ""),
        num!(depth_m, f64, "Depth", 0.4, 3.0, 0.05, Fmt::Metres, ""),
        num!(height_m, f64, "Height", 0.4, 3.0, 0.05, Fmt::Metres, ""),
        Ctl {
            key: "access_both_sides",
            label: "Reach in from both long sides",
            kind: Kind::Bool,
            fmt: Fmt::Plain,
            help: "If not, only one row of cages (max reach deep) is usable.",
            get: |p| p.access_both_sides as u8 as f64,
            set: |p, v| p.access_both_sides = v > 0.5,
        },
    ],
};

/// Fixed inputs with sensible defaults, tucked away under "Assumptions".
pub static ASSUMPTIONS: [Group; 5] = [
    Group {
        title: "Housing details",
        open: false,
        note: "",
        ctls: &[
            num!(cage_height_cm, f64, "Cage interior height", 45.0, 90.0, 1.0, Fmt::Cm, "Code minimum 45 cm for rabbits over 12 weeks."),
            num!(tier_overhead_cm, f64, "Floor + tray + gap per tier", 5.0, 30.0, 1.0, Fmt::Cm, ""),
            num!(max_reach_m, f64, "Max cage depth you can reach", 0.4, 1.2, 0.05, Fmt::Metres, ""),
        ],
    },
    Group {
        title: "Management details",
        open: false,
        note: "",
        ctls: &[
            num!(
                min_butcher_days,
                u32,
                "Youngest age worth butchering",
                42.0,
                112.0,
                1.0,
                Fmt::WeeksDays,
                "Under 'when room is needed': rabbits culled younger than this count as forced."
            ),
            num!(sex_separate_days, u32, "Split growers by sex from", 56.0, 140.0, 1.0, Fmt::WeeksDays, ""),
            num!(breed_age_days, u32, "Age of first breeding", 112.0, 240.0, 1.0, Fmt::WeeksDays, ""),
            num!(breeder_life_years, f64, "Breeder working life", 1.0, 5.0, 0.5, Fmt::Years, ""),
            Ctl {
                key: "replacement",
                label: "Replace breeders",
                kind: Kind::Choice(REPLACE_CHOICES),
                fmt: Fmt::Plain,
                help: "",
                get: |p| (p.replacement == Replacement::Buy) as u8 as f64,
                set: |p, v| p.replacement = if v > 0.5 { Replacement::Buy } else { Replacement::Own },
            },
            num!(purchase_age_days, u32, "Age of bought-in rabbits", 56.0, 200.0, 1.0, Fmt::WeeksDays, ""),
        ],
    },
    Group {
        title: "Feed & costs",
        open: false,
        note: "",
        ctls: &[
            num!(pellet_price, f64, "Pellets", 0.3, 4.0, 0.05, Fmt::MoneyPerKg, ""),
            num!(hay_price, f64, "Hay", 0.1, 4.0, 0.05, Fmt::MoneyPerKg, ""),
            num!(wastage_pct, f64, "Feed wastage", 0.0, 30.0, 1.0, Fmt::PctPoints, "Feed pulled out of the hopper and not eaten."),
            num!(breeder_price, f64, "Price per bought-in rabbit", 0.0, 150.0, 5.0, Fmt::Money, ""),
        ],
    },
    Group {
        title: "Biology",
        open: false,
        note: "Not set by the Code — typical NZ White figures. Adjust to your stock.",
        ctls: &[
            num!(
                dressed_weight_kg,
                f64,
                "Average dressed weight",
                0.9,
                2.2,
                0.01,
                Fmt::Kg,
                "Carcass weight of a typical rabbit butchered at 12 weeks. Commercial NZ White lines reach about 1.4 kg; smaller local lines less. Scales the whole growth curve, so meat per rabbit and feed both follow."
            ),
            num!(litter_mean, f64, "Kits born per litter (mean)", 4.0, 12.0, 0.5, Fmt::Plain, ""),
            num!(pre_wean_mortality, f64, "Kit losses before weaning", 0.0, 0.4, 0.01, Fmt::Pct, ""),
            num!(
                post_wean_mortality,
                f64,
                "Grower losses, 6 to 12 weeks",
                0.0,
                0.2,
                0.01,
                Fmt::Pct,
                "The same daily risk applies at every age after weaning, so growers kept longer lose more."
            ),
            num!(conception_rate, f64, "Matings that take", 0.4, 1.0, 0.01, Fmt::Pct, "For a doe that is not nursing a litter."),
            num!(
                nursing_conception_rate,
                f64,
                "Matings that take while nursing",
                0.2,
                1.0,
                0.01,
                Fmt::Pct,
                "For a doe re-mated while her litter is still with her. Usually lower; set from your own records."
            ),
            num!(breeder_mortality, f64, "Breeder deaths per year", 0.0, 0.3, 0.01, Fmt::Pct, ""),
            num!(gestation_days, u32, "Gestation", 29.0, 34.0, 1.0, Fmt::Days, ""),
            num!(pellet_rate_grower, f64, "Grower pellets (g/kg^0.75/day)", 50.0, 120.0, 1.0, Fmt::Int, ""),
            num!(pellet_rate_adult, f64, "Adult pellets (g/kg^0.75/day)", 30.0, 80.0, 1.0, Fmt::Int, ""),
            num!(lactation_per_kit, f64, "Extra doe pellets per nursing kit", 10.0, 50.0, 1.0, Fmt::GPerDay, ""),
            num!(hay_rate, f64, "Hay (g/kg^0.75/day)", 0.0, 60.0, 1.0, Fmt::Int, ""),
        ],
    },
    Group {
        title: "Simulation",
        open: false,
        note: "Runs use every CPU core, so thousands of runs over many years are practical.",
        ctls: &[
            num!(runs, u32, "Randomised runs", 10.0, 5000.0, 10.0, Fmt::Int, ""),
            num!(years, u32, "Years simulated", 2.0, 30.0, 1.0, Fmt::Years, ""),
            num!(
                warmup_days,
                u32,
                "Start-up excluded from annual figures",
                91.0,
                728.0,
                7.0,
                Fmt::WeeksDays,
                "Shortened if needed so at least a year is counted after start-up."
            ),
            num!(seed, u64, "Random seed", 1.0, 999.0, 1.0, Fmt::Int, ""),
        ],
    },
];

pub fn all_ctls() -> impl Iterator<Item = &'static Ctl> {
    std::iter::once(&SPACE).chain(ASSUMPTIONS.iter()).flat_map(|g| g.ctls.iter())
}

pub fn ctl(key: &str) -> Option<&'static Ctl> {
    all_ctls().find(|c| c.key == key)
}

/// Overlay submitted form fields onto `p`. Unknown or malformed fields are ignored.
pub fn apply_form(p: &mut Params, form: &HashMap<String, String>) {
    for c in all_ctls() {
        if let Some(raw) = form.get(c.key) {
            c.apply(p, raw);
        }
    }
}

// ---------------- constraint pool ----------------

pub struct VarUi {
    pub var: Var,
    pub label: &'static str,
    pub group: &'static str,
    pub kind: Kind,
    pub fmt: Fmt,
    pub help: &'static str,
}

const RULE_CHOICES: &[(f64, &str)] = &[(0.0, "At set age"), (1.0, "When room is needed")];
const fn rng(min: f64, max: f64, step: f64) -> Kind {
    Kind::Range { min, max, step }
}

pub static VAR_UI: [VarUi; 14] = [
    VarUi {
        var: Var::Qol,
        label: "All cages — floor space vs code minimum",
        group: "Welfare",
        kind: rng(1.0, 3.0, 0.05),
        fmt: Fmt::Qol,
        help: "Applies to every cage type below that has no limit of its own.",
    },
    VarUi {
        var: Var::QolDoe,
        label: "Doe + litter cage",
        group: "Welfare",
        kind: rng(1.0, 3.0, 0.05),
        fmt: Fmt::Qol,
        help: "Code: 0.56 m² (litter to 5 wk) or 0.74 m² (to 8 wk). A doe keeps this cage whether or not she has a litter.",
    },
    VarUi {
        var: Var::QolBuck,
        label: "Buck cage",
        group: "Welfare",
        kind: rng(1.0, 3.0, 0.05),
        fmt: Fmt::Qol,
        help: "Code: 0.56 m² per adult breeder.",
    },
    VarUi {
        var: Var::QolYoung,
        label: "Growers 5–12 weeks (per rabbit)",
        group: "Welfare",
        kind: rng(1.0, 3.0, 0.05),
        fmt: Fmt::Qol,
        help: "Code: 0.07 m² each.",
    },
    VarUi {
        var: Var::QolOld,
        label: "Growers 12 weeks+ (per rabbit)",
        group: "Welfare",
        kind: rng(1.0, 3.0, 0.05),
        fmt: Fmt::Qol,
        help: "Code: 0.18 m² each in groups; also used for replacements housed alone.",
    },
    VarUi {
        var: Var::MaxHeadcount,
        label: "Max rabbits at any one time",
        group: "Herd",
        kind: rng(0.0, 200.0, 1.0),
        fmt: Fmt::CapOrNone,
        help: "Counts every rabbit including unweaned kits. Matings are held back when there would be no room for the litter; the oldest growers are butchered early if needed. 0 = no cap.",
    },
    VarUi { var: Var::Tiers, label: "Tiers (stacked)", group: "Cage design", kind: rng(1.0, 5.0, 1.0), fmt: Fmt::Int, help: "" },
    VarUi {
        var: Var::GrowerCageW,
        label: "Grower cage width",
        group: "Cage design",
        kind: rng(0.3, 3.0, 0.05),
        fmt: Fmt::Metres,
        help: "Doe and buck cages are sized from the Code. The rest of each row is cut into grower cages about this wide.",
    },
    VarUi { var: Var::Does, label: "Breeding does", group: "Herd", kind: rng(1.0, 24.0, 1.0), fmt: Fmt::Int, help: "" },
    VarUi { var: Var::Bucks, label: "Bucks", group: "Herd", kind: rng(1.0, 3.0, 1.0), fmt: Fmt::Int, help: "" },
    VarUi {
        var: Var::WeanDays,
        label: "Weaning age",
        group: "Timing",
        kind: rng(28.0, 56.0, 1.0),
        fmt: Fmt::WeeksDays,
        help: "Weaning at 5 weeks or earlier lets doe cages be the smaller size.",
    },
    VarUi {
        var: Var::RebreedDays,
        label: "Re-mate doe after kindling",
        group: "Timing",
        kind: rng(11.0, 70.0, 1.0),
        fmt: Fmt::Days,
        help: "The fastest allowed is 11 days (intensive commercial practice).",
    },
    VarUi {
        var: Var::ButcherDays,
        label: "Butchering age (latest, if 'when room is needed')",
        group: "Timing",
        kind: rng(56.0, 168.0, 1.0),
        fmt: Fmt::WeeksDays,
        help: "",
    },
    VarUi {
        var: Var::ButcherRule,
        label: "Butchering rule",
        group: "Timing",
        kind: Kind::Choice(RULE_CHOICES),
        fmt: Fmt::Plain,
        help: "'When room is needed' butchers the oldest growers whenever the cages fill, and everyone by the butchering age.",
    },
];

/// Short names for the cramped constraints list.
pub fn short_label(v: Var) -> &'static str {
    match v {
        Var::Qol => "All cages: floor space",
        Var::QolDoe => "Doe + litter cage",
        Var::QolBuck => "Buck cage",
        Var::QolYoung => "Growers 5–12 wk",
        Var::QolOld => "Growers 12 wk+",
        Var::MaxHeadcount => "Max rabbits",
        Var::Tiers => "Tiers",
        Var::GrowerCageW => "Grower cage width",
        Var::Does => "Breeding does",
        Var::Bucks => "Bucks",
        Var::WeanDays => "Weaning age",
        Var::RebreedDays => "Re-mate after kindling",
        Var::ButcherDays => "Butchering age",
        Var::ButcherRule => "Butchering rule",
    }
}

pub fn var_ui(v: Var) -> &'static VarUi {
    VAR_UI.iter().find(|u| u.var == v).expect("every Var has UI metadata")
}

impl VarUi {
    pub fn value_text(&self, v: f64) -> String {
        match self.kind {
            Kind::Choice(cs) => cs.iter().find(|(x, _)| (*x - v).abs() < 1e-9).map(|c| c.1.to_string()).unwrap_or_default(),
            Kind::Bool => {
                if v > 0.5 {
                    "yes".into()
                } else {
                    "no".into()
                }
            }
            Kind::Range { .. } => self.fmt.render(v),
        }
    }
    pub fn range(&self) -> Option<(f64, f64, f64)> {
        match self.kind {
            Kind::Range { min, max, step } => Some((min, max, step)),
            _ => None,
        }
    }
}

pub struct OutUi {
    pub out: Out,
    pub label: &'static str,
    pub fmt: Fmt,
    pub unit: &'static str,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

pub static OUT_UI: [OutUi; 3] = [
    OutUi { out: Out::MeatKgPerYr, label: "Dressed meat", fmt: Fmt::Int, unit: "kg/yr", min: 0.0, max: 1000.0, step: 5.0 },
    OutUi { out: Out::FryersPerYr, label: "Rabbits butchered", fmt: Fmt::Int, unit: "/yr", min: 0.0, max: 1500.0, step: 10.0 },
    OutUi { out: Out::PeakHead, label: "Rabbits on hand (peak)", fmt: Fmt::Int, unit: "", min: 0.0, max: 300.0, step: 1.0 },
];

pub fn short_out_label(o: Out) -> &'static str {
    match o {
        Out::MeatKgPerYr => "Dressed meat",
        Out::FryersPerYr => "Rabbits butchered",
        Out::PeakHead => "Rabbits on hand (peak)",
    }
}

pub fn out_ui(o: Out) -> &'static OutUi {
    OUT_UI.iter().find(|u| u.out == o).expect("every Out has UI metadata")
}

impl OutUi {
    pub fn value_text(&self, v: f64) -> String {
        let n = self.fmt.render(v);
        match self.unit {
            "" => n,
            "$/kg" | "$/yr" => format!("${n}{}", &self.unit[1..]),
            u => format!("{n} {u}"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    AtLeast,
    Exactly,
    AtMost,
}

impl Op {
    pub fn key(self) -> &'static str {
        match self {
            Op::AtLeast => "min",
            Op::Exactly => "eq",
            Op::AtMost => "max",
        }
    }
    pub fn from_key(k: &str) -> Option<Op> {
        match k {
            "min" => Some(Op::AtLeast),
            "eq" => Some(Op::Exactly),
            "max" => Some(Op::AtMost),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Op::AtLeast => "at least",
            Op::Exactly => "exactly",
            Op::AtMost => "at most",
        }
    }
    pub fn bound(self, v: f64) -> Bound {
        match self {
            Op::AtLeast => Bound::at_least(v),
            Op::Exactly => Bound::exactly(v),
            Op::AtMost => Bound::at_most(v),
        }
    }
    pub fn of(b: &Bound) -> Op {
        if b.exact().is_some() {
            Op::Exactly
        } else if b.min.is_some() {
            Op::AtLeast
        } else {
            Op::AtMost
        }
    }
}

/// Which operators make sense for an input. A cap on the herd only has a ceiling; choices are exact.
pub fn allowed_ops(var: Var) -> &'static [Op] {
    match var {
        Var::MaxHeadcount => &[Op::AtMost],
        Var::ButcherRule => &[Op::Exactly],
        _ => &[Op::AtLeast, Op::Exactly, Op::AtMost],
    }
}

/// The number a bound is anchored on (its limit, or the pinned value).
pub fn bound_value(b: &Bound) -> f64 {
    b.exact().or(b.min).or(b.max).unwrap_or(0.0)
}

/// Everything the form carries.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub objective: Objective,
    /// Fixed inputs (space + assumptions).
    pub params: Params,
    pub vars: Vec<(Var, Bound)>,
    pub outs: Vec<(Out, Bound)>,
}

impl Default for Request {
    fn default() -> Self {
        Request {
            objective: Objective::MaxMeat,
            params: Params::default(),
            // the starting constraints: one QoL and a herd cap (0 = none)
            vars: vec![(Var::Qol, Bound::at_least(1.0)), (Var::MaxHeadcount, Bound::at_most(0.0))],
            outs: Vec::new(),
        }
    }
}

impl Request {
    pub fn problem(&self) -> crate::engine::Problem {
        crate::engine::Problem { base: self.params.clone(), objective: self.objective, vars: self.vars.clone(), outs: self.outs.clone() }
    }

    pub fn has(&self, key: &str) -> bool {
        self.vars.iter().any(|(v, _)| v.key() == key) || self.outs.iter().any(|(o, _)| o.key() == key)
    }

    pub fn remove(&mut self, key: &str) {
        self.vars.retain(|(v, _)| v.key() != key);
        self.outs.retain(|(o, _)| o.key() != key);
    }

    /// "Maximise floor space" needs something to trade against. Suggest keeping three quarters of the
    /// meat (rounded down to 5 kg) and of the herd (peak rabbits) that the same constraints allow at
    /// minimum floor space. Existing limits on either are left alone.
    pub fn suggest_floors(&mut self, max_meat: f64, max_peak: f64) {
        if !self.has(Out::MeatKgPerYr.key()) {
            let floor = ((max_meat * 0.75) / 5.0).floor() * 5.0;
            self.outs.push((Out::MeatKgPerYr, Bound::at_least(floor.max(0.0))));
        }
        if !self.has(Out::PeakHead.key()) {
            self.outs.push((Out::PeakHead, Bound::at_least((max_peak * 0.75).floor().max(0.0))));
        }
    }

    /// True when something the user set can limit how much floor space the rabbits get. Otherwise
    /// "maximise floor space" is only limited by what fits in the space.
    pub fn limits_floor_space(&self) -> bool {
        !self.outs.is_empty()
            || self.vars.iter().any(|(v, b)| match v {
                Var::MaxHeadcount => false,
                v if v.is_qol() => b.max.is_some(),
                _ => true,
            })
    }

    /// Add a constraint anchored on what the current solution does, so nothing changes until it is edited.
    pub fn add_from(&mut self, key: &str, sol: &Solution) {
        if self.has(key) {
            return;
        }
        if let Some(v) = Var::from_key(key) {
            let cur = v.get(&sol.params);
            let b = match v {
                Var::Qol | Var::QolDoe | Var::QolBuck | Var::QolYoung | Var::QolOld => Bound::at_least(cur),
                Var::MaxHeadcount => Bound::at_most(sol.summary.metric(crate::engine::MetricKey::PeakHead).mean.ceil()),
                _ => Bound::exactly(cur),
            };
            self.vars.push((v, b));
        } else if let Some(o) = Out::from_key(key) {
            let cur = o.of(&sol.summary);
            self.outs.push((o, Bound::at_least(cur.floor())));
        }
    }
}

/// Parse a form submission. `cons` marks that constraint fields are present (an empty list is valid).
pub fn parse_request(q: &HashMap<String, String>) -> Request {
    let mut req = Request::default();
    apply_form(&mut req.params, q);
    if let Some(o) = q.get("objective").and_then(|o| Objective::from_key(o)) {
        req.objective = o;
    }
    if !q.contains_key("cons") {
        return req;
    }
    let read = |key: &str, min: f64, max: f64, choice: bool, allowed: &[Op]| -> Option<Bound> {
        q.get(&format!("on_{key}"))?;
        let v: f64 = q.get(&format!("val_{key}"))?.parse().ok().filter(|v: &f64| v.is_finite())?;
        let asked = if choice { Some(Op::Exactly) } else { q.get(&format!("op_{key}")).and_then(|o| Op::from_key(o)) };
        let fallback = if allowed.contains(&Op::Exactly) { Op::Exactly } else { allowed[0] };
        let op = asked.filter(|o| allowed.contains(o)).unwrap_or(fallback);
        Some(op.bound(v.clamp(min, max)))
    };
    req.vars.clear();
    req.outs.clear();
    for u in VAR_UI.iter() {
        let (min, max) = u.range().map_or((0.0, 1.0), |(a, b, _)| (a, b));
        if let Some(b) = read(u.var.key(), min, max, matches!(u.kind, Kind::Choice(_)), allowed_ops(u.var)) {
            req.vars.push((u.var, b));
        }
    }
    for u in OUT_UI.iter() {
        if let Some(b) = read(u.out.key(), u.min, u.max, false, &[Op::AtLeast, Op::Exactly, Op::AtMost]) {
            req.outs.push((u.out, b));
        }
    }
    req
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn keys_are_unique() {
        let mut keys: Vec<_> = all_ctls().map(|c| c.key).collect();
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n);
    }

    #[test]
    fn get_set_roundtrip_for_every_control() {
        for c in all_ctls() {
            let mut p = Params::default();
            let v = (c.get)(&p);
            (c.set)(&mut p, v);
            assert_eq!(p, Params::default(), "{} changed the defaults", c.key);
        }
    }

    #[test]
    fn defaults_sit_inside_every_slider_range() {
        let d = Params::default();
        for c in all_ctls() {
            if let Kind::Range { min, max, .. } = c.kind {
                let v = (c.get)(&d);
                assert!(v >= min - 1e-9 && v <= max + 1e-9, "{} default {v} outside {min}..{max}", c.key);
            }
        }
    }

    #[test]
    fn form_values_are_applied_and_clamped() {
        let mut p = Params::default();
        apply_form(
            &mut p,
            &form(&[("runs", "700"), ("width_m", "99"), ("depth_m", "-3"), ("access_both_sides", "false"), ("replacement", "1")]),
        );
        assert_eq!(p.runs, 700);
        assert_eq!(p.width_m, 6.0);
        assert_eq!(p.depth_m, 0.4);
        assert!(!p.access_both_sides);
        assert_eq!(p.replacement, Replacement::Buy);
    }

    #[test]
    fn garbage_is_ignored() {
        let mut p = Params::default();
        apply_form(
            &mut p,
            &form(&[("runs", "lots"), ("width_m", "NaN"), ("replacement", "7"), ("nope", "1"), ("access_both_sides", "maybe")]),
        );
        assert_eq!(p, Params::default());
    }

    #[test]
    fn formatting() {
        assert_eq!(Fmt::WeeksDays.render(42.0), "6 wk (42 d)");
        assert_eq!(Fmt::WeeksDays.render(45.0), "6.4 wk (45 d)");
        assert_eq!(Fmt::Qol.render(1.0), "minimum");
        assert_eq!(Fmt::Qol.render(1.25), "+25%");
        assert_eq!(Fmt::CapOrNone.render(0.0), "no cap");
        assert_eq!(Fmt::Metres.render(0.75), "0.75 m");
    }

    #[test]
    fn every_var_and_out_has_ui_metadata_and_a_sane_range() {
        for v in Var::ALL {
            let u = var_ui(v);
            assert_eq!(u.var, v);
            if let Some((min, max, step)) = u.range() {
                assert!(min < max && step > 0.0);
                if v.is_qol() {
                    assert_eq!(min, 1.0, "QoL cannot go below the Code minimum");
                }
                let d = (v.get(&Params::default()) - min) / step;
                let _ = d; // defaults need not be on the grid for optimisable inputs
            }
        }
        for o in Out::ALL {
            let u = out_ui(o);
            assert_eq!(u.out, o);
            assert!(u.min < u.max);
        }
        assert_eq!(VAR_UI.len(), Var::ALL.len());
        assert_eq!(OUT_UI.len(), Out::ALL.len());
    }

    #[test]
    fn default_request_has_one_qol_and_a_herd_cap_only() {
        let r = Request::default();
        assert_eq!(r.vars.iter().map(|(v, _)| *v).collect::<Vec<_>>(), vec![Var::Qol, Var::MaxHeadcount]);
        assert!(r.outs.is_empty());
        assert_eq!(r.objective, Objective::MaxMeat);
        assert_eq!(parse_request(&HashMap::new()), r, "no `cons` marker -> defaults");
    }

    #[test]
    fn constraints_parse_with_operators_and_clamping() {
        let q = form(&[
            ("cons", "1"),
            ("objective", "max_qol"),
            ("on_does", "1"),
            ("op_does", "max"),
            ("val_does", "99"),
            ("on_qol_doe", "1"),
            ("op_qol_doe", "min"),
            ("val_qol_doe", "1.5"),
            ("on_wean_days", "1"),
            ("op_wean_days", "eq"),
            ("val_wean_days", "42"),
            ("on_meat", "1"),
            ("op_meat", "min"),
            ("val_meat", "250"),
            ("on_butcher_rule", "1"),
            ("op_butcher_rule", "max"),
            ("val_butcher_rule", "1"),
        ]);
        let r = parse_request(&q);
        assert_eq!(r.objective, Objective::MaxQol);
        let get = |v: Var| r.vars.iter().find(|(x, _)| *x == v).map(|(_, b)| *b);
        assert_eq!(get(Var::Does), Some(Bound::at_most(24.0)), "clamped to the slider range");
        assert_eq!(get(Var::QolDoe), Some(Bound::at_least(1.5)));
        assert_eq!(get(Var::WeanDays), Some(Bound::exactly(42.0)));
        assert_eq!(get(Var::ButcherRule), Some(Bound::exactly(1.0)), "choices are always equalities");
        assert_eq!(get(Var::Qol), None, "present constraints replace the defaults");
        assert_eq!(r.outs, vec![(Out::MeatKgPerYr, Bound::at_least(250.0))]);
    }

    #[test]
    fn malformed_constraints_are_dropped_not_fatal() {
        let q = form(&[
            ("cons", "1"),
            ("on_does", "1"),
            ("val_does", "lots"),
            ("on_bucks", "1"),
            ("on_tiers", "1"),
            ("val_tiers", "2"),
            ("op_tiers", "???"),
        ]);
        let r = parse_request(&q);
        assert_eq!(r.vars, vec![(Var::Tiers, Bound::exactly(2.0))], "bad value / missing value dropped; bad op falls back to exact");
    }

    #[test]
    fn suggested_floors_are_three_quarters_rounded_down_and_never_overwrite() {
        let mut r = Request::default();
        r.suggest_floors(337.0, 89.0);
        assert_eq!(r.outs, vec![(Out::MeatKgPerYr, Bound::at_least(250.0)), (Out::PeakHead, Bound::at_least(66.0))]);
        r.suggest_floors(1000.0, 500.0);
        assert_eq!(r.outs.len(), 2, "existing limits are kept");
        assert_eq!(r.outs[0].1, Bound::at_least(250.0));
        let mut r = Request::default();
        r.outs.push((Out::PeakHead, Bound::at_most(30.0)));
        r.suggest_floors(337.0, 89.0);
        assert!(r.outs.contains(&(Out::PeakHead, Bound::at_most(30.0))) && r.has("meat"));
    }

    #[test]
    fn herd_cap_only_accepts_at_most() {
        for op in ["min", "eq", "max", "garbage"] {
            let r =
                parse_request(&form(&[("cons", "1"), ("on_max_headcount", "1"), ("op_max_headcount", op), ("val_max_headcount", "40")]));
            assert_eq!(r.vars, vec![(Var::MaxHeadcount, Bound::at_most(40.0))], "op {op}");
        }
        // other inputs keep all three
        let r = parse_request(&form(&[("cons", "1"), ("on_does", "1"), ("op_does", "min"), ("val_does", "3")]));
        assert_eq!(r.vars, vec![(Var::Does, Bound::at_least(3.0))]);
        assert_eq!(allowed_ops(Var::ButcherRule), &[Op::Exactly]);
    }

    #[test]
    fn floor_space_is_limited_only_by_constraints_that_can_bite() {
        let mut r = Request::default();
        assert!(!r.limits_floor_space(), "default QoL minimum and no cap limit nothing");
        r.vars.push((Var::MaxHeadcount, Bound::at_most(40.0)));
        assert!(!r.limits_floor_space(), "a herd cap does not limit room");
        r.vars.push((Var::QolDoe, Bound::at_least(1.5)));
        assert!(!r.limits_floor_space(), "a QoL floor does not limit room");
        r.vars.push((Var::QolBuck, Bound::at_most(1.5)));
        assert!(r.limits_floor_space(), "a QoL ceiling does");
        let mut r = Request::default();
        r.vars.push((Var::Does, Bound::at_least(4.0)));
        assert!(r.limits_floor_space());
        let mut r = Request::default();
        r.outs.push((Out::PeakHead, Bound::at_least(40.0)));
        assert!(r.limits_floor_space());
    }

    #[test]
    fn short_labels_exist_and_units_are_consistent() {
        for v in Var::ALL {
            assert!(!short_label(v).is_empty() && short_label(v).len() <= 26, "{v:?}");
        }
        for o in Out::ALL {
            assert!(!short_out_label(o).is_empty());
        }
        assert_eq!(Fmt::Pct.scale(), 100.0);
        assert_eq!(Fmt::Metres.scale(), 1.0);
        assert_eq!(Fmt::MoneyPerKg.affixes(), ("$", "/kg"));
        assert_eq!(Fmt::Qol.affixes().0, "×");
        // every stored value round-trips through the typed value
        for c in all_ctls() {
            if let Kind::Range { min, max, .. } = c.kind {
                let typed = max * c.fmt.scale();
                assert!((typed / c.fmt.scale() - max).abs() < 1e-9, "{}", c.key);
                assert!(min < max);
            }
        }
    }

    #[test]
    fn op_helpers_round_trip() {
        for op in [Op::AtLeast, Op::Exactly, Op::AtMost] {
            assert_eq!(Op::from_key(op.key()), Some(op));
            assert_eq!(Op::of(&op.bound(3.0)), op);
            assert_eq!(bound_value(&op.bound(3.0)), 3.0);
        }
        assert_eq!(Op::from_key("??"), None);
    }

    #[test]
    fn has_and_remove_cover_vars_and_outs() {
        let mut r = Request::default();
        r.outs.push((Out::PeakHead, Bound::at_most(40.0)));
        assert!(r.has("qol") && r.has("peak_head") && !r.has("does"));
        r.remove("peak_head");
        r.remove("qol");
        assert!(!r.has("qol") && !r.has("peak_head") && r.has("max_headcount"));
    }

    #[test]
    fn value_text_for_outputs_carries_units() {
        assert_eq!(out_ui(Out::MeatKgPerYr).value_text(250.0), "250 kg/yr");
        assert_eq!(out_ui(Out::FryersPerYr).value_text(180.0), "180 /yr");
        assert_eq!(out_ui(Out::PeakHead).value_text(40.0), "40");
        assert_eq!(out_ui(Out::PeakHead).label, "Rabbits on hand (peak)");
        assert_eq!(Fmt::PctPoints.render(10.0), "10%");
        assert_eq!(var_ui(Var::ButcherRule).value_text(1.0), "When room is needed");
    }
}
