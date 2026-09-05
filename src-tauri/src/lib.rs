//! DeepSeek Harness Launcher shell library.

mod app;
pub mod cli_shim;
mod navigation;
mod platform;
pub(crate) mod tray;

pub mod commands;
pub mod dsh;
pub mod error;
pub mod host;
mod ipc_commands;
pub mod logging;
pub mod node;
pub mod paths;
pub mod state;

pub use app::run;
