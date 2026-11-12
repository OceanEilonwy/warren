//! Growth and dressing tables (typical NZ White figures; not set by the Code).

use std::sync::OnceLock;

const W_TABLE: [(f64, f64); 11] = [
    (0.0, 0.06),
    (28.0, 0.6),
    (42.0, 1.1),
    (56.0, 1.7),
    (70.0, 2.2),
    (84.0, 2.6),
    (98.0, 3.0),
    (112.0, 3.3),
    (140.0, 3.8),
    (180.0, 4.3),
    (365.0, 4.8),
];
const DRESS: [(f64, f64); 3] = [(56.0, 0.5), (84.0, 0.55), (112.0, 0.58)];

fn interp(t: &[(f64, f64)], x: f64) -> f64 {
    if x <= t[0].0 {
        return t[0].1;
    }
    for w in t.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x <= x1 {
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        }
    }
    t[t.len() - 1].1
}

/// Live weight (kg) at `age` days for an animal with individual size multiplier `mult`.
pub fn live_weight(age: f64, mult: f64) -> f64 {
    interp(&W_TABLE, age) * mult
}

/// Dressing-out fraction at `age` days.
pub fn dressing(age: f64) -> f64 {
    interp(&DRESS, age)
}

/// Age (days) at which `Params::dressed_weight_kg` is defined: a typical 12-week fryer.
pub const REFERENCE_DRESSED_AGE_DAYS: f64 = 84.0;

/// Dressed (carcass) weight in kg of the built-in curve at the reference age (2.6 kg live × 55%).
/// `Params::dressed_weight_kg` scales the whole curve relative to this, keeping its shape.
pub fn reference_dressed_kg() -> f64 {
    live_weight(REFERENCE_DRESSED_AGE_DAYS, 1.0) * dressing(REFERENCE_DRESSED_AGE_DAYS)
}

pub const MAX_AGE: usize = 366;

pub struct Tables {
    pub weight: [f64; MAX_AGE],
    pub metabolic: [f64; MAX_AGE],
    pub dress: [f64; MAX_AGE],
}

pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Tables { weight: [0.0; MAX_AGE], metabolic: [0.0; MAX_AGE], dress: [0.0; MAX_AGE] };
        for a in 0..MAX_AGE {
            t.weight[a] = interp(&W_TABLE, a as f64);
            t.metabolic[a] = t.weight[a].powf(0.75);
            t.dress[a] = interp(&DRESS, a as f64);
        }
        t
    })
}

/// Clamp an age in days to a table index.
pub fn age_idx(age: i32) -> usize {
    age.clamp(0, MAX_AGE as i32 - 1) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_documented_growth_points() {
        assert!((live_weight(56.0, 1.0) - 1.7).abs() < 1e-9);
        assert!((live_weight(84.0, 1.0) - 2.6).abs() < 1e-9);
        assert!((live_weight(112.0, 1.0) - 3.3).abs() < 1e-9);
        assert!((dressing(56.0) - 0.50).abs() < 1e-9);
        assert!((dressing(112.0) - 0.58).abs() < 1e-9);
    }

    #[test]
    fn weight_is_monotonic_and_clamped() {
        let mut prev = 0.0;
        for a in 0..400 {
            let w = live_weight(a as f64, 1.0);
            assert!(w >= prev);
            prev = w;
        }
        assert_eq!(live_weight(-5.0, 1.0), live_weight(0.0, 1.0));
        assert_eq!(live_weight(1000.0, 1.0), 4.8);
        assert!((reference_dressed_kg() - 1.43).abs() < 1e-9, "{}", reference_dressed_kg());
        assert_eq!(dressing(10.0), 0.5);
        assert_eq!(dressing(500.0), 0.58);
    }

    #[test]
    fn tables_agree_with_functions() {
        let t = tables();
        for a in [0usize, 30, 77, 200, 365] {
            assert!((t.weight[a] - live_weight(a as f64, 1.0)).abs() < 1e-12);
        }
        assert_eq!(age_idx(-3), 0);
        assert_eq!(age_idx(10_000), MAX_AGE - 1);
    }
}
