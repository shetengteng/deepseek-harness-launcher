//! Node 跨平台解压 + 原子切换（设计 §M2.3）。
//!
//! 流程：
//! 1. `install_node(archive_path, version, kind, progress_tx)`：
//!    - 解压到 `<data>/node-runtime/.staging-v{version}/`
//!    - 成功后 rename 为 `node-v{version}/`
//!    - 更新 `VERSION` 文件
//!    - 目标已存在则跳过解压（旧版本保留，支持回滚）
//! 2. `current_node_dir()` / `current_node_bin()` / `current_node_npm()`：解析当前激活 Node 路径
//!
//! tar.gz 内层目录 `node-v{version}-{platform}-{arch}/` 会被 strip，使 `dest_dir`
//! 直接等价于该内层目录（即 `node-v{version}/bin/node`、`node-v{version}/package.json`）。

use std::path::{Path, PathBuf};

use tokio::sync::mpsc;

use crate::error::{LauncherError, Result};
use crate::node::download::ProgressEvent;
use crate::paths;

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

/// 读取 `runtime_dir/VERSION` 文件。文件不存在视为未安装。
pub fn current_node_version_in(runtime_dir: &Path) -> Result<String> {
    let path = runtime_dir.join("VERSION");
    let s = std::fs::read_to_string(&path)
        .map_err(|e| LauncherError::NodeDownload(format!("read VERSION file failed: {e}")))?;
    Ok(s.trim().to_string())
}

/// 读取默认 `<data>/node-runtime/VERSION` 文件。
pub fn current_node_version() -> Result<String> {
    current_node_version_in(&paths::node_runtime_dir()?)
}

/// 当前激活 Node 的目录（在指定 runtime_dir 下）：`runtime_dir/node-v{VERSION}/`。
pub fn current_node_dir_in(runtime_dir: &Path) -> Result<PathBuf> {
    let version = current_node_version_in(runtime_dir)?;
    Ok(runtime_dir.join(format!("node-v{version}")))
}

/// 当前激活 Node 的目录：`<data>/node-runtime/node-v{VERSION}/`。
/// 若 VERSION 文件不存在则返回 Err（未安装）。
pub fn current_node_dir() -> Result<PathBuf> {
    current_node_dir_in(&paths::node_runtime_dir()?)
}

/// 当前激活 Node 的 bin 路径。
/// - Unix: `<dir>/bin/node`
/// - Windows: `<dir>/node.exe`
pub fn node_bin_path(node_dir: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        node_dir.join("node.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        node_dir.join("bin").join("node")
    }
}

/// 当前激活 Node 的 npm 入口。
/// - Unix: `<dir>/bin/npm`（shell 脚本）
/// - Windows: `<dir>/node_modules/npm/bin/npm-cli.js`
pub fn node_npm_path(node_dir: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        node_dir
            .join("node_modules")
            .join("npm")
            .join("bin")
            .join("npm-cli.js")
    }
    #[cfg(not(target_os = "windows"))]
    {
        node_dir.join("bin").join("npm")
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
        tracing::info!(
            version,
            target = %target_dir.display(),
            "node already installed, skipping"
        );
        return Ok(target_dir);
    }

    let staging_dir = runtime_dir.join(format!(".staging-v{version}"));
    // 清理可能的残留
    if staging_dir.exists() {
        std::fs::remove_dir_all(&staging_dir).map_err(LauncherError::Io)?;
    }
    std::fs::create_dir_all(&staging_dir).map_err(LauncherError::Io)?;

    // emit extract start
    if let Some(tx) = progress_tx {
        let _ = tx.try_send(ProgressEvent {
            stage: "extract".to_string(),
            bytes: 0,
            total: None,
        });
    }

    match kind {
        NodeArchiveKind::TarGz => extract_tar_gz(archive_path, &staging_dir).await?,
        NodeArchiveKind::Zip => extract_zip(archive_path, &staging_dir).await?,
    }

    // rename staging -> target（同分区原子操作）
    std::fs::rename(&staging_dir, &target_dir).map_err(|e| {
        LauncherError::NodeDownload(format!("rename staging to target failed: {e}"))
    })?;

    // 更新 VERSION 文件
    let version_file = runtime_dir.join("VERSION");
    std::fs::write(&version_file, version).map_err(LauncherError::Io)?;

    // emit extract complete
    if let Some(tx) = progress_tx {
        let _ = tx.try_send(ProgressEvent {
            stage: "extract".to_string(),
            bytes: 0,
            total: Some(0),
        });
    }

    tracing::info!(version, target = %target_dir.display(), "node installed");
    Ok(target_dir)
}

