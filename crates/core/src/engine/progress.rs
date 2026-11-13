//! How far a solve has got. Rayon threads count finished runs; the solver splits the bar into stages
//! (each lever search, the final report) and estimates each stage's work up front. Readable from any
//! thread: the WASM build hands the address of `shown` to the page, which reads it while the solve runs.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

/// Progress is kept in ten-thousandths.
const FULL: f64 = 10_000.0;

#[derive(Debug, Default)]
pub struct Progress {
    /// Fraction done in ten-thousandths; never goes down until [`Progress::reset`].
    shown: AtomicU32,
    /// The current stage's share of the bar, in ten-thousandths.
    lo: AtomicU32,
    width: AtomicU32,
    /// Work in the current stage, in simulated run-years.
    total: AtomicU64,
    done: AtomicU64,
}

impl Progress {
    /// Start again from zero (a new request).
    pub fn reset(&self) {
        for a in [&self.shown, &self.lo, &self.width] {
            a.store(0, Relaxed);
        }
        self.total.store(1, Relaxed);
        self.done.store(0, Relaxed);
    }

    /// Begin a stage covering `lo .. lo + width` of the bar (fractions of 1) that should take `units`
    /// run-years. Called between parallel steps, never while runs are being counted.
    pub fn stage(&self, lo: f64, width: f64, units: u64) {
        self.lo.store((lo.clamp(0.0, 1.0) * FULL) as u32, Relaxed);
        self.width.store((width.clamp(0.0, 1.0) * FULL) as u32, Relaxed);
        self.total.store(units.max(1), Relaxed);
        self.done.store(0, Relaxed);
        self.publish();
    }

    /// Count finished work (run-years) in the current stage.
    pub fn add(&self, units: u64) {
        self.done.fetch_add(units, Relaxed);
        self.publish();
    }

    fn publish(&self) {
        let total = self.total.load(Relaxed).max(1);
        let done = self.done.load(Relaxed).min(total);
        let f = self.lo.load(Relaxed) as f64 + self.width.load(Relaxed) as f64 * done as f64 / total as f64;
        // never claim to be finished: the caller knows when it is
        self.shown.fetch_max((f as u32).min(FULL as u32 - 1), Relaxed);
    }

    /// Fraction done, 0 to just under 1.
    pub fn fraction(&self) -> f64 {
        self.shown.load(Relaxed) as f64 / FULL
    }

    /// Where the ten-thousandths counter lives (for readers that share this memory, e.g. the browser page).
    pub fn shown_ptr(&self) -> *const u32 {
        self.shown.as_ptr()
    }
}
