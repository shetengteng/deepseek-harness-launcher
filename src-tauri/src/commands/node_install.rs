use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize)]
pub struct NodeUpdateTarget {
    pub current_version: String,
    pub target_version: String,
    pub engines_node: Option<String>,
    pub target_source: String,
    pub update_available: bool,
}

#[tauri::command]
pub async fn install_node_command(
    app: tauri::AppHandle,
    operations: State<'_, node::NodeDownloadOperations>,
    args: InstallNodeArgs,
) -> Result<String> {
    use crate::node::{
        cleanup_cancelled_install, download_with_retry_cancellable, install_node_to_cancellable,
        remove_archive, DownloadRequest,
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
            DownloadRequest {
                client: &client,
                mirror: &mirror,
                version: &version,
                archive_filename: &archive_filename,
                dest_dir: &staging_download_dir,
                progress_tx: Some(&tx),
                max_retries: 2,
            },
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
                let cleanup_result = cleanup_cancelled_install(&runtime_dir, &installed);
                remove_cancelled_artifacts(
                    &staging_download_dir,
                    &archive_filename,
                    &runtime_dir,
                    &version,
                )
                .await;
                cleanup_result?;
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

#[tauri::command]
pub async fn get_node_update_target_command() -> Result<NodeUpdateTarget> {
    use crate::dsh::{default_client, fetch_package_metadata, RegistryCache};

    let state = match AppState::load()? {
        StateStatus::FirstRun => {
            return Err(LauncherError::NodeNotInstalled {
                reason: "cannot update Node before the first-run runtime exists".to_string(),
            })
        }
        StateStatus::Loaded(state) => *state,
    };
    let node = state
        .node
        .as_ref()
        .ok_or_else(|| LauncherError::NodeNotInstalled {
            reason: "cannot update Node before a managed runtime exists".to_string(),
        })?;
    let dsh_version =
        state
            .dsh
            .current
            .as_deref()
            .ok_or_else(|| LauncherError::DshNotInstalled {
                reason: "cannot select a compatible Node version without an installed dsh"
                    .to_string(),
            })?;
    let client = default_client();
    let metadata =
        fetch_package_metadata(&state.dsh.registry, &RegistryCache::new(), &client).await?;
    let manifest = metadata.manifest_for(dsh_version)?;
    let (platform, arch) = host_platform_arch();
    let engines_node = manifest.engines.node.trim();
    let (target_version, target_source, engines_node) = if engines_node.is_empty() {
        (
            node::DEFAULT_NODE_VERSION.to_string(),
            "launcher-verified-fallback".to_string(),
            None,
        )
    } else {
        (
            node::resolve_latest_node_target(engines_node, platform, arch, &client).await?,
            "dsh-engines".to_string(),
            Some(engines_node.to_string()),
        )
    };
    let update_available =
        node::parse_node_version(&target_version)? > node::parse_node_version(&node.version)?;

    Ok(NodeUpdateTarget {
        current_version: node.version.clone(),
        target_version,
        engines_node,
        target_source,
        update_available,
    })
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
