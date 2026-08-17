//! Node 跨平台解压 + 原子切换（设计 §M2.3）。

mod extract;
mod paths;

use std::path::{Path, PathBuf};

use tokio::sync::mpsc;

use crate::error::{LauncherError, Result};
use crate::node::download::ProgressEvent;
use crate::paths as app_paths;

pub use paths::{
    current_node_dir, current_node_dir_in, current_node_version, current_node_version_in,
    node_bin_path, node_npm_path,
};

/// Archive 类型。macOS/Linux 用 tar.gz，Windows 用 zip。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeArchiveKind {
    /// `.tar.gz`
    TarGz,
    /// `.zip`
    Zip,
}

/// 推断当前平台的 archive 类型。
pub fn archive_kind_for_current_platform() -> NodeArchiveKind {
    #[cfg(target_os = "windows")]
    {
        NodeArchiveKind::Zip
    }
    #[cfg(not(target_os = "windows"))]
    {
        NodeArchiveKind::TarGz
    }
}

/// 安装 Node 到指定 `runtime_dir`：解压 archive 到 `runtime_dir/node-v{version}/`，更新 VERSION。
///
/// 原子性：先解压到 `runtime_dir/.staging-v{version}/`，成功后 rename 到 `node-v{version}/`。
/// 若目标目录已存在，跳过解压（旧版本保留，支持回滚）。
pub async fn install_node_to(
    archive_path: &Path,
    version: &str,
    kind: NodeArchiveKind,
    runtime_dir: &Path,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(runtime_dir).map_err(LauncherError::Io)?;
    let target_dir = runtime_dir.join(format!("node-v{version}"));
    if target_dir.exists() {
        tracing::info!(version, target = %target_dir.display(), "node already installed, skipping");
        return Ok(target_dir);
    }

    let staging_dir = runtime_dir.join(format!(".staging-v{version}"));
    if staging_dir.exists() {
        std::fs::remove_dir_all(&staging_dir).map_err(LauncherError::Io)?;
    }
    std::fs::create_dir_all(&staging_dir).map_err(LauncherError::Io)?;
    send_extract_progress(progress_tx, None);

    extract::extract_archive(archive_path, &staging_dir, kind).await?;
    std::fs::rename(&staging_dir, &target_dir).map_err(|error| {
        LauncherError::NodeDownload(format!("rename staging to target failed: {error}"))
    })?;
    std::fs::write(runtime_dir.join("VERSION"), version).map_err(LauncherError::Io)?;

    send_extract_progress(progress_tx, Some(0));
    tracing::info!(version, target = %target_dir.display(), "node installed");
    Ok(target_dir)
}

/// 安装 Node 到默认 `<data>/node-runtime/`。
pub async fn install_node(
    archive_path: &Path,
    version: &str,
    kind: NodeArchiveKind,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
) -> Result<PathBuf> {
    install_node_to(
        archive_path,
        version,
        kind,
        &app_paths::node_runtime_dir()?,
        progress_tx,
    )
    .await
}

fn send_extract_progress(progress_tx: Option<&mpsc::Sender<ProgressEvent>>, total: Option<u64>) {
    if let Some(sender) = progress_tx {
        let _ = sender.try_send(ProgressEvent {
            stage: "extract".to_string(),
            bytes: 0,
            total,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_kind_for_current_platform_is_consistent() {
        #[cfg(target_os = "windows")]
        assert_eq!(archive_kind_for_current_platform(), NodeArchiveKind::Zip);
        #[cfg(not(target_os = "windows"))]
        assert_eq!(archive_kind_for_current_platform(), NodeArchiveKind::TarGz);
    }
}
