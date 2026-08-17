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
    use crate::node::{download_with_retry, install_node_to, NodeArchiveKind};
    let InstallNodeArgs {
        version,
        operation_id,
        mirror_base_url,
        platform: _,
        arch: _,
    } = args;
    let (platform, arch) = host_platform_arch();
    let mirror = Mirror {
        id: MirrorId::Custom(mirror_base_url.clone()),
        name: "user-selected",
        base_url: Box::leak(mirror_base_url.clone().into_boxed_str()),
        trusted: false,
    };
    let archive_filename = format!("node-v{version}-{platform}-{arch}.tar.gz");
    let runtime_dir = crate::paths::node_runtime_dir()?;
    std::fs::create_dir_all(&runtime_dir)?;
    let staging_download_dir = runtime_dir.join(".downloads");
    std::fs::create_dir_all(&staging_download_dir)?;
    crate::node::disk::ensure_disk_space(&staging_download_dir)?;
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
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|error| LauncherError::NodeDownload(error.to_string()))?;
    let archive_path = download_with_retry(
        &client,
        &mirror,
        &version,
        &archive_filename,
        &staging_download_dir,
        Some(&tx),
        2,
        operations.inner(),
        &operation_id,
    )
    .await?;
    let target = install_node_to(
        &archive_path,
        &version,
        NodeArchiveKind::TarGz,
        &runtime_dir,
        Some(&tx),
    )
    .await?;
    drop(tx);
    let _ = emit_task.await;
    tracing::info!(target = %target.display(), "node runtime installed");
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
    let _ = std::fs::remove_file(archive_path);
    Ok(version)
}

#[tauri::command]
pub fn cancel_node_install_command(
    operations: State<'_, node::NodeDownloadOperations>,
    operation_id: String,
) -> bool {
    operations.cancel(&operation_id)
}
