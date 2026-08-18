//! DeepSeek Harness Launcher shell library.

mod app;
pub mod cli_shim;
mod navigation;
mod platform;
mod tray;

pub mod commands;
pub mod dsh;
pub mod error;
pub mod host;
pub mod logging;
pub mod marketplace;
pub mod node;
pub mod paths;
pub mod state;

pub use app::run;
