use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use super::{HostExitDetail, SupervisorInner, SupervisorState, UnexpectedExitCallback};
use crate::host::Origin;

pub(super) fn spawn_exit_monitor(
    inner: Arc<Mutex<SupervisorInner>>,
    origin_cache: Arc<Mutex<Option<Origin>>>,
    shutdown_flag: Arc<AtomicBool>,
    intentional_restart_generation: Arc<AtomicU64>,
    exit_handler: Arc<RwLock<Option<UnexpectedExitCallback>>>,
    configured_handler: Option<UnexpectedExitCallback>,
    generation: u64,
    stdout_join: JoinHandle<()>,
    stderr_join: JoinHandle<()>,
) {
    tokio::spawn(async move {
        let _ = stdout_join.await;
        let _ = stderr_join.await;
        let exit = wait_for_exit(&inner, generation).await;
        if shutdown_flag.load(Ordering::Acquire)
            || intentional_restart_generation
                .compare_exchange(generation, 0, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            return;
        }

        *origin_cache.lock().await = None;
        inner.lock().await.state = SupervisorState::Starting;
        let handler = exit_handler.read().unwrap().clone().or(configured_handler);
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
