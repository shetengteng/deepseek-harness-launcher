//! Host readiness-line parsing.

mod line;
mod parser;
#[cfg(test)]
mod tests;

pub use line::{parse_readiness_line, Origin, ReadinessError, READINESS_PREFIX};
pub use parser::ReadinessParser;

/// Startup stdout buffer capacity.
pub const MAX_STARTUP_OUTPUT_CHARS: usize = 32_768;