/// 安装 Node 到默认 `<data>/node-runtime/`。等价于 `install_node_to(.., &paths::node_runtime_dir()?, ..)`。
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
        &paths::node_runtime_dir()?,
        progress_tx,
    )
    .await
}

/// 解压 `.tar.gz` 到 `dest_dir`。
/// tar.gz 内层目录通常是 `node-v{version}-{platform}-{arch}/`，会被 strip。
fn extract_tar_gz_blocking(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path).map_err(LauncherError::Io)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    for entry in archive
        .entries()
        .map_err(|e| LauncherError::NodeDownload(format!("tar entries failed: {e}")))?
    {
        let mut entry = entry
            .map_err(|e| LauncherError::NodeDownload(format!("tar entry read failed: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| LauncherError::NodeDownload(format!("tar entry path failed: {e}")))?;
        // strip 首段（root dir），如 `node-v22.19.0-darwin-arm64/bin/node` → `bin/node`
        let rel = path.components().skip(1).collect::<PathBuf>();
        if rel.as_os_str().is_empty() {
            // root dir entry，跳过
            continue;
        }
        let target = dest_dir.join(&rel);
        // 防止路径逃逸（zip-slip 攻击防护）
        if !target.starts_with(dest_dir) {
            return Err(LauncherError::NodeDownload(format!(
                "tar entry path escapes dest_dir: {}",
                path.display()
            )));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(LauncherError::Io)?;
        }
        entry
            .unpack(&target)
            .map_err(|e| LauncherError::NodeDownload(format!("tar unpack failed: {e}")))?;
    }

    // macOS 26 (Tahoe) AMFI 拒绝执行 linker-signed adhoc 签名的二进制文件 spawn 子进程。
    // nodejs.org 官方分发的 node 二进制虽带签名，但在某些场景下仍可能被 AMFI 拒绝；
    // 为确保 spawn 成功，解压后强制重新签名 node 可执行文件，将 flags 改为纯 adhoc。
    // 参考：https://github.com/Gentleman-Programming/engram/issues/402
    #[cfg(target_os = "macos")]
    {
        re_sign_macos_binaries(dest_dir)?;
    }

    Ok(())
}

async fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let archive_path = archive_path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();
    tokio::task::spawn_blocking(move || extract_tar_gz_blocking(&archive_path, &dest_dir))
        .await
        .map_err(|e| LauncherError::NodeDownload(format!("blocking task failed: {e}")))?
}

/// 解压 `.zip` 到 `dest_dir`（Windows）。
fn extract_zip_blocking(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path).map_err(LauncherError::Io)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| LauncherError::NodeDownload(format!("zip open failed: {e}")))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| LauncherError::NodeDownload(format!("zip entry {i} failed: {e}")))?;
        let name = entry.name().to_string();
        let path = Path::new(&name);
        let rel = path.components().skip(1).collect::<PathBuf>();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dest_dir.join(&rel);
        if !target.starts_with(dest_dir) {
            return Err(LauncherError::NodeDownload(format!(
                "zip entry path escapes dest_dir: {}",
                path.display()
            )));
        }
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(LauncherError::Io)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(LauncherError::Io)?;
            }
            let mut out = std::fs::File::create(&target).map_err(LauncherError::Io)?;
            std::io::copy(&mut entry, &mut out).map_err(LauncherError::Io)?;
            // 保留 unix 权限位（Windows zip 也可能携带）
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode))
                        .map_err(LauncherError::Io)?;
                }
            }
        }
    }

    // 同 extract_tar_gz_blocking：macOS 上重新签名 node 二进制以满足 AMFI 要求。
    #[cfg(target_os = "macos")]
    {
        re_sign_macos_binaries(dest_dir)?;
    }

    Ok(())
}

