//! Daily stochastic simulation of one run of the herd.

use super::bio::{age_idx, reference_dressed_kg, tables};
use super::layout::{CageKind, Layout};
use super::params::{ButcherRule, Params, Replacement, code};
use super::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sex {
    F,
    M,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CageRole {
    Empty,
    Doe,
    DoeLitter,
    Buck,
    Replacement,
    Growers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CageSnap {
    pub role: CageRole,
    /// Rabbits in the cage (for `DoeLitter`, the doe plus her kits).
    pub count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Weekly {
    pub head: Vec<f64>,
    pub growers: Vec<f64>,
    pub cages: Vec<f64>,
    pub meat: Vec<f64>,
    pub cost: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct RunResult {
    pub weekly: Weekly,
    /// Young rabbits butchered per year, planned or forced (forced is a subset).
    pub fryers_per_yr: f64,
    /// Fryers + forced culls + retired breeders, dressed kg.
    pub meat_kg_per_yr: f64,
    pub fryer_kg_per_yr: f64,
    pub cull_kg_per_yr: f64,
    /// Of `fryers_per_yr`, those butchered before the planned/minimum age for lack of room.
    pub forced_per_yr: f64,
    pub pellet_kg_per_yr: f64,
    pub hay_kg_per_yr: f64,
    pub feed_cost_per_yr: f64,
    pub buy_cost_per_yr: f64,
    pub peak_head: f64,
    /// Mean fraction of cages occupied (post warm-up).
    pub cage_use: f64,
    pub litters_per_doe_yr: f64,
    pub butcher_age_wk: f64,
    pub snapshot: Vec<CageSnap>,
}

/// `Params::post_wean_mortality` is the share of growers lost over this many days (6 to 12 weeks of age).
pub const GROWER_LOSS_SPAN_DAYS: f64 = 42.0;

/// Youngest home-bred grower that can be picked as a replacement breeder (8 weeks).
const MIN_REPLACEMENT_AGE: i32 = 56;

/// Parameters the run actually uses after the layout has been applied.
#[derive(Clone, Copy, Debug)]
pub struct Effective {
    pub wean: u32,
    pub does: usize,
    pub bucks: usize,
}

pub fn effective(p: &Params, l: &Layout) -> Effective {
    Effective { wean: p.wean_days.clamp(28, 56), does: l.n_doe, bucks: l.n_buck }
}

#[derive(Clone, Copy)]
struct Litter {
    born: i32,
    n: u32,
    wean_day: i32,
}

#[derive(Clone, Copy)]
struct Grower {
    sex: Sex,
    born: i32,
    wm: f64,
    wm75: f64,
    repl: bool,
}

struct Doe {
    born: i32,
    wm: f64,
    wm75: f64,
    pregnant: bool,
    due: i32,
    next_breed: i32,
    litter: Option<Litter>,
    misses: u32,
    since: i32,
}

struct Buck {
    born: i32,
    wm: f64,
    wm75: f64,
    since: i32,
}

struct Purchase {
    arrive: i32,
    sex: Sex,
}

struct Sim<'a> {
    p: &'a Params,
    l: &'a Layout,
    eff: Effective,
    rng: Rng,
    does: Vec<Doe>,
    bucks: Vec<Buck>,
    growers: Vec<Grower>,
    purchases: Vec<Purchase>,
    doe_idx: Vec<usize>,
    buck_idx: Vec<usize>,
    t: i32,
    post: bool,
    day_meat: f64,
    // counters (post warm-up only)
    fryers: f64,
    fryer_kg: f64,
    cull_kg: f64,
    forced: f64,
    age_sum: f64,
    age_n: f64,
}

impl<'a> Sim<'a> {
    fn mk(&mut self, sex: Sex, born: i32, repl: bool) -> Grower {
        // individual size variation, on top of the herd's average size
        let wm = (1.0 + 0.08 * self.rng.normal()).max(0.75) * (self.p.dressed_weight_kg / reference_dressed_kg());
        Grower { sex, born, wm, wm75: wm.powf(0.75), repl }
    }

    fn wean_litter(&mut self, litter: Litter) {
        for _ in 0..litter.n {
            let sex = if self.rng.chance(0.5) { Sex::F } else { Sex::M };
            let g = self.mk(sex, litter.born, false);
            self.growers.push(g);
        }
    }

    fn slaughter_kg(age: i32, wm: f64) -> f64 {
        let tb = tables();
        let i = age_idx(age);
        tb.weight[i] * wm * tb.dress[i]
    }

    fn req_grower(&self, age: i32) -> f64 {
        if age < 84 { code::GROWER_5_TO_12WK * self.p.qol_young } else { code::OVER_12WK_GROUP * self.p.qol_old }
    }

    fn single_age(g: &Grower) -> i32 {
        if g.sex == Sex::M { 70 } else { 84 }
    }

    fn kits_now(&self) -> u32 {
        self.does.iter().map(|d| d.litter.map_or(0, |l| l.n)).sum()
    }

    fn head(&self) -> u32 {
        (self.does.len() + self.bucks.len() + self.growers.len()) as u32 + self.kits_now()
    }

    /// Butcher the oldest rabbit to free space. Returns false if there is nobody to butcher.
    fn cull_oldest(&mut self) -> bool {
        let mut idx: Option<usize> = None;
        for (i, g) in self.growers.iter().enumerate() {
            if !g.repl && idx.is_none_or(|j| g.born < self.growers[j].born) {
                idx = Some(i);
            }
        }
        if idx.is_none() {
            for (i, g) in self.growers.iter().enumerate() {
                if idx.is_none_or(|j| g.born < self.growers[j].born) {
                    idx = Some(i);
                }
            }
        }
        let Some(idx) = idx else { return false };
        let g = self.growers.remove(idx);
        let age = self.t - g.born;
        let kg = Self::slaughter_kg(age, g.wm);
        let planned = self.p.butcher_rule == ButcherRule::Room && !g.repl && age >= self.p.min_butcher_days as i32;
        if self.post {
            self.fryers += 1.0;
            if !planned {
                self.forced += 1.0
            }
            self.fryer_kg += kg;
            self.age_sum += age as f64;
            self.age_n += 1.0;
        }
        self.day_meat += kg;
        true
    }

    /// Code-compliant packing of everyone into the cages that exist. Works in `sc`, which is reused from
    /// day to day: this runs at least once per simulated day, and allocating here made multi-threaded
    /// WASM crawl (its allocator takes a global spin lock).
    #[allow(clippy::needless_range_loop)] // parallel per-cage arrays are indexed together
    fn pack(&self, sc: &mut Scratch) -> Packed {
        let t = self.t;
        let n_c = self.l.n_cages();
        let Scratch { role, cnt, free, tag, solos, items } = sc;
        role.clear();
        role.resize(n_c, CageRole::Empty);
        cnt.clear();
        cnt.resize(n_c, 0);
        free.clear();
        free.extend(self.l.cages.iter().map(|c| c.area));
        tag.clear();
        tag.resize(n_c, None);
        solos.clear();
        items.clear();
        let mut unplaced = 0usize;
        for (i, d) in self.does.iter().enumerate() {
            let Some(&c) = self.doe_idx.get(i) else {
                unplaced += 1;
                continue;
            };
            role[c] = if d.litter.is_some() { CageRole::DoeLitter } else { CageRole::Doe };
            cnt[c] = 1 + d.litter.map_or(0, |l| l.n);
            free[c] = 0.0;
        }
        for i in 0..self.bucks.len() {
            let Some(&c) = self.buck_idx.get(i) else {
                unplaced += 1;
                continue;
            };
            role[c] = CageRole::Buck;
            cnt[c] = 1;
            free[c] = 0.0;
        }
        for g in &self.growers {
            let age = t - g.born;
            if g.repl && age >= Self::single_age(g) { solos.push(age) } else { items.push((age, g.sex)) }
        }
        // replacements housed alone: smallest empty cage that meets the area
        solos.sort_by(|a, b| b.cmp(a));
        for &age in solos.iter() {
            // housed alone from 10-12 wk: never less than the 12 wk+ area, even if a little younger
            let req = self.req_grower(age).max(code::OVER_12WK_GROUP * self.p.qol_old);
            let mut best: Option<usize> = None;
            for c in 0..n_c {
                if role[c] == CageRole::Empty
                    && self.l.cages[c].area + 1e-9 >= req
                    && best.is_none_or(|b| self.l.cages[c].area < self.l.cages[b].area)
                {
                    best = Some(c);
                }
            }
            match best {
                Some(b) => {
                    role[b] = CageRole::Replacement;
                    cnt[b] = 1;
                    free[b] = 0.0;
                }
                None => unplaced += 1,
            }
        }
        // everyone else: best-fit by area, single-sex once old enough; open the biggest empty cage when needed
        items.sort_by_key(|a| std::cmp::Reverse(a.0));
        for &(age, sex) in items.iter() {
            let req = self.req_grower(age);
            let old = age >= self.p.sex_separate_days as i32;
            let mut best: Option<usize> = None;
            for c in 0..n_c {
                if role[c] != CageRole::Growers || free[c] + 1e-9 < req {
                    continue;
                }
                if old && tag[c].is_some_and(|s| s != sex) {
                    continue;
                }
                if best.is_none_or(|b| free[c] < free[b]) {
                    best = Some(c);
                }
            }
            if best.is_none() {
                for c in 0..n_c {
                    if role[c] == CageRole::Empty
                        && self.l.cages[c].area + 1e-9 >= req
                        && best.is_none_or(|b| self.l.cages[c].area > self.l.cages[b].area)
                    {
                        best = Some(c);
                    }
                }
                match best {
                    Some(b) => role[b] = CageRole::Growers,
                    None => {
                        unplaced += 1;
                        continue;
                    }
                }
            }
            let b = best.unwrap();
            free[b] -= req;
            cnt[b] += 1;
            if old {
                tag[b] = Some(sex)
            }
        }
        let used = role.iter().filter(|r| **r != CageRole::Empty).count();
        Packed { unplaced, used }
    }
}

struct Packed {
    unplaced: usize,
    used: usize,
}

/// Buffers `Sim::pack` works in, kept between days.
#[derive(Default)]
struct Scratch {
    role: Vec<CageRole>,
    cnt: Vec<u32>,
    free: Vec<f64>,
    tag: Vec<Option<Sex>>,
    solos: Vec<i32>,
    items: Vec<(i32, Sex)>,
}

impl Scratch {
    /// What is in each cage after the last `pack`.
    fn snapshot(&self) -> Vec<CageSnap> {
        self.role.iter().zip(&self.cnt).map(|(&role, &count)| CageSnap { role, count }).collect()
    }
}

/// Days simulated for `p.years`: whole weeks.
pub fn sim_days(p: &Params) -> i32 {
    ((p.years as f64 * 365.0 / 7.0).floor() as i32) * 7
}

/// First day counted in annual figures. Annual figures need at least a year after start-up, so a long
/// start-up is shortened if needed.
pub fn counted_from_day(p: &Params) -> i32 {
    (p.warmup_days as i32).min(sim_days(p) - 364).max(0)
}

pub fn simulate_run(p: &Params, l: &Layout, seed: u64) -> RunResult {
    let tb = tables();
    let eff = effective(p, l);
    let days = sim_days(p);
    let weeks = ((days as f64) / 7.0).ceil() as usize;
    let mut weekly = Weekly {
        head: vec![0.0; weeks],
        growers: vec![0.0; weeks],
        cages: vec![0.0; weeks],
        meat: vec![0.0; weeks],
        cost: vec![0.0; weeks],
    };
    let mut doe_idx = Vec::new();
    let mut buck_idx = Vec::new();
    for (i, c) in l.cages.iter().enumerate() {
        match c.kind {
            CageKind::Doe => doe_idx.push(i),
            CageKind::Buck => buck_idx.push(i),
            CageKind::Grow => {}
        }
    }
    let mut s = Sim {
        p,
        l,
        eff,
        rng: Rng::new(seed.wrapping_mul(2_654_435_761).wrapping_add(12_345)),
        does: Vec::new(),
        bucks: Vec::new(),
        growers: Vec::new(),
        purchases: Vec::new(),
        doe_idx,
        buck_idx,
        t: 0,
        post: false,
        day_meat: 0.0,
        fryers: 0.0,
        fryer_kg: 0.0,
        cull_kg: 0.0,
        forced: 0.0,
        age_sum: 0.0,
        age_n: 0.0,
    };
    // kit losses on days 1..=21 after birth
    let h_kit = 1.0 - (1.0 - p.pre_wean_mortality).powf(1.0 / 21.0);
    // grower losses are given for 6 to 12 weeks of age; the same daily risk applies at every age after
    // weaning, so keeping growers longer costs more of them (it does not spread a fixed loss thinner)
    let h_grow = 1.0 - (1.0 - p.post_wean_mortality).powf(1.0 / GROWER_LOSS_SPAN_DAYS);
    let h_breed = 1.0 - (1.0 - p.breeder_mortality).powf(1.0 / 365.0);
    let life_days = p.breeder_life_years * 365.0;
    // bought replacements are ordered this many days before a breeder retires, to be ready on time
    let buy_lead_days = ((p.breed_age_days as i32 - p.purchase_age_days as i32).max(0) + 7) as f64;
    // nursing kits have no individual size yet: scale their intake by the herd's average size
    let herd75 = (p.dressed_weight_kg / reference_dressed_kg()).powf(0.75);

    let (mut pellet, mut hay, mut buy_cost, mut peak) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut cage_days, mut counted_days, mut litters, mut doe_days) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut snapshot: Vec<CageSnap> = Vec::new();
    let mut scratch = Scratch::default();
    let mut spare: Vec<Grower> = Vec::new();
    let warm = counted_from_day(p);

    for _ in 0..eff.does {
        s.purchases.push(Purchase { arrive: 0, sex: Sex::F })
    }
    for _ in 0..eff.bucks {
        s.purchases.push(Purchase { arrive: 0, sex: Sex::M })
    }

    for t in 0..days {
        s.t = t;
        s.post = t >= warm;
        s.day_meat = 0.0;
        let post = s.post;
        let wk = (t / 7) as usize;

        // purchases arriving
        for i in (0..s.purchases.len()).rev() {
            if s.purchases[i].arrive <= t {
                let sex = s.purchases[i].sex;
                let g = s.mk(sex, t - p.purchase_age_days as i32, true);
                s.growers.push(g);
                if post {
                    buy_cost += p.breeder_price
                }
                s.purchases.remove(i);
            }
        }

        // breeder deaths & retirement
        for i in (0..s.does.len()).rev() {
            if s.rng.chance(h_breed) {
                if let Some(lit) = s.does[i].litter {
                    // orphaned litter old enough to survive is weaned now (simplification)
                    if t - lit.born >= 28 {
                        s.wean_litter(lit)
                    }
                }
                s.does.remove(i);
                continue;
            }
            let d = &s.does[i];
            let idle = !d.pregnant && d.litter.is_none();
            if idle && ((t - d.since) as f64 >= life_days || d.misses >= 3) {
                let kg = Sim::slaughter_kg(t - d.born, d.wm);
                if post {
                    s.cull_kg += kg
                }
                s.day_meat += kg;
                s.does.remove(i);
            }
        }
        for i in (0..s.bucks.len()).rev() {
            if s.rng.chance(h_breed) {
                s.bucks.remove(i);
                continue;
            }
            if (t - s.bucks[i].since) as f64 >= life_days {
                let kg = Sim::slaughter_kg(t - s.bucks[i].born, s.bucks[i].wm);
                if post {
                    s.cull_kg += kg
                }
                s.day_meat += kg;
                s.bucks.remove(i);
            }
        }

        // promote replacements of breeding age into free breeder places (otherwise they wait)
        for i in (0..s.growers.len()).rev() {
            let g = s.growers[i];
            let free = match g.sex {
                Sex::F => s.does.len() < s.eff.does,
                Sex::M => s.bucks.len() < s.eff.bucks,
            };
            if g.repl && free && t - g.born >= p.breed_age_days as i32 {
                s.growers.remove(i);
                match g.sex {
                    Sex::F => s.does.push(Doe {
                        born: g.born,
                        wm: g.wm,
                        wm75: g.wm75,
                        pregnant: false,
                        due: 0,
                        next_breed: t,
                        litter: None,
                        misses: 0,
                        since: t,
                    }),
                    Sex::M => s.bucks.push(Buck { born: g.born, wm: g.wm, wm75: g.wm75, since: t }),
                }
            }
        }

        // vacancies (and breeders retiring soon) -> replacements
        for sex in [Sex::F, Sex::M] {
            let pend = s.growers.iter().filter(|g| g.repl && g.sex == sex).count() + s.purchases.iter().filter(|x| x.sex == sex).count();
            // oldest suitable home-bred grower (>= 8 wk)
            let oldest = |s: &Sim| -> Option<usize> {
                let mut cand: Option<usize> = None;
                for (i, g) in s.growers.iter().enumerate() {
                    if !g.repl && g.sex == sex && t - g.born >= MIN_REPLACEMENT_AGE && cand.is_none_or(|c| g.born < s.growers[c].born) {
                        cand = Some(i);
                    }
                }
                cand
            };
            // start a replacement just early enough that it reaches breeding age as the breeder retires
            let lead_days = match p.replacement {
                Replacement::Buy => buy_lead_days,
                Replacement::Own => oldest(&s).map_or(0, |c| (p.breed_age_days as i32 - (t - s.growers[c].born)).max(0)) as f64,
            };
            let soon = |since: i32| (t - since) as f64 >= life_days - lead_days;
            let (have, staying, want) = if sex == Sex::F {
                (s.does.len(), s.does.iter().filter(|d| !soon(d.since)).count(), s.eff.does)
            } else {
                (s.bucks.len(), s.bucks.iter().filter(|b| !soon(b.since)).count(), s.eff.bucks)
            };
            let need = want as i64 - staying as i64 - pend as i64;
            for _ in 0..need.max(0) {
                if p.replacement == Replacement::Buy {
                    s.purchases.push(Purchase { arrive: t + 7, sex });
                } else {
                    // buy if a breeder line has died out
                    if let Some(c) = oldest(&s) {
                        s.growers[c].repl = true;
                    } else if have == 0 && s.growers.is_empty() {
                        s.purchases.push(Purchase { arrive: t + 7, sex });
                    }
                }
            }
        }

        // matings
        let mut services = s.bucks.len() as i32 * 2;
        let expected_litter = p.litter_mean.round() as u32;
        // room kept for the new litter: the mean plus two standard deviations (Poisson)
        let reserve = (p.litter_mean + 2.0 * p.litter_mean.sqrt()).ceil() as u32;
        for i in 0..s.does.len() {
            if s.does[i].pregnant || s.does[i].next_breed > t || services <= 0 {
                continue;
            }
            // at the end of her working life a doe is not mated again: she retires once her litter is weaned
            if (t - s.does[i].since) as f64 >= life_days {
                continue;
            }
            // With a headcount cap, only mate a doe if there will be room for her expected litter.
            if p.max_headcount > 0 {
                let coming = s.does.iter().filter(|d| d.pregnant).count() as u32 * expected_litter;
                if s.head() + coming + reserve > p.max_headcount {
                    continue;
                }
            }
            services -= 1;
            // does mated while still nursing a litter conceive less often
            let rate = if s.does[i].litter.is_some() { p.nursing_conception_rate } else { p.conception_rate };
            if s.rng.chance(rate) {
                let jitter = ((s.rng.unit() - 0.5) * 2.0 + 0.5).floor() as i32; // JS-style Math.round
                let d = &mut s.does[i];
                d.pregnant = true;
                d.due = t + p.gestation_days as i32 + jitter;
                d.misses = 0;
                let due = d.due;
                if let Some(lit) = d.litter.as_mut() {
                    let earliest = if t - lit.born >= 28 { t } else { lit.born + 28 };
                    lit.wean_day = earliest.max(lit.wean_day.min(due - 2));
                }
            } else {
                let d = &mut s.does[i];
                d.misses += 1;
                d.next_breed = t + p.retest_days as i32;
            }
        }

        // kindling, kit losses, weaning
        for i in 0..s.does.len() {
            if let Some(mut lit) = s.does[i].litter {
                if t - lit.born <= 21 {
                    lit.n -= s.rng.binomial(lit.n, h_kit)
                }
                s.does[i].litter = Some(lit);
                if t >= lit.wean_day || lit.n == 0 {
                    s.does[i].litter = None;
                    s.wean_litter(lit);
                }
            }
            if s.does[i].pregnant && t >= s.does[i].due {
                if let Some(lit) = s.does[i].litter.take() {
                    s.wean_litter(lit)
                }
                s.does[i].pregnant = false;
                let n = s.rng.poisson(p.litter_mean);
                if post {
                    litters += 1.0
                }
                s.does[i].litter = (n > 0).then_some(Litter { born: t, n, wean_day: t + eff.wean as i32 });
                s.does[i].next_breed = t + p.rebreed_days as i32;
            }
        }

        // grower deaths and butchering
        // walk yesterday's growers into a reused buffer (no allocation once both have grown)
        std::mem::swap(&mut s.growers, &mut spare);
        s.growers.clear();
        for g in spare.drain(..) {
            let age = t - g.born;
            let h = if g.repl && age >= p.butcher_days as i32 { h_breed } else { h_grow };
            if s.rng.chance(h) {
                continue;
            }
            if !g.repl && age >= p.butcher_days as i32 {
                let kg = Sim::slaughter_kg(age, g.wm);
                if post {
                    s.fryers += 1.0;
                    s.fryer_kg += kg;
                    s.age_sum += age as f64;
                    s.age_n += 1.0;
                }
                s.day_meat += kg;
                continue;
            }
            s.growers.push(g);
        }

        // headcount cap
        if p.max_headcount > 0 {
            while s.head() > p.max_headcount && s.cull_oldest() {}
        }

        // cage allocation
        let mut used = 0usize;
        for _ in 0..500 {
            let res = s.pack(&mut scratch);
            if res.unplaced == 0 || !s.cull_oldest() {
                used = res.used;
                if t == days - 1 {
                    snapshot = scratch.snapshot()
                }
                break;
            }
        }

        // feed
        let (mut pg, mut hg) = (0.0f64, 0.0f64);
        for d in &s.does {
            let mm = tb.metabolic[age_idx(t - d.born)] * d.wm75;
            let mut f = p.pellet_rate_adult * mm;
            if d.pregnant && d.due - t <= 10 {
                f *= 1.3
            }
            if let Some(lit) = d.litter {
                let a = t - lit.born;
                if a < 42 {
                    f += p.lactation_per_kit * lit.n as f64
                }
                if a >= 18 {
                    let km = tb.metabolic[age_idx(a)] * herd75;
                    pg += lit.n as f64 * p.pellet_rate_grower * km;
                    hg += lit.n as f64 * p.hay_rate * km;
                }
            }
            pg += f;
            hg += p.hay_rate * mm;
        }
        for b in &s.bucks {
            let mm = tb.metabolic[age_idx(t - b.born)] * b.wm75;
            pg += p.pellet_rate_adult * mm;
            hg += p.hay_rate * mm;
        }
        for g in &s.growers {
            let a = t - g.born;
            let mm = tb.metabolic[age_idx(a)] * g.wm75;
            pg += (if a < 365 { p.pellet_rate_grower } else { p.pellet_rate_adult }) * mm;
            hg += p.hay_rate * mm;
        }
        let waste = 1.0 + p.wastage_pct / 100.0;
        let pk = pg / 1000.0 * waste;
        let hk = hg / 1000.0 * waste;
        let day_cost = pk * p.pellet_price + hk * p.hay_price;

        let h = s.head() as f64;
        if post {
            pellet += pk;
            hay += hk;
            peak = peak.max(h);
            cage_days += used as f64 / l.n_cages().max(1) as f64;
            counted_days += 1.0;
            doe_days += s.does.len() as f64;
        }
        weekly.meat[wk] += s.day_meat;
        weekly.cost[wk] += day_cost;
        weekly.cages[wk] += used as f64 / 7.0;
        if t % 7 == 6 || t == days - 1 {
            weekly.head[wk] = h;
            weekly.growers[wk] = s.growers.len() as f64;
        }
    }

    let yrs = counted_days.max(1.0) / 365.0;
    RunResult {
        weekly,
        fryers_per_yr: s.fryers / yrs,
        fryer_kg_per_yr: s.fryer_kg / yrs,
        cull_kg_per_yr: s.cull_kg / yrs,
        meat_kg_per_yr: (s.fryer_kg + s.cull_kg) / yrs,
        forced_per_yr: s.forced / yrs,
        pellet_kg_per_yr: pellet / yrs,
        hay_kg_per_yr: hay / yrs,
        feed_cost_per_yr: (pellet * p.pellet_price + hay * p.hay_price) / yrs,
        buy_cost_per_yr: buy_cost / yrs,
        peak_head: peak,
        cage_use: cage_days / counted_days.max(1.0),
        litters_per_doe_yr: if doe_days > 0.0 { litters / (doe_days / 365.0) } else { 0.0 },
        butcher_age_wk: if s.age_n > 0.0 { s.age_sum / s.age_n / 7.0 } else { 0.0 },
        snapshot,
    }
}
