mod fallback;
mod spawn;
mod status;

#[cfg(test)]
mod test_support;

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::error::{LauncherError, Result};
use crate::host::{HostSupervisor, HostSupervisorConfig, HostSupervisorError};
use crate::state::{AppState, StateStatus};

pub(crate) use spawn::build_spawn_options;
pub use status::{build_status_snapshot, host_platform_arch, StatusSnapshot};

use self::fallback::{
    current_dsh_version, rollback_failed_dsh_update, start_with_one_known_good_fallback,
};

pub struct SharedState {
    pub supervisor: Arc<HostSupervisor>,
    pub(crate) navigation: crate::navigation::NavigationPolicy,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            supervisor: Arc::new(HostSupervisor::new(HostSupervisorConfig::default())),
            navigation: crate::navigation::NavigationPolicy::default(),
        }
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub async fn launcher_status() -> Result<StatusSnapshot> {
    status::load_runtime_status().await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DshUpgradeRestartResult {
    pub origin: String,
    pub active_version: String,
    pub rolled_back: bool,
}

#[tauri::command]
pub async fn start_host(app: AppHandle, state: State<'_, SharedState>) -> Result<String> {
    crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Starting);
    let result = start_with_one_known_good_fallback(|| start_host_inner(&state.supervisor)).await;
    match result {
        Ok(origin) => {
            state.navigation.activate_dsh_origin(&origin);
            crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Running);
            Ok(origin)
        }
        Err(error) => {
            crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Failed);
            Err(error)
        }
    }
}

async fn start_host_inner(supervisor: &Arc<HostSupervisor>) -> Result<String> {
    let origin = supervisor
        .start(&build_spawn_options().await?)
        .await
        .map_err(map_host_error)?;
    reset_crash_counter_after_manual_start();
    Ok(origin.as_str().to_string())
}

pub async fn restart_host_inner(supervisor: &Arc<HostSupervisor>) -> Result<String> {
    let origin = supervisor
        .restart(&build_spawn_options().await?)
        .await
        .map_err(map_host_error)?;
    reset_crash_counter_after_manual_start();
    Ok(origin.as_str().to_string())
}

#[tauri::command]
pub async fn restart_host(app: AppHandle, state: State<'_, SharedState>) -> Result<String> {
    crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Restarting);
    state.navigation.clear_dsh_origin();
    let result = start_with_one_known_good_fallback(|| restart_host_inner(&state.supervisor)).await;
    match result {
        Ok(origin) => {
            state.navigation.activate_dsh_origin(&origin);
            crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Running);
            Ok(origin)
        }
        Err(error) => {
            crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Failed);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn restart_host_after_dsh_update(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<DshUpgradeRestartResult> {
    crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Restarting);
    state.navigation.clear_dsh_origin();
    let result = async {
        let active_version = current_dsh_version()?;
        match restart_host_inner(&state.supervisor).await {
            Ok(origin) => {
                state.navigation.activate_dsh_origin(&origin);
                Ok(DshUpgradeRestartResult {
                    origin,
                    active_version,
                    rolled_back: false,
                })
            }
            Err(restart_error) => {
                let restart_error = restart_error.to_string();
                let active_version = rollback_failed_dsh_update()?;
                let origin = restart_host_inner(&state.supervisor)
                    .await
                    .map_err(|rollback_error| {
                        LauncherError::Host(format!(
                            "dsh update restart failed: {restart_error}; rollback restart failed: {rollback_error}"
                        ))
                    })?;
                state.navigation.activate_dsh_origin(&origin);
                tracing::warn!(%restart_error, %active_version, "dsh update failed to start; restored known-good version");
                Ok(DshUpgradeRestartResult {
                    origin,
                    active_version,
                    rolled_back: true,
                })
            }
        }
    }
    .await;
    match result {
        Ok(result) => {
            crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Running);
            Ok(result)
        }
        Err(error) => {
            crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Failed);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn shutdown_host(app: AppHandle, state: State<'_, SharedState>) -> Result<()> {
    crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Stopping);
    state.navigation.clear_dsh_origin();
    let shutdown = state.supervisor.shutdown().await;
    shutdown.await_completion().await;
    crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Stopped);
    Ok(())
}

fn reset_crash_counter_after_manual_start() {
    if let Ok(StateStatus::Loaded(mut state)) = AppState::load() {
        crate::host::reset_crash_counter(&mut state);
        if let Err(error) = state.save() {
            tracing::warn!(%error, "failed to save reset crash counter");
        }
    }
}

fn map_host_error(error: HostSupervisorError) -> LauncherError {
    LauncherError::Host(error.to_string())
}
