use std::sync::Arc;
use std::time::Duration;

use super::UnexpectedExitCallback;

pub const DEFAULT_READINESS_TIMEOUT: Duration = Duration::from_millis(90_000);
pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(5_000);

pub type LogCallback = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Clone)]
pub struct HostSupervisorConfig {
    pub readiness_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub log: Option<LogCallback>,
    pub on_unexpected_exit: Option<UnexpectedExitCallback>,
}

impl Default for HostSupervisorConfig {
    fn default() -> Self {
        Self {
            readiness_timeout: DEFAULT_READINESS_TIMEOUT,
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
            log: None,
            on_unexpected_exit: None,
        }
    }
}
