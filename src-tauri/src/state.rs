//! Persistent launcher state facade.

mod persistence;
mod types;

pub use persistence::{load_from, save_to};
pub use types::{AppState, BootstrapPlan, DshState, InstalledDsh, NodeState, StateStatus};
