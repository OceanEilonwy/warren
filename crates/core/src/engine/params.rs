//! All user-adjustable levers and biology assumptions, with strong types.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Replacement {
    /// Keep replacement breeders from your own litters.
    Own,
    /// Buy replacement breeders in.
    Buy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButcherRule {
    /// Butcher at a fixed age.
    Age,
    /// Butcher the oldest growers whenever cages fill, and everyone by the butchering age.
    Room,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    // ---- Space ----
    pub width_m: f64,
    pub depth_m: f64,
    pub height_m: f64,
    /// Can you reach in from both long sides?
    pub access_both_sides: bool,
    /// Max practical cage depth from one side.
    pub max_reach_m: f64,
    /// Mesh floor + droppings tray + clearance per tier.
    pub tier_overhead_cm: f64,
    /// Interior height (Code: >= 45 cm for rabbits > 12 wk).
    pub cage_height_cm: f64,
    /// Floor taken by the nest box in each doe cage (m²), added on top of the Code area for doe + litter.
    pub nest_box_m2: f64,
    /// Requested tiers (clamped to what fits).
    pub tiers: u32,
    /// Target width of grower cages (m).
    pub grower_cage_w: f64,
    // ---- Welfare ----
    pub qol_doe: f64,
    pub qol_buck: f64,
    pub qol_young: f64,
    pub qol_old: f64,
    // ---- Herd ----
    pub does: u32,
    pub bucks: u32,
    /// 0 = no cap; counts every rabbit incl. unweaned kits.
    pub max_headcount: u32,
    // ---- Management ----
    pub breed_age_days: u32,
    pub rebreed_days: u32,
    pub wean_days: u32,
    pub butcher_days: u32,
    pub butcher_rule: ButcherRule,
    pub min_butcher_days: u32,
    pub sex_separate_days: u32,
    pub replacement: Replacement,
    pub purchase_age_days: u32,
    pub breeder_life_years: f64,
    // ---- Feed & costs ----
    pub pellet_price: f64,
    pub hay_price: f64,
    pub wastage_pct: f64,
    pub breeder_price: f64,
    // ---- Biology (assumptions, not from the Code) ----
    /// Average dressed (carcass) weight of a rabbit butchered at 12 weeks, in kg. Scales the whole
    /// growth curve: meat per rabbit and feed eaten.
    pub dressed_weight_kg: f64,
    pub gestation_days: u32,
    /// Share of matings that take when the doe is not nursing a litter.
    pub conception_rate: f64,
    /// Share of matings that take when the doe is still nursing a litter (early re-mating).
    pub nursing_conception_rate: f64,
    pub retest_days: u32,
    pub litter_mean: f64,
    pub pre_wean_mortality: f64,
    /// Share of growers lost between 6 and 12 weeks; the same daily risk applies at every age after weaning.
    pub post_wean_mortality: f64,
    pub breeder_mortality: f64,
    pub pellet_rate_grower: f64,
    pub pellet_rate_adult: f64,
    pub lactation_per_kit: f64,
    pub hay_rate: f64,
    // ---- Simulation ----
    pub years: u32,
    pub warmup_days: u32,
    pub runs: u32,
    pub seed: u64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            width_m: 2.0,
            depth_m: 1.5,
            height_m: 1.35,
            access_both_sides: true,
            max_reach_m: 0.75,
            tier_overhead_cm: 15.0,
            cage_height_cm: 45.0,
            nest_box_m2: 0.12,
            tiers: 2,
            grower_cage_w: 1.0,
            qol_doe: 1.0,
            qol_buck: 1.0,
            qol_young: 1.0,
            qol_old: 1.0,
            does: 4,
            bucks: 1,
            max_headcount: 0,
            breed_age_days: 150,
            rebreed_days: 28,
            wean_days: 42,
            butcher_days: 77,
            butcher_rule: ButcherRule::Age,
            min_butcher_days: 63,
            sex_separate_days: 84,
            replacement: Replacement::Own,
            purchase_age_days: 84,
            breeder_life_years: 3.0,
            pellet_price: 1.5,
            hay_price: 0.8,
            wastage_pct: 10.0,
            breeder_price: 40.0,
            dressed_weight_kg: 1.0,
            gestation_days: 31,
            conception_rate: 0.75,
            nursing_conception_rate: 0.6,
            retest_days: 14,
            litter_mean: 8.0,
            pre_wean_mortality: 0.15,
            post_wean_mortality: 0.04,
            breeder_mortality: 0.1,
            pellet_rate_grower: 85.0,
            pellet_rate_adult: 45.0,
            lactation_per_kit: 25.0,
            hay_rate: 20.0,
            years: 5,
            warmup_days: 364,
            runs: 300,
            seed: 1,
        }
    }
}

impl Params {
    /// Set all four per-type quality-of-life multipliers together.
    pub fn set_all_qol(&mut self, v: f64) {
        self.qol_doe = v;
        self.qol_buck = v;
        self.qol_young = v;
        self.qol_old = v;
    }
}

/// WA Code of Practice for keeping rabbits, Section 3 (medium breeds e.g. NZ White), m².
pub mod code {
    pub const DOE_LITTER_5WK: f64 = 0.56;
    pub const DOE_LITTER_8WK: f64 = 0.74;
    pub const GROWER_5_TO_12WK: f64 = 0.07;
    pub const OVER_12WK_GROUP: f64 = 0.18;
    pub const BREEDER: f64 = 0.56;
    pub const MIN_HEIGHT_CM: f64 = 45.0;
    pub const REST_BOARD_M2: f64 = 0.1;
}
