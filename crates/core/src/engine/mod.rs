//! The simulation engine: no web or UI dependencies.

pub mod bio;
pub mod constraints;
pub mod layout;
pub mod params;
pub mod progress;
pub mod rng;
pub mod sim;
pub mod solve;
pub mod summary;

pub use constraints::{Bound, Objective, Out, Problem, Solution, Subject, Var, Violation};
pub use layout::{Cage, CageKind, Layout, compute_layout};
pub use params::{ButcherRule, Params, Replacement, code};
pub use progress::Progress;
pub use sim::{CageRole, CageSnap, simulate_run};
pub use solve::{MAX_FORCED_SHARE, OptConfig, optimise, solve, solve_tracked};
pub use summary::{MetricKey, Stat, Summary, simulate, simulate_runs, simulate_runs_tracked};
