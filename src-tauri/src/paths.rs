//! 用户数据/日志/缓存目录解析（设计 §4.2）。
//!
//! 使用 `directories::ProjectDirs` 拿到平台规范的目录：
//! - macOS:   `~/Library/Application Support/<qualifier>/<organization>/<application>/`
//! - Windows: `%APPDATA%\<qualifier>\<organization>\<application>\`
//! - Linux:   `~/.local/share/<application>/`
//!
//! 本 launcher 用 `ProjectDirs::from("io.deepseek", "DeepSeek", "deepseek-harness-launcher")`，
//! 在所有平台拿到一致的根目录，子目录按设计 §4.2 拼接。

use std::path::PathBuf;

use directories::ProjectDirs;

use crate::error::{LauncherError, Result};

const QUALIFIER: &str = "io.deepseek";
const ORGANIZATION: &str = "DeepSeek";
const APPLICATION: &str = "deepseek-harness-launcher";

/// 拿到 `ProjectDirs`。在极端环境（HOME 未设、权限拒绝）可能失败。
pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION).ok_or_else(|| {
        LauncherError::PathResolve {
            what: "project",
            cause: "ProjectDirs::from returned None (HOME or APPDATA may be unset)".to_string(),
        }
    })
}

/// 数据目录根。所有持久化文件（state.json、node-runtime/、dsh/、logs/）的父目录。
pub fn data_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.data_dir().to_path_buf())
}

/// 壳子日志目录（tracing 输出）。与设计 §11.2 对齐：
/// - macOS:   `~/Library/Logs/deepseek-harness-launcher/`（用户习惯位置）
/// - Windows: `<data_dir>\logs\`（与 dsh 子进程日志同根，便于打包导出）
/// - Linux:   `<data_dir>/logs/`
pub fn log_dir() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        use directories::BaseDirs;
        let base = BaseDirs::new().ok_or_else(|| LauncherError::PathResolve {
            what: "base",
            cause: "BaseDirs::new returned None (HOME may be unset)".to_string(),
        })?;
        let mut p = base.home_dir().to_path_buf();
        p.extend(["Library", "Logs", APPLICATION]);
        Ok(p)
    }
    #[cfg(not(target_os = "macos"))]
    {
        // 非 macOS 平台：跟 dsh 子进程日志同根，简化跨平台一致行为。
        Ok(data_dir()?.join("logs"))
    }
}

/// dsh 子进程日志目录：`<data_dir>/logs/`。
/// 设计 §4.2/§11.2：dsh 输出重定向到 `<data_dir>/logs/dsh-<timestamp>.log`。
pub fn dsh_log_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("logs"))
}

/// state.json 完整路径。
pub fn state_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("state.json"))
}

/// `node-runtime/` 目录：Node 二进制解压目标。
pub fn node_runtime_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("node-runtime"))
}

/// `node-runtime/VERSION` 文件，记录当前 Node 版本。
pub fn node_version_file() -> Result<PathBuf> {
    Ok(node_runtime_dir()?.join("VERSION"))
}

/// `dsh/` 目录：dsh 安装目标，按版本分子目录。
pub fn dsh_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("dsh"))
}

/// `dsh/<version>/` 子目录。
pub fn dsh_version_dir(version: &str) -> Result<PathBuf> {
    Ok(dsh_dir()?.join(version))
}

/// `dsh/current` 软链或 JSON 指针。
pub fn dsh_current_pointer() -> Result<PathBuf> {
    Ok(dsh_dir()?.join("current"))
}

/// `dsh/known-good` 软链或 JSON 指针。
pub fn dsh_known_good_pointer() -> Result<PathBuf> {
    Ok(dsh_dir()?.join("known-good"))
}

/// 确保所有父目录存在（幂等）。首次启动或迁移时调用。
pub fn ensure_dirs() -> Result<()> {
    let dirs = [
        data_dir()?,
        node_runtime_dir()?,
        dsh_dir()?,
        log_dir()?,
        dsh_log_dir()?,
    ];
    for d in dirs {
        std::fs::create_dir_all(&d)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dirs_resolves_on_dev_machine() {
        // 在 macOS/Linux/Windows 开发机上都应该成功。
        let dirs = project_dirs().expect("project_dirs should resolve");
        // ProjectDirs 5.x 在 macOS 把 `qualifier.organization.application` 拼成单一组件，
        // 不强求末段名等于 APPLICATION；只要解析到非空路径即可。
        assert!(!dirs.project_path().as_os_str().is_empty());
    }

    #[test]
    fn state_file_under_data_dir() {
        let state = state_file().expect("state_file");
        let data = data_dir().expect("data_dir");
        assert!(state.starts_with(&data));
        assert_eq!(state.file_name(), Some(std::ffi::OsStr::new("state.json")));
    }

    #[test]
    fn dsh_version_dir_is_child_of_dsh_dir() {
        let parent = dsh_dir().expect("dsh_dir");
        let child = dsh_version_dir("0.1.0-rc.6").expect("dsh_version_dir");
        assert!(child.starts_with(&parent));
        assert_eq!(child.file_name(), Some(std::ffi::OsStr::new("0.1.0-rc.6")));
    }

    #[test]
    fn ensure_dirs_is_idempotent() {
        // 用真实路径验证幂等性（ProjectDirs 解析的目录，正常情况下都创建得了）。
        ensure_dirs().expect("ensure_dirs first call");
        ensure_dirs().expect("ensure_dirs second call");
    }
}
