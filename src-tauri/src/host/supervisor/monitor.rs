use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use super::{HostExitDetail, SupervisorInner, SupervisorState, UnexpectedExitCallback};
use crate::host::Origin;

pub(super) struct ExitMonitorContext {
    pub(super) inner: Arc<Mutex<SupervisorInner>>,
    pub(super) origin_cache: Arc<Mutex<Option<Origin>>>,
    pub(super) shutdown_flag: Arc<AtomicBool>,
    pub(super) intentional_restart_generation: Arc<AtomicU64>,
    pub(super) exit_handler: Arc<RwLock<Option<UnexpectedExitCallback>>>,
    pub(super) configured_handler: Option<UnexpectedExitCallback>,
}

pub(super) fn spawn_exit_monitor(
    context: ExitMonitorContext,
    generation: u64,
    stdout_join: JoinHandle<()>,
    stderr_join: JoinHandle<()>,
) {
    tokio::spawn(async move {
        let _ = stdout_join.await;
        let _ = stderr_join.await;
        let exit = wait_for_exit(&context.inner, generation).await;
        if context.shutdown_flag.load(Ordering::Acquire)
            || context
                .intentional_restart_generation
                .compare_exchange(generation, 0, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            return;
        }

        *context.origin_cache.lock().await = None;
        context.inner.lock().await.state = SupervisorState::Starting;
        let handler = context
            .exit_handler
            .read()
            .unwrap()
            .clone()
            .or(context.configured_handler);
        if let Some(callback) = handler {
            callback(exit);
        }
    });
}

async fn wait_for_exit(inner: &Arc<Mutex<SupervisorInner>>, generation: u64) -> HostExitDetail {
    let mut inner = inner.lock().await;
    if inner.generation != generation {
        return HostExitDetail {
            code: Some(0),
            signal: None,
        };
    }
    let Some(mut child) = inner.child.take() else {
        return HostExitDetail {
            code: Some(0),
            signal: None,
        };
    };

    match child.wait().await {
        Ok(status) => HostExitDetail {
            code: status.code(),
            #[cfg(unix)]
            signal: {
                use std::os::unix::process::ExitStatusExt;
                status.signal()
            },
            #[cfg(not(unix))]
            signal: None,
        },
        Err(_) => HostExitDetail {
            code: None,
            signal: None,
        },
    }
}
