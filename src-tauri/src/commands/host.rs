use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::error::{LauncherError, Result};
use crate::host::{HostSupervisor, HostSupervisorConfig, HostSupervisorError, SpawnDshWebOptions};
use crate::state::{AppState, StateStatus};

pub struct SharedState {
    pub supervisor: Arc<HostSupervisor>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            supervisor: Arc::new(HostSupervisor::new(HostSupervisorConfig::default())),
        }
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct StatusSnapshot {
    pub phase: String,
    pub host_origin: Option<String>,
    pub dsh_version: Option<String>,
    pub node_version: Option<String>,
    pub platform: String,
    pub arch: String,
}

pub fn host_platform_arch() -> (&'static str, &'static str) {
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win",
        _ => "linux",
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "x64",
    };
    (platform, arch)
}

#[tauri::command]
pub async fn launcher_status() -> Result<StatusSnapshot> {
    Ok(build_status_snapshot(AppState::load()?))
}

pub fn build_status_snapshot(status: StateStatus) -> StatusSnapshot {
    let (platform, arch) = host_platform_arch();
    match status {
        StateStatus::FirstRun => StatusSnapshot {
            phase: "first_run".to_string(),
            host_origin: None,
            dsh_version: None,
            node_version: None,
            platform: platform.to_string(),
            arch: arch.to_string(),
        },
        StateStatus::Loaded(state) => {
            let node_version = state.node.as_ref().map(|node| node.version.clone());
            let dsh_version = state.dsh.current.clone();
            let phase = match (node_version.as_ref(), dsh_version.as_ref()) {
                (Some(_), Some(_)) => "idle",
                _ => "first_run",
            };
            StatusSnapshot {
                phase: phase.to_string(),
                host_origin: None,
                dsh_version,
                node_version,
                platform: platform.to_string(),
                arch: arch.to_string(),
            }
        }
    }
}

#[tauri::command]
pub async fn start_host(state: State<'_, SharedState>) -> Result<String> {
    start_host_inner(&state.supervisor).await
}

async fn start_host_inner(supervisor: &Arc<HostSupervisor>) -> Result<String> {
    let origin = supervisor
        .start(&build_spawn_options()?)
        .await
        .map_err(map_host_error)?;
    reset_crash_counter_after_manual_start();
    Ok(origin.as_str().to_string())
}

pub async fn restart_host_inner(supervisor: &Arc<HostSupervisor>) -> Result<String> {
    let origin = supervisor
        .restart(&build_spawn_options()?)
        .await
        .map_err(map_host_error)?;
    reset_crash_counter_after_manual_start();
    Ok(origin.as_str().to_string())
}

#[tauri::command]
pub async fn restart_host(state: State<'_, SharedState>) -> Result<String> {
    restart_host_inner(&state.supervisor).await
}

#[tauri::command]
pub async fn shutdown_host(state: State<'_, SharedState>) -> Result<()> {
    let shutdown = state.supervisor.shutdown().await;
    shutdown.await_completion().await;
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

pub(super) fn build_spawn_options() -> Result<SpawnDshWebOptions> {
    use crate::dsh::{read_current_pointer, DSH_ENTRY_REL};
    use crate::node::install::current_node_dir;

    let node_dir = current_node_dir().map_err(|error| match error {
        LauncherError::NodeDownload(message) if message.contains("read VERSION file failed") => {
            LauncherError::NodeNotInstalled {
                reason: "node-runtime/VERSION not found; first-run wizard not completed"
                    .to_string(),
            }
        }
        other => other,
    })?;
    let dsh_dir = crate::paths::dsh_dir()?;
    let current_version = read_current_pointer(&dsh_dir)
        .map_err(|error| LauncherError::PathResolve {
            what: "dsh_current_pointer",
            cause: error.to_string(),
        })?
        .ok_or_else(|| LauncherError::DshNotInstalled {
            reason: "dsh/current pointer not set; first-run wizard not completed".to_string(),
        })?;
    let version_dir = dsh_dir.join(&current_version);
    let cli_entry = version_dir
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join(DSH_ENTRY_REL);
    if !cli_entry.exists() {
        return Err(LauncherError::DshNotInstalled {
            reason: format!(
                "dsh cli entry not found: {} (version {current_version} may be broken)",
                cli_entry.display()
            ),
        });
    }
    let mut env = crate::host::filtered_env();
    env.insert(
        "DSH_CLI_ENTRY".to_string(),
        cli_entry.to_string_lossy().into_owned(),
    );
    Ok(SpawnDshWebOptions {
        node_executable: crate::node::install::node_bin_path(&node_dir),
        cli_entry,
        cwd: version_dir,
        env,
        electron_run_as_node: false,
    })
}

fn map_host_error(error: HostSupervisorError) -> LauncherError {
    LauncherError::Host(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, NodeState};
    use chrono::Utc;

    #[test]
    fn loaded_runtime_is_idle() {
        let mut state = AppState::new();
        state.dsh.current = Some("0.1.0".to_string());
        state.node = Some(NodeState {
            version: "20.18.0".to_string(),
            installed_at: Utc::now(),
            mirror: "https://example.test".to_string(),
        });
        assert_eq!(
            build_status_snapshot(StateStatus::Loaded(Box::new(state))).phase,
            "idle"
        );
    }

    #[test]
    fn host_platform_uses_node_archive_names() {
        let (platform, arch) = host_platform_arch();
        assert!(matches!(platform, "darwin" | "win" | "linux"));
        assert!(matches!(arch, "arm64" | "x64"));
    }
}
