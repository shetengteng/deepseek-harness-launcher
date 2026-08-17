//! Single-owner dsh web process supervisor.

mod config;
mod monitor;
mod output;
mod reader;
#[cfg(test)]
mod tests;
mod types;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use tokio::process::Child;
use tokio::sync::Mutex;
use tokio::time::timeout;

use self::monitor::spawn_exit_monitor;
use self::reader::start_output_tasks;
pub use config::{
    HostSupervisorConfig, LogCallback, DEFAULT_READINESS_TIMEOUT, DEFAULT_SHUTDOWN_TIMEOUT,
};
pub use types::{HostExitDetail, HostSupervisorError, Shutdown, UnexpectedExitCallback};

use super::lifecycle::{spawn_dsh_web, SpawnDshWebOptions};
use super::readiness::{Origin, ReadinessParser};

pub(super) struct SupervisorInner {
    child: Option<Child>,
    state: SupervisorState,
    generation: u64,
}

#[derive(Default, PartialEq, Copy, Clone)]
pub(super) enum SupervisorState {
    #[default]
    Starting,
    Ready,
    Shutdown,
}

pub struct HostSupervisor {
    inner: Arc<Mutex<SupervisorInner>>,
    origin_cache: Arc<Mutex<Option<Origin>>>,
    shutdown_flag: Arc<AtomicBool>,
    intentional_restart_generation: Arc<AtomicU64>,
    output: Arc<Mutex<String>>,
    config: HostSupervisorConfig,
    exit_handler: Arc<RwLock<Option<UnexpectedExitCallback>>>,
}

impl HostSupervisor {
    pub fn new(config: HostSupervisorConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SupervisorInner {
                child: None,
                state: SupervisorState::Starting,
                generation: 0,
            })),
            origin_cache: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            intentional_restart_generation: Arc::new(AtomicU64::new(0)),
            output: Arc::new(Mutex::new(String::new())),
            config,
            exit_handler: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_exit_handler(&self, handler: UnexpectedExitCallback) {
        *self.exit_handler.write().unwrap() = Some(handler);
    }

    pub async fn start(
        self: &Arc<Self>,
        options: &SpawnDshWebOptions,
    ) -> Result<Origin, HostSupervisorError> {
        if self.shutdown_flag.load(Ordering::Acquire) {
            return Err(HostSupervisorError::AlreadyShutdown);
        }

        let mut cache = self.origin_cache.lock().await;
        if let Some(origin) = cache.as_ref() {
            return Ok(origin.clone());
        }
        let origin = self.start_inner(options).await?;
        *cache = Some(origin.clone());
        Ok(origin)
    }

    async fn start_inner(
        self: &Arc<Self>,
        options: &SpawnDshWebOptions,
    ) -> Result<Origin, HostSupervisorError> {
        let mut child = spawn_dsh_web(options)
            .map_err(|error| HostSupervisorError::SpawnFailed(error.to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| HostSupervisorError::Other("stdout not piped".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| HostSupervisorError::Other("stderr not piped".into()))?;
        let parser = new_startup_parser();
        let (ready_rx, stdout_task, stderr_task) = start_output_tasks(
            stdout,
            stderr,
            parser,
            Arc::clone(&self.output),
            self.config.log.clone(),
        );

        let origin = match timeout(self.config.readiness_timeout, ready_rx).await {
            Ok(Ok(Ok(origin))) => origin,
            Ok(Ok(Err(error))) => {
                let _ = child.kill().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(HostSupervisorError::Readiness(error));
            }
            Ok(Err(_)) => {
                let _ = child.kill().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(HostSupervisorError::Other(
                    "readiness channel closed unexpectedly".into(),
                ));
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(HostSupervisorError::ReadinessTimeout(
                    self.config.readiness_timeout,
                ));
            }
        };

        let generation = {
            let mut inner = self.inner.lock().await;
            inner.generation = inner.generation.wrapping_add(1).max(1);
            inner.child = Some(child);
            inner.state = SupervisorState::Ready;
            inner.generation
        };
        spawn_exit_monitor(
            Arc::clone(&self.inner),
            Arc::clone(&self.origin_cache),
            Arc::clone(&self.shutdown_flag),
            Arc::clone(&self.intentional_restart_generation),
            Arc::clone(&self.exit_handler),
            self.config.on_unexpected_exit.clone(),
            generation,
            stdout_task,
            stderr_task,
        );
        Ok(origin)
    }

    pub async fn restart(
        self: &Arc<Self>,
        options: &SpawnDshWebOptions,
    ) -> Result<Origin, HostSupervisorError> {
        if self.shutdown_flag.load(Ordering::Acquire) {
            return Err(HostSupervisorError::AlreadyShutdown);
        }

        {
            let mut inner = self.inner.lock().await;
            self.intentional_restart_generation
                .store(inner.generation, Ordering::Release);
            if let Some(mut child) = inner.child.take() {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
            inner.state = SupervisorState::Starting;
        }
        *self.origin_cache.lock().await = None;
        self.start(options).await
    }

    pub async fn shutdown(&self) -> Shutdown {
        self.shutdown_flag.store(true, Ordering::Release);
        let inner = Arc::clone(&self.inner);
        let join = tokio::spawn(async move {
            let mut guard = inner.lock().await;
            guard.state = SupervisorState::Shutdown;
            if let Some(mut child) = guard.child.take() {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        });
        Shutdown { inner: join }
    }

    pub async fn output_snapshot(&self) -> String {
        self.output.lock().await.clone()
    }
}

fn new_startup_parser() -> ReadinessParser {
    ReadinessParser::new()
}
