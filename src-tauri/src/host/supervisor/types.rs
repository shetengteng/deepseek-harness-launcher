use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

use crate::host::ReadinessError;

#[derive(Debug, Clone)]
pub struct HostExitDetail {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

pub type UnexpectedExitCallback = Arc<dyn Fn(HostExitDetail) + Send + Sync>;

#[derive(Debug, Error)]
pub enum HostSupervisorError {
    #[error("desktop Host cannot start after shutdown")]
    AlreadyShutdown,
    #[error("desktop Host readiness timed out after {0:?}")]
    ReadinessTimeout(Duration),
    #[error("desktop Host failed to spawn: {0}")]
    SpawnFailed(String),
    #[error("desktop Host exited before readiness (code {code:?}, signal {signal:?})")]
    ExitedBeforeReadiness {
        code: Option<i32>,
        signal: Option<i32>,
    },
    #[error("{0}")]
    Readiness(#[from] ReadinessError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

pub struct Shutdown {
    pub(super) inner: tokio::task::JoinHandle<()>,
}

impl Shutdown {
    pub async fn await_completion(self) {
        let _ = self.inner.await;
    }
}
