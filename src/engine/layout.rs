//! Cage layout: every cage has its own size and position.

use super::params::{Params, code};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CageKind {
    Doe,
    Buck,
    Grow,
}

#[derive(Clone, Debug)]
pub struct Cage {
    pub kind: CageKind,
    pub tier: usize,
    pub row: usize,
    /// Distance along the width from the left end of the row (m).
    pub x: f64,
    pub w: f64,
    pub d: f64,
    pub area: f64,
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub max_tiers: usize,
    pub tiers: usize,
    pub rows: usize,
    pub row_depth: f64,
    pub unused_depth: f64,
    /// Area of a doe + litter cage (code x quality of life).
    pub doe_area: f64,
    /// Area of an adult breeder cage (code x quality of life).
    pub buck_area: f64,
    pub cages: Vec<Cage>,
    pub n_doe: usize,
    pub n_buck: usize,
    pub n_grow: usize,
    pub issues: Vec<String>,
    pub feasible: bool,
}

impl Layout {
    pub fn n_cages(&self) -> usize {
        self.cages.len()
    }
}

/// Each tier x row is a "slot" as long as the space is wide. Doe cages (sized for doe + litter to
/// weaning) are placed first-fit, then buck cages, then the remaining width of every slot is cut into
/// grower cages of about `grower_cage_w`. Spare width is shared out so no floor space is wasted.
pub fn compute_layout(p: &Params) -> Layout {
    let mut issues = Vec::new();
    let tier_h = (p.cage_height_cm + p.tier_overhead_cm) / 100.0;
    let max_tiers = if tier_h > 0.0 { ((p.height_m + 1e-9) / tier_h).floor().max(0.0) as usize } else { 0 };
    let tiers = (p.tiers as usize).min(max_tiers);
    if p.tiers as usize > max_tiers {
        issues.push(format!(
            "Only {max_tiers} tier(s) of {} cm cages + {} cm overhead fit in {} m.",
            p.cage_height_cm, p.tier_overhead_cm, p.height_m
        ));
    }
    let rows = if p.access_both_sides { 2 } else { 1 };
    let row_depth = (p.depth_m / rows as f64).min(p.max_reach_m).max(0.0);
    let unused_depth = (p.depth_m - rows as f64 * row_depth).max(0.0);
    let doe_area = (if p.wean_days <= 35 { code::DOE_LITTER_5WK } else { code::DOE_LITTER_8WK }) * p.qol_doe;
    let buck_area = code::BREEDER * p.qol_buck;
    let w_doe = doe_area / row_depth.max(1e-9);
    let w_buck = buck_area / row_depth.max(1e-9);
    let n_slots = if row_depth > 0.0 { tiers * rows } else { 0 };
    let mut used = vec![0.0f64; n_slots];
    let mut slots: Vec<Vec<Cage>> = vec![Vec::new(); n_slots];

    let mut place = |kind: CageKind, w: f64, n: u32| -> usize {
        let mut placed = 0;
        for _ in 0..n {
            let Some(s) = used.iter().position(|u| u + w <= p.width_m + 1e-9) else { break };
            slots[s].push(Cage { kind, tier: s / rows, row: s % rows, x: 0.0, w, d: row_depth, area: w * row_depth });
            used[s] += w;
            placed += 1;
        }
        placed
    };
    let n_doe = place(CageKind::Doe, w_doe, p.does);
    let n_buck = place(CageKind::Buck, w_buck, p.bucks);
    if n_doe < p.does as usize {
        issues.push(format!("Only {n_doe} of {} doe cages ({doe_area:.2} m² each) fit.", p.does));
    }
    if n_buck < p.bucks as usize {
        issues.push(format!("Only {n_buck} of {} buck cages ({buck_area:.2} m² each) fit.", p.bucks));
    }

    let min_grow_area = (3.0 * code::REST_BOARD_M2).max(code::OVER_12WK_GROUP * p.qol_old);
    let min_grow_w = min_grow_area / row_depth.max(1e-9);
    let mut cages = Vec::new();
    for s in 0..n_slots {
        let left = p.width_m - used[s];
        let mut k = (left / p.grower_cage_w.max(min_grow_w) + 1e-9).floor() as usize;
        if k == 0 && left + 1e-9 >= min_grow_w {
            k = 1;
        }
        for _ in 0..k {
            let w = left / k as f64;
            slots[s].push(Cage { kind: CageKind::Grow, tier: s / rows, row: s % rows, x: 0.0, w, d: row_depth, area: w * row_depth });
        }
        if k == 0 && !slots[s].is_empty() && left > 1e-9 {
            let add = left / slots[s].len() as f64;
            for c in &mut slots[s] {
                c.w += add;
                c.area = c.w * row_depth;
            }
        }
        let mut x = 0.0;
        for mut c in std::mem::take(&mut slots[s]) {
            c.x = x;
            x += c.w;
            cages.push(c);
        }
    }
    let n_grow = cages.iter().filter(|c| c.kind == CageKind::Grow).count();
    let feasible = n_doe > 0 && n_buck > 0;
    if n_grow == 0 && !cages.is_empty() {
        issues.push("No room left for grower cages.".into());
    }
    if cages.iter().any(|c| c.kind == CageKind::Grow && c.area + 1e-9 < min_grow_area) {
        issues.push("A grower cage is too small for a 0.1 m² resting board that is at most ⅓ of the floor.".into());
    }
    if p.cage_height_cm < code::MIN_HEIGHT_CM {
        issues.push("Cage height below the 45 cm minimum.".into());
    }
    if cages.is_empty() {
        issues.push("No cages fit in this space.".into());
    }
    Layout { max_tiers, tiers, rows, row_depth, unused_depth, doe_area, buck_area, cages, n_doe, n_buck, n_grow, issues, feasible }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_of(f: impl FnOnce(&mut Params)) -> Layout {
        let mut p = Params::default();
        f(&mut p);
        compute_layout(&p)
    }

    #[test]
    fn default_space_matches_prototype() {
        let l = layout_of(|_| {});
        assert_eq!(l.max_tiers, 2);
        assert_eq!(l.tiers, 2);
        assert_eq!(l.rows, 2);
        assert!((l.row_depth - 0.75).abs() < 1e-9);
        assert_eq!(l.n_doe, 4);
        assert_eq!(l.n_buck, 1);
        assert!(l.feasible);
        assert!(l.issues.is_empty(), "defaults raise no layout issues: {:?}", l.issues);
        assert!(layout_of(|p| p.tiers = 3).issues.iter().any(|i| i.contains("tier")), "asked for 3 tiers, only 2 fit");
    }

    #[test]
    fn cages_tile_each_row_exactly_without_overlap() {
        for qol in [1.0, 1.3, 2.0] {
            let l = layout_of(|p| {
                p.set_all_qol(qol);
                p.does = 3;
            });
            for tier in 0..l.tiers {
                for row in 0..l.rows {
                    let mut cs: Vec<&Cage> = l.cages.iter().filter(|c| c.tier == tier && c.row == row).collect();
                    cs.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
                    let mut x = 0.0;
                    for c in &cs {
                        assert!((c.x - x).abs() < 1e-9, "gap/overlap at x={x}");
                        x += c.w;
                    }
                    if !cs.is_empty() {
                        assert!((x - 2.0).abs() < 1e-6, "row of {tier}/{row} spans {x}");
                    }
                }
            }
        }
    }

    #[test]
    fn every_cage_meets_its_code_area() {
        for qol in [1.0, 1.5, 2.5] {
            let l = layout_of(|p| p.set_all_qol(qol));
            for c in &l.cages {
                match c.kind {
                    CageKind::Doe => assert!(c.area + 1e-9 >= l.doe_area),
                    CageKind::Buck => assert!(c.area + 1e-9 >= code::BREEDER * qol),
                    CageKind::Grow => assert!(c.area + 1e-9 >= code::OVER_12WK_GROUP * qol),
                }
            }
        }
    }

    #[test]
    fn wean_age_sets_doe_cage_size() {
        assert!((layout_of(|p| p.wean_days = 35).doe_area - 0.56).abs() < 1e-9);
        assert!((layout_of(|p| p.wean_days = 36).doe_area - 0.74).abs() < 1e-9);
    }

    #[test]
    fn doe_qol_only_grows_doe_cages() {
        let base = layout_of(|_| {});
        let big = layout_of(|p| p.qol_doe = 1.5);
        assert!((big.doe_area / base.doe_area - 1.5).abs() < 1e-9);
        assert!((big.buck_area - base.buck_area).abs() < 1e-9);
    }

    #[test]
    fn requested_tiers_are_capped_by_the_height_and_a_single_tier_fits_fewer_cages() {
        assert_eq!(layout_of(|p| p.tiers = 1).tiers, 1);
        assert!(layout_of(|p| p.tiers = 1).n_cages() < layout_of(|p| p.tiers = 2).n_cages());
        assert_eq!(layout_of(|p| p.tiers = 5).tiers, 2, "only 2 fit in 1.35 m");
        assert_eq!(
            layout_of(|p| {
                p.tiers = 5;
                p.height_m = 2.0
            })
            .tiers,
            3
        );
    }

    #[test]
    fn one_sided_access_has_one_row() {
        let l = layout_of(|p| p.access_both_sides = false);
        assert_eq!(l.rows, 1);
        assert!(l.unused_depth > 0.7);
    }

    #[test]
    fn too_small_space_is_infeasible_not_a_panic() {
        let l = layout_of(|p| {
            p.width_m = 0.5;
            p.depth_m = 0.4;
        });
        assert!(!l.feasible);
        let l = layout_of(|p| p.height_m = 0.3);
        assert_eq!(l.n_cages(), 0);
        assert!(!l.feasible);
    }

    #[test]
    fn requesting_more_does_than_fit_places_what_fits() {
        let l = layout_of(|p| p.does = 24);
        assert!(l.n_doe < 24 && l.n_doe > 0);
        assert!(l.issues.iter().any(|i| i.contains("doe cages")));
    }
}