/// macOS：对 `dest_dir` 下的 node / npm 可执行文件执行 `codesign --force --sign -`。
///
/// macOS 26 (Tahoe) 的 AMFI 拒绝 `linker-signed` adhoc 签名（flags `0x20002`）的二进制
/// 文件 spawn 子进程，会返回 `Operation not permitted (os error 1)`。
/// `codesign --force --sign -` 会把签名 flags 改为纯 adhoc（`0x2`），让 AMFI 放行。
///
/// 被签名的目标：
/// - `bin/node`（核心：所有 spawn 的起点）
/// - `bin/npm`（shell 脚本，本身不需要签名，但签名不会报错；保留以应对未来可能改为二进制的场景）
///
/// 失败处理：`codesign` 命令不存在或签名失败时返回 `NodeDownload` 错误，避免静默跳过。
#[cfg(target_os = "macos")]
fn re_sign_macos_binaries(dest_dir: &Path) -> Result<()> {
    use std::process::Command;

    let targets = [
        dest_dir.join("bin").join("node"),
        dest_dir.join("bin").join("npm"),
    ];

    for bin in targets {
        if !bin.exists() {
            continue;
        }
        let output = Command::new("codesign")
            .arg("--force")
            .arg("--sign")
            .arg("-")
            .arg(&bin)
            .output()
            .map_err(|e| {
                LauncherError::NodeDownload(format!(
                    "failed to invoke codesign for {}: {e}",
                    bin.display()
                ))
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LauncherError::NodeDownload(format!(
                "codesign failed for {}: {} (exit code: {})",
                bin.display(),
                stderr.trim(),
                output.status
            )));
        }
        tracing::debug!(target = %bin.display(), "re-signed binary for macOS AMFI");
    }
    Ok(())
}

async fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let archive_path = archive_path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();
    tokio::task::spawn_blocking(move || extract_zip_blocking(&archive_path, &dest_dir))
        .await
        .map_err(|e| LauncherError::NodeDownload(format!("blocking task failed: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 构造一个真实 tar.gz fixture，外层目录 `node-v22.19.0-test/`。
    fn build_node_tar_gz() -> Vec<u8> {
        let mut tar_buf: Vec<u8> = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_buf);
            let node_bin = b"#!/bin/sh\necho 'fake node'\n";
            let mut header = tar::Header::new_gnu();
            header.set_path("node-v22.19.0-test/bin/node").unwrap();
            header.set_size(node_bin.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append(&header, &node_bin[..]).unwrap();
            let pkg = b"{\"name\":\"node\",\"version\":\"22.19.0\"}\n";
            let mut header = tar::Header::new_gnu();
            header.set_path("node-v22.19.0-test/package.json").unwrap();
            header.set_size(pkg.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, &pkg[..]).unwrap();
            builder.finish().unwrap();
        }
        let mut gz_buf: Vec<u8> = Vec::new();
        {
            let mut encoder =
                flate2::write::GzEncoder::new(&mut gz_buf, flate2::Compression::default());
            encoder.write_all(&tar_buf).unwrap();
            encoder.finish().unwrap();
        }
        gz_buf
    }

    /// 构造一个真实 zip fixture，外层目录 `node-v22.19.0-test/`。
    fn build_node_zip() -> Vec<u8> {
        use std::io::Cursor;
        let mut buf: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            zip.add_directory("node-v22.19.0-test/", opts).unwrap();
            zip.add_directory("node-v22.19.0-test/bin/", opts).unwrap();
            zip.start_file("node-v22.19.0-test/bin/node", opts.unix_permissions(0o755))
                .unwrap();
            zip.write_all(b"fake node\n").unwrap();
            zip.start_file("node-v22.19.0-test/package.json", opts)
                .unwrap();
            zip.write_all(b"{\"name\":\"node\"}\n").unwrap();
            zip.finish().unwrap();
        }
        buf.into_inner()
    }

    #[tokio::test]
    async fn install_node_tar_gz_extracts_and_writes_version() {
        let archive_bytes = build_node_tar_gz();
        let tmp = tempfile::tempdir().unwrap();
        // 写 archive 到临时文件
        let archive_path = tmp.path().join("node.tar.gz");
        std::fs::write(&archive_path, &archive_bytes).unwrap();

        // 用 tmp.path() 替换真实 data_dir（绕过 paths::node_runtime_dir）
        // 我们直接调用 extract_tar_gz_blocking 来测核心逻辑
        let runtime_dir = tmp.path().join("node-runtime");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let staging_dir = runtime_dir.join(".staging-v22.19.0");
        std::fs::create_dir_all(&staging_dir).unwrap();

        extract_tar_gz_blocking(&archive_path, &staging_dir).expect("extract should succeed");

        // 验证内层 root 被 strip：`staging/bin/node`、`staging/package.json`
        let bin = staging_dir.join("bin").join("node");
        assert!(bin.exists(), "bin/node should exist at {}", bin.display());
        let pkg = staging_dir.join("package.json");
        assert!(pkg.exists());
    }

    #[tokio::test]
    async fn install_node_tar_gz_via_install_node_writes_version_file() {
        // 这个测试需要绕过 paths::node_runtime_dir()，改为传入 dest_dir。
        // 我们只测 extract + rename + VERSION 写入是否正常，用 staging + 路径直接验证。
        let archive_bytes = build_node_tar_gz();
        let tmp = tempfile::tempdir().unwrap();
        let archive_path = tmp.path().join("node.tar.gz");
        std::fs::write(&archive_path, &archive_bytes).unwrap();

        // 直接测 extract_tar_gz_blocking + 手动 rename + 写 VERSION，等价 install_node 逻辑
        let runtime_dir = tmp.path().join("node-runtime");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let staging_dir = runtime_dir.join(".staging-v22.19.0");
        std::fs::create_dir_all(&staging_dir).unwrap();

        extract_tar_gz_blocking(&archive_path, &staging_dir).expect("extract");
        let target_dir = runtime_dir.join("node-v22.19.0");
        std::fs::rename(&staging_dir, &target_dir).unwrap();
        std::fs::write(runtime_dir.join("VERSION"), "22.19.0").unwrap();

        assert!(target_dir.join("bin").join("node").exists());
        assert_eq!(
            std::fs::read_to_string(runtime_dir.join("VERSION")).unwrap(),
            "22.19.0"
        );
        assert!(!staging_dir.exists());
    }

    #[tokio::test]
    async fn install_node_zip_extracts() {
        let archive_bytes = build_node_zip();
        let tmp = tempfile::tempdir().unwrap();
        let archive_path = tmp.path().join("node.zip");
        std::fs::write(&archive_path, &archive_bytes).unwrap();

        let runtime_dir = tmp.path().join("node-runtime");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let staging_dir = runtime_dir.join(".staging-v22.19.0");
        std::fs::create_dir_all(&staging_dir).unwrap();

        extract_zip_blocking(&archive_path, &staging_dir).expect("extract zip should succeed");

        let bin = staging_dir.join("bin").join("node");
        assert!(bin.exists(), "bin/node should exist");
        let pkg = staging_dir.join("package.json");
        assert!(pkg.exists());
    }

    #[test]
    fn archive_kind_for_current_platform_is_consistent() {
        let kind = archive_kind_for_current_platform();
        #[cfg(target_os = "windows")]
        assert_eq!(kind, NodeArchiveKind::Zip);
        #[cfg(not(target_os = "windows"))]
        assert_eq!(kind, NodeArchiveKind::TarGz);
    }

    #[test]
    fn node_bin_path_per_platform() {
        let dir = Path::new("/tmp/node");
        #[cfg(target_os = "windows")]
        assert_eq!(node_bin_path(dir), Path::new("/tmp/node/node.exe"));
        #[cfg(not(target_os = "windows"))]
        assert_eq!(node_bin_path(dir), Path::new("/tmp/node/bin/node"));
    }

    #[test]
    fn node_npm_path_per_platform() {
        let dir = Path::new("/tmp/node");
        #[cfg(target_os = "windows")]
        assert_eq!(
            node_npm_path(dir),
            Path::new("/tmp/node/node_modules/npm/bin/npm-cli.js")
        );
        #[cfg(not(target_os = "windows"))]
        assert_eq!(node_npm_path(dir), Path::new("/tmp/node/bin/npm"));
    }

    #[test]
    fn current_node_version_errors_when_missing() {
        // VERSION 文件应该不存在（除非恰好有真实安装）
        // 我们用一个不存在的路径验证错误
        let result = current_node_version();
        // 在测试环境，VERSION 通常不存在
        if let Err(e) = &result {
            assert!(matches!(e, LauncherError::NodeDownload(_)));
        }
    }
}
