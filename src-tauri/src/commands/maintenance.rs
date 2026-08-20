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
    remove_managed_runtime(&crate::paths::data_dir()?)?;
    if let Err(error) = crate::cli_shim::uninstall_default() {
        tracing::warn!(
            %error,
            "managed dsh command shim could not be removed during reinstall"
        );
    }
    tracing::info!("managed runtime uninstalled");
    app.exit(0);
    Ok(())
}

fn remove_managed_runtime(data_dir: &std::path::Path) -> Result<()> {
    for name in [
        "state.json",
        "node-runtime",
        "dsh",
        "bin",
        "dsh-cli-shim.cjs",
        "npm-cache",
        "npmrc",
        "npmrc-global",
    ] {
        remove_path_if_exists(&data_dir.join(name))?;
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
        let data = temp.path();
        let logs = data.join("logs");
        for name in [
            "state.json",
            "node-runtime",
            "dsh",
            "bin",
            "dsh-cli-shim.cjs",
            "npm-cache",
            "npmrc",
            "npmrc-global",
        ] {
            let path = data.join(name);
            if name.ends_with(".json") || name.ends_with(".cjs") || name.starts_with("npmrc") {
                std::fs::write(&path, "{}").expect("file");
            } else {
                std::fs::create_dir_all(&path).expect("dir");
            }
        }
        std::fs::create_dir_all(&logs).expect("logs");
        remove_managed_runtime(data).expect("uninstall");
        assert!(!data.join("state.json").exists());
        assert!(!data.join("node-runtime").exists());
        assert!(!data.join("dsh").exists());
        assert!(!data.join("bin").exists());
        assert!(!data.join("dsh-cli-shim.cjs").exists());
        assert!(!data.join("npm-cache").exists());
        assert!(!data.join("npmrc").exists());
        assert!(!data.join("npmrc-global").exists());
        assert!(logs.exists());
    }
}
