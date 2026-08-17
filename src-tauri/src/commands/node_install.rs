use serde::Deserialize;
use tauri::{Emitter, State};

use crate::commands::host::host_platform_arch;
use crate::error::{LauncherError, Result};
use crate::node::{self, Mirror, MirrorId};
use crate::state::{AppState, StateStatus};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InstallNodeArgs {
    pub version: String,
    pub operation_id: String,
    pub mirror_base_url: String,
    pub platform: String,
    pub arch: String,
}

#[tauri::command]
pub async fn install_node_command(
    app: tauri::AppHandle,
    operations: State<'_, node::NodeDownloadOperations>,
    args: InstallNodeArgs,
) -> Result<String> {
    use crate::node::{
        cleanup_cancelled_install, download_with_retry_cancellable, install_node_to_cancellable,
        remove_archive,
    };

    let InstallNodeArgs {
        version,
        operation_id,
        mirror_base_url,
        platform: _,
        arch: _,
    } = args;
    let (platform, arch) = host_platform_arch();
    let kind = node::NodeArchiveKind::for_platform(platform);
    let archive_filename = node::node_archive_filename(&version, platform, arch);
    let mirror = Mirror {
        id: MirrorId::Custom(mirror_base_url.clone()),
        name: "user-selected",
        base_url: Box::leak(mirror_base_url.clone().into_boxed_str()),
        trusted: false,
    };
    let runtime_dir = crate::paths::node_runtime_dir()?;
    std::fs::create_dir_all(&runtime_dir)?;
    let staging_download_dir = runtime_dir.join(".downloads");
    std::fs::create_dir_all(&staging_download_dir)?;
    crate::node::disk::ensure_disk_space(&staging_download_dir)?;

    let active_install = operations.register(&operation_id)?;
    let cancellation = active_install.cancellation();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<node::ProgressEvent>(64);
    let app_clone = app.clone();
    let emit_task = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let name = if event.stage == "extract" {
                "extract-progress"
            } else {
                "download-progress"
            };
            let _ = app_clone.emit(name, &event);
        }
    });

    let lifecycle_result = async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|error| LauncherError::NodeDownload(error.to_string()))?;
        let archive_path = download_with_retry_cancellable(
            &client,
            &mirror,
            &version,
            &archive_filename,
            &staging_download_dir,
            Some(&tx),
            2,
            &cancellation,
        )
        .await?;
        let installed = install_node_to_cancellable(
            &archive_path,
            &version,
            kind,
            &runtime_dir,
            Some(&tx),
            &cancellation,
        )
        .await?;
        Ok::<_, LauncherError>(installed)
    }
    .await;

    drop(tx);
    let _ = emit_task.await;

    let installed = match lifecycle_result {
        Ok(installed) => {
            if let Err(error) = cancellation.check() {
                cleanup_cancelled_install(&runtime_dir, &installed);
                remove_cancelled_artifacts(
                    &staging_download_dir,
                    &archive_filename,
                    &runtime_dir,
                    &version,
                )
                .await;
                return Err(error);
            }
            installed
        }
        Err(error) => {
            if matches!(error, LauncherError::NodeInstallCancelled { .. })
                || cancellation.check().is_err()
            {
                remove_cancelled_artifacts(
                    &staging_download_dir,
                    &archive_filename,
                    &runtime_dir,
                    &version,
                )
                .await;
                return Err(cancellation.error());
            }
            return Err(error);
        }
    };

    remove_archive(&staging_download_dir.join(&archive_filename)).await;
    drop(active_install);
    tracing::info!(target = %installed.target.display(), "node runtime installed");
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => AppState::new(),
        StateStatus::Loaded(state) => *state,
    };
    state.node = Some(crate::state::NodeState {
        version: version.clone(),
        installed_at: chrono::Utc::now(),
        mirror: mirror_base_url.clone(),
    });
    state.node_mirror = Some(mirror_base_url);
    if let Some(plan) = state.bootstrap_plan.as_mut() {
        plan.phase = "node_installed".to_string();
    }
    state.save()?;
    Ok(version)
}

#[tauri::command]
pub async fn upgrade_node_command(
    app: tauri::AppHandle,
    operations: State<'_, node::NodeDownloadOperations>,
    version: String,
    operation_id: String,
) -> Result<String> {
    let state = match AppState::load()? {
        StateStatus::FirstRun => {
            return Err(LauncherError::NodeNotInstalled {
                reason: "cannot upgrade Node before the first-run runtime exists".to_string(),
            })
        }
        StateStatus::Loaded(state) => *state,
    };
    let current = state
        .node
        .as_ref()
        .ok_or_else(|| LauncherError::NodeNotInstalled {
            reason: "cannot upgrade Node before a managed runtime exists".to_string(),
        })?;
    let mirror_base_url = state
        .node_mirror
        .clone()
        .unwrap_or_else(|| current.mirror.clone());
    install_node_command(
        app,
        operations,
        InstallNodeArgs {
            version,
            operation_id,
            mirror_base_url,
            platform: String::new(),
            arch: String::new(),
        },
    )
    .await
}

async fn remove_cancelled_artifacts(
    downloads_dir: &std::path::Path,
    archive_filename: &str,
    runtime_dir: &std::path::Path,
    version: &str,
) {
    crate::node::remove_archive(&downloads_dir.join(archive_filename)).await;
    let _ = tokio::fs::remove_dir_all(runtime_dir.join(format!(".staging-v{version}"))).await;
}

#[tauri::command]
pub fn cancel_node_install_command(
    operations: State<'_, node::NodeDownloadOperations>,
    operation_id: String,
) -> bool {
    operations.cancel(&operation_id)
}
