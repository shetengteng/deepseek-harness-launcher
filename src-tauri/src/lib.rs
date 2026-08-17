//! DeepSeek Harness Launcher shell library.

mod app;
mod platform;
mod tray;

pub mod commands;
pub mod dsh;
pub mod error;
pub mod host;
pub mod logging;
pub mod node;
pub mod paths;
pub mod state;

pub use app::run;
