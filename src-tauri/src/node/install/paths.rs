use std::path::{Path, PathBuf};

use crate::error::{LauncherError, Result};
use crate::paths;

/// 读取 `runtime_dir/VERSION` 文件。文件不存在视为未安装。
pub fn current_node_version_in(runtime_dir: &Path) -> Result<String> {
    let version_file = runtime_dir.join("VERSION");
    std::fs::read_to_string(&version_file)
        .map(|version| version.trim().to_string())
        .map_err(|error| LauncherError::NodeDownload(format!("read VERSION file failed: {error}")))
}

/// 读取默认 `<data>/node-runtime/VERSION` 文件。
pub fn current_node_version() -> Result<String> {
    current_node_version_in(&paths::node_runtime_dir()?)
}

/// 当前激活 Node 的目录（在指定 runtime_dir 下）：`runtime_dir/node-v{VERSION}/`。
pub fn current_node_dir_in(runtime_dir: &Path) -> Result<PathBuf> {
    Ok(runtime_dir.join(format!("node-v{}", current_node_version_in(runtime_dir)?)))
}

/// 当前激活 Node 的目录：`<data>/node-runtime/node-v{VERSION}/`。
pub fn current_node_dir() -> Result<PathBuf> {
    current_node_dir_in(&paths::node_runtime_dir()?)
}

/// 当前激活 Node 的 bin 路径。
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_paths_match_current_platform() {
        let directory = Path::new("/tmp/node");
        #[cfg(target_os = "windows")]
        {
            assert_eq!(node_bin_path(directory), Path::new("/tmp/node/node.exe"));
            assert_eq!(
                node_npm_path(directory),
                Path::new("/tmp/node/node_modules/npm/bin/npm-cli.js")
            );
        }
        #[cfg(not(target_os = "windows"))]
        {
            assert_eq!(node_bin_path(directory), Path::new("/tmp/node/bin/node"));
            assert_eq!(node_npm_path(directory), Path::new("/tmp/node/bin/npm"));
        }
    }

    #[test]
    fn missing_version_file_returns_node_download_error() {
        let directory = tempfile::tempdir().unwrap();
        assert!(matches!(
            current_node_version_in(directory.path()),
            Err(LauncherError::NodeDownload(_))
        ));
    }
}
