//! Node 跨平台解压 + 原子切换（设计 §M2.3）。

mod extract;
mod paths;
mod verify;

use std::path::{Path, PathBuf};

use tokio::sync::mpsc;

use crate::error::{LauncherError, Result};
use crate::node::download::{DownloadCancellation, ProgressEvent};
use crate::paths as app_paths;

pub use paths::{
    current_node_dir, current_node_dir_in, current_node_version, current_node_version_in,
    node_bin_path, node_npm_path,
};
pub use verify::verify_node_binary;

/// Archive 类型。macOS/Linux 用 tar.gz，Windows 用 zip。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeArchiveKind {
    /// `.tar.gz`
    TarGz,
    /// `.zip`
    Zip,
}

impl NodeArchiveKind {
    pub fn for_platform(platform: &str) -> Self {
        if platform == "win" {
            Self::Zip
        } else {
            Self::TarGz
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::TarGz => "tar.gz",
            Self::Zip => "zip",
        }
    }
}

pub fn node_archive_filename(version: &str, platform: &str, arch: &str) -> String {
    format!(
        "node-v{version}-{platform}-{arch}.{}",
        NodeArchiveKind::for_platform(platform).extension()
    )
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

#[derive(Debug)]
pub(crate) struct InstalledNode {
    pub(crate) target: PathBuf,
    created: bool,
    previous_version: Option<Vec<u8>>,
    version: String,
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
    let created = !target_dir.exists();
    if created {
        let staging_dir = runtime_dir.join(format!(".staging-v{version}"));
        if staging_dir.exists() {
            std::fs::remove_dir_all(&staging_dir).map_err(LauncherError::Io)?;
        }
        std::fs::create_dir_all(&staging_dir).map_err(LauncherError::Io)?;
        send_extract_progress(progress_tx, None);

        let extract_result = async {
            extract::extract_archive(archive_path, &staging_dir, kind, None).await?;
            std::fs::rename(&staging_dir, &target_dir).map_err(|error| {
                LauncherError::NodeDownload(format!("rename staging to target failed: {error}"))
            })?;
            Ok(())
        }
        .await;
        if let Err(error) = extract_result {
            let _ = std::fs::remove_dir_all(&staging_dir);
            let _ = std::fs::remove_dir_all(&target_dir);
            return Err(error);
        }
    }

    if let Err(error) =
        activate_installed_node(runtime_dir, version, &target_dir, progress_tx).await
    {
        if created {
            let _ = std::fs::remove_dir_all(&target_dir);
        }
        return Err(error);
    }
    tracing::info!(version, target = %target_dir.display(), "node installed");
    Ok(target_dir)
}

pub(crate) async fn install_node_to_cancellable(
    archive_path: &Path,
    version: &str,
    kind: NodeArchiveKind,
    runtime_dir: &Path,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
    cancellation: &DownloadCancellation,
) -> Result<InstalledNode> {
    cancellation.check()?;
    std::fs::create_dir_all(runtime_dir).map_err(LauncherError::Io)?;
    let target_dir = runtime_dir.join(format!("node-v{version}"));
    let previous_version = std::fs::read(runtime_dir.join("VERSION")).ok();
    let created = !target_dir.exists();
    if created {
        let staging_dir = runtime_dir.join(format!(".staging-v{version}"));
        if staging_dir.exists() {
            std::fs::remove_dir_all(&staging_dir).map_err(LauncherError::Io)?;
        }
        std::fs::create_dir_all(&staging_dir).map_err(LauncherError::Io)?;
        send_extract_progress(progress_tx, None);

        let install_result = async {
            extract::extract_archive(archive_path, &staging_dir, kind, Some(cancellation)).await?;
            cancellation.check()?;
            std::fs::rename(&staging_dir, &target_dir).map_err(|error| {
                LauncherError::NodeDownload(format!("rename staging to target failed: {error}"))
            })?;
            Ok(())
        }
        .await;

        if let Err(error) = install_result {
            let _ = std::fs::remove_dir_all(&staging_dir);
            if matches!(error, LauncherError::NodeInstallCancelled { .. }) {
                cleanup_created_runtime(
                    runtime_dir,
                    &target_dir,
                    version,
                    previous_version.as_deref(),
                );
            }
            return Err(error);
        }
    }

    cancellation.check()?;
    if let Err(error) =
        activate_installed_node(runtime_dir, version, &target_dir, progress_tx).await
    {
        if created {
            cleanup_created_runtime(
                runtime_dir,
                &target_dir,
                version,
                previous_version.as_deref(),
            );
        }
        return Err(error);
    }
    let installed = InstalledNode {
        target: target_dir,
        created,
        previous_version,
        version: version.to_string(),
    };
    if let Err(error) = cancellation.check() {
        cleanup_cancelled_install(runtime_dir, &installed);
        return Err(error);
    }
    tracing::info!(version, target = %installed.target.display(), "node installed");
    Ok(installed)
}

pub(crate) fn cleanup_cancelled_install(runtime_dir: &Path, installed: &InstalledNode) {
    restore_previous_selection(
        runtime_dir,
        &installed.version,
        installed.previous_version.as_deref(),
    );
    if installed.created {
        let _ = std::fs::remove_dir_all(&installed.target);
    }
}

fn cleanup_created_runtime(
    runtime_dir: &Path,
    target_dir: &Path,
    version: &str,
    previous_version: Option<&[u8]>,
) {
    restore_previous_selection(runtime_dir, version, previous_version);
    let _ = std::fs::remove_dir_all(target_dir);
}

fn restore_previous_selection(runtime_dir: &Path, version: &str, previous_version: Option<&[u8]>) {
    let version_path = runtime_dir.join("VERSION");
    if std::fs::read(&version_path)
        .ok()
        .is_some_and(|current| current == version.as_bytes())
    {
        match previous_version {
            Some(previous_version) => {
                let _ = std::fs::write(&version_path, previous_version);
            }
            None => {
                let _ = std::fs::remove_file(&version_path);
            }
        }
    }
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

async fn activate_installed_node(
    runtime_dir: &Path,
    version: &str,
    target_dir: &Path,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
) -> Result<()> {
    verify_node_binary(target_dir, version).await?;
    std::fs::write(runtime_dir.join("VERSION"), version).map_err(LauncherError::Io)?;
    send_extract_progress(progress_tx, Some(0));
    Ok(())
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
    use crate::node::download::NodeDownloadOperations;

    #[cfg(unix)]
    fn write_fake_node(node_dir: &Path, version_line: &str) {
        use std::os::unix::fs::PermissionsExt;
        let bin = node_dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let path = bin.join("node");
        std::fs::write(&path, format!("#!/bin/sh\necho {version_line}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn archive_kind_for_current_platform_is_consistent() {
        #[cfg(target_os = "windows")]
        assert_eq!(archive_kind_for_current_platform(), NodeArchiveKind::Zip);
        #[cfg(not(target_os = "windows"))]
        assert_eq!(archive_kind_for_current_platform(), NodeArchiveKind::TarGz);
    }

    #[test]
    fn archive_filename_matches_official_node_dist_names() {
        assert_eq!(
            node_archive_filename("22.19.0", "darwin", "arm64"),
            "node-v22.19.0-darwin-arm64.tar.gz"
        );
        assert_eq!(
            node_archive_filename("22.19.0", "linux", "x64"),
            "node-v22.19.0-linux-x64.tar.gz"
        );
        assert_eq!(
            node_archive_filename("22.19.0", "win", "x64"),
            "node-v22.19.0-win-x64.zip"
        );
        assert_eq!(NodeArchiveKind::for_platform("win"), NodeArchiveKind::Zip);
        assert_eq!(
            NodeArchiveKind::for_platform("darwin"),
            NodeArchiveKind::TarGz
        );
    }

    #[tokio::test]
    async fn cancellation_cleans_staging_and_preserves_the_active_runtime() {
        let runtime = tempfile::tempdir().unwrap();
        let existing = runtime.path().join("node-v20.0.0");
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::write(runtime.path().join("VERSION"), "20.0.0").unwrap();
        let archive = runtime.path().join("node.tar.gz");
        std::fs::write(&archive, b"not used").unwrap();
        let operations = NodeDownloadOperations::default();
        let active = operations.register("operation-1").unwrap();
        let cancellation = active.cancellation();
        assert!(operations.cancel("operation-1"));

        let error = install_node_to_cancellable(
            &archive,
            "22.0.0",
            NodeArchiveKind::TarGz,
            runtime.path(),
            None,
            &cancellation,
        )
        .await
        .unwrap_err();

        assert!(matches!(error, LauncherError::NodeInstallCancelled { .. }));
        assert!(existing.exists());
        assert_eq!(
            std::fs::read_to_string(runtime.path().join("VERSION")).unwrap(),
            "20.0.0"
        );
        assert!(!runtime.path().join(".staging-v22.0.0").exists());
        assert!(!runtime.path().join("node-v22.0.0").exists());
    }

    #[test]
    fn cancellation_after_runtime_switch_restores_the_prior_selection() {
        let runtime = tempfile::tempdir().unwrap();
        let existing = runtime.path().join("node-v20.0.0");
        let created = runtime.path().join("node-v22.0.0");
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::create_dir_all(&created).unwrap();
        std::fs::write(runtime.path().join("VERSION"), "22.0.0").unwrap();

        cleanup_cancelled_install(
            runtime.path(),
            &InstalledNode {
                target: created.clone(),
                created: true,
                previous_version: Some(b"20.0.0".to_vec()),
                version: "22.0.0".to_string(),
            },
        );

        assert!(existing.exists());
        assert!(!created.exists());
        assert_eq!(
            std::fs::read_to_string(runtime.path().join("VERSION")).unwrap(),
            "20.0.0"
        );
    }

    #[test]
    fn cancellation_after_switching_to_an_existing_runtime_restores_the_prior_selection() {
        let runtime = tempfile::tempdir().unwrap();
        let existing = runtime.path().join("node-v20.0.0");
        let selected = runtime.path().join("node-v22.0.0");
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::create_dir_all(&selected).unwrap();
        std::fs::write(runtime.path().join("VERSION"), "22.0.0").unwrap();

        cleanup_cancelled_install(
            runtime.path(),
            &InstalledNode {
                target: selected.clone(),
                created: false,
                previous_version: Some(b"20.0.0".to_vec()),
                version: "22.0.0".to_string(),
            },
        );

        assert!(existing.exists());
        assert!(selected.exists());
        assert_eq!(
            std::fs::read_to_string(runtime.path().join("VERSION")).unwrap(),
            "20.0.0"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn switching_runtimes_updates_version_and_keeps_the_previous_install() {
        let runtime = tempfile::tempdir().unwrap();
        let previous = runtime.path().join("node-v22.19.0");
        let next = runtime.path().join("node-v24.0.0");
        write_fake_node(&previous, "v22.19.0");
        write_fake_node(&next, "v24.0.0");
        std::fs::write(runtime.path().join("VERSION"), "22.19.0").unwrap();
        let archive = runtime.path().join("node.tar.gz");
        std::fs::write(&archive, b"not used").unwrap();
        let operations = NodeDownloadOperations::default();
        let active = operations.register("switch").unwrap();

        let installed = install_node_to_cancellable(
            &archive,
            "24.0.0",
            NodeArchiveKind::TarGz,
            runtime.path(),
            None,
            &active.cancellation(),
        )
        .await
        .unwrap();

        assert_eq!(installed.target, next);
        assert!(!installed.created);
        assert!(previous.exists());
        assert!(next.exists());
        assert_eq!(
            std::fs::read_to_string(runtime.path().join("VERSION")).unwrap(),
            "24.0.0"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_binary_check_does_not_switch_the_active_runtime() {
        let runtime = tempfile::tempdir().unwrap();
        let previous = runtime.path().join("node-v22.19.0");
        let next = runtime.path().join("node-v24.0.0");
        write_fake_node(&previous, "v22.19.0");
        write_fake_node(&next, "v20.0.0");
        std::fs::write(runtime.path().join("VERSION"), "22.19.0").unwrap();
        let archive = runtime.path().join("node.tar.gz");
        std::fs::write(&archive, b"not used").unwrap();
        let operations = NodeDownloadOperations::default();
        let active = operations.register("mismatch").unwrap();

        let error = install_node_to_cancellable(
            &archive,
            "24.0.0",
            NodeArchiveKind::TarGz,
            runtime.path(),
            None,
            &active.cancellation(),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, LauncherError::NodeVersion(_)));
        assert!(previous.exists());
        assert!(next.exists());
        assert_eq!(
            std::fs::read_to_string(runtime.path().join("VERSION")).unwrap(),
            "22.19.0"
        );
    }
}
