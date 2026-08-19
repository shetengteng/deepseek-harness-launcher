use tauri::State;

use crate::commands::host::SharedState;
use crate::error::{LauncherError, Result};
use crate::state::{AppState, StateStatus};

#[tauri::command]
pub fn exit_app_command(app: tauri::AppHandle) -> Result<()> {
    crate::tray::request_exit(app);
    Ok(())
}

#[tauri::command]
pub async fn rollback_dsh_command() -> Result<String> {
    use crate::dsh::version::rollback_to_known_good;
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => {
            return Err(LauncherError::DshNotInstalled {
                reason: "state.json not found".to_string(),
            })
        }
        StateStatus::Loaded(state) => *state,
    };
    let version = rollback_to_known_good(&mut state, &crate::paths::dsh_dir()?)?;
    state.save()?;
    Ok(version)
}

#[tauri::command]
pub async fn uninstall_managed_runtime(
    app: tauri::AppHandle,
    state: State<'_, SharedState>,
) -> Result<()> {
    let shutdown = state.supervisor.shutdown().await;
    shutdown.await_completion().await;
    remove_managed_runtime(
        &crate::paths::state_file()?,
        &crate::paths::node_runtime_dir()?,
        &crate::paths::dsh_dir()?,
    )?;
    tracing::info!("managed runtime uninstalled");
    app.exit(0);
    Ok(())
}

fn remove_managed_runtime(
    state_file: &std::path::Path,
    node_runtime_dir: &std::path::Path,
    dsh_dir: &std::path::Path,
) -> Result<()> {
    for path in [state_file, node_runtime_dir, dsh_dir] {
        remove_path_if_exists(path)?;
    }
    Ok(())
}

fn remove_path_if_exists(path: &std::path::Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => std::fs::remove_dir_all(path)?,
        Ok(_) => std::fs::remove_file(path)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(LauncherError::Io(error)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uninstall_keeps_logs_outside_owned_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state = temp.path().join("state.json");
        let node = temp.path().join("node-runtime");
        let dsh = temp.path().join("dsh");
        let logs = temp.path().join("logs");
        std::fs::write(&state, "{}").expect("state");
        std::fs::create_dir_all(&node).expect("node");
        std::fs::create_dir_all(&dsh).expect("dsh");
        std::fs::create_dir_all(&logs).expect("logs");
        remove_managed_runtime(&state, &node, &dsh).expect("uninstall");
        assert!(!state.exists() && !node.exists() && !dsh.exists() && logs.exists());
    }
}
