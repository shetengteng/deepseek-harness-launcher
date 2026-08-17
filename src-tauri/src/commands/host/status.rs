use std::path::Path;

use serde::Serialize;

use crate::error::Result;
use crate::state::{AppState, StateStatus};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct StatusSnapshot {
    pub phase: String,
    pub host_origin: Option<String>,
    pub dsh_version: Option<String>,
    pub node_version: Option<String>,
    pub platform: String,
    pub arch: String,
}

pub fn host_platform_arch() -> (&'static str, &'static str) {
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win",
        _ => "linux",
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "x64",
    };
    (platform, arch)
}

pub async fn load_runtime_status() -> Result<StatusSnapshot> {
    let node_runtime_dir = crate::paths::node_runtime_dir()?;
    let dsh_dir = crate::paths::dsh_dir()?;
    Ok(build_runtime_status_snapshot(
        AppState::load()?,
        &node_runtime_dir,
        &dsh_dir,
    ))
}

pub fn build_status_snapshot(status: StateStatus) -> StatusSnapshot {
    let (platform, arch) = host_platform_arch();
    match status {
        StateStatus::FirstRun => StatusSnapshot {
            phase: "first_run".to_string(),
            host_origin: None,
            dsh_version: None,
            node_version: None,
            platform: platform.to_string(),
            arch: arch.to_string(),
        },
        StateStatus::Loaded(state) => {
            let node_version = state.node.as_ref().map(|node| node.version.clone());
            let dsh_version = state.dsh.current.clone();
            let phase = match (node_version.as_ref(), dsh_version.as_ref()) {
                (Some(_), Some(_)) => "idle",
                _ => "first_run",
            };
            StatusSnapshot {
                phase: phase.to_string(),
                host_origin: None,
                dsh_version,
                node_version,
                platform: platform.to_string(),
                arch: arch.to_string(),
            }
        }
    }
}

fn build_runtime_status_snapshot(
    status: StateStatus,
    node_runtime_dir: &Path,
    _dsh_dir: &Path,
) -> StatusSnapshot {
    let StateStatus::Loaded(state) = status else {
        return build_status_snapshot(StateStatus::FirstRun);
    };
    let node_ready = managed_node_is_usable(&state, node_runtime_dir);
    let (platform, arch) = host_platform_arch();
    let node_version = node_ready
        .then(|| state.node.as_ref().map(|node| node.version.clone()))
        .flatten();
    // dsh 文件缺失时仍进入启动路径：`start_host` 会校验 current，并在存在
    // `known_good` 时仅回退一次。没有可回退版本时，它返回 `dsh_not_installed`，
    // 前端再进入修复安装流程。
    let dsh_version = node_ready.then(|| state.dsh.current.clone()).flatten();
    let phase = if node_version.is_some() && dsh_version.is_some() {
        "idle"
    } else {
        "first_run"
    };
    StatusSnapshot {
        phase: phase.to_string(),
        host_origin: None,
        dsh_version,
        node_version,
        platform: platform.to_string(),
        arch: arch.to_string(),
    }
}

fn managed_node_is_usable(state: &AppState, node_runtime_dir: &Path) -> bool {
    let Some(node_state) = state.node.as_ref() else {
        return false;
    };
    let Ok(node_dir) = crate::node::install::current_node_dir_in(node_runtime_dir) else {
        return false;
    };
    node_dir
        .file_name()
        .is_some_and(|name| name == std::ffi::OsStr::new(&format!("node-v{}", node_state.version)))
        && crate::node::install::node_bin_path(&node_dir).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, NodeState};
    use chrono::Utc;

    use super::super::test_support::{write_dsh_entry, write_node_runtime};

    #[test]
    fn loaded_runtime_is_idle() {
        let mut state = AppState::new();
        state.dsh.current = Some("0.1.0".to_string());
        state.node = Some(NodeState {
            version: "20.18.0".to_string(),
            installed_at: Utc::now(),
            mirror: "https://example.test".to_string(),
        });
        assert_eq!(
            build_status_snapshot(StateStatus::Loaded(Box::new(state))).phase,
            "idle"
        );
    }

    #[test]
    fn host_platform_uses_node_archive_names() {
        let (platform, arch) = host_platform_arch();
        assert!(matches!(platform, "darwin" | "win" | "linux"));
        assert!(matches!(arch, "arm64" | "x64"));
    }

    #[test]
    fn status_keeps_dsh_startup_recovery_available_when_current_entry_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("node-runtime");
        let dsh = temp.path().join("dsh");
        std::fs::create_dir_all(&runtime).unwrap();
        write_node_runtime(&runtime, "22.19.0");
        crate::dsh::write_current_pointer(&dsh, "0.2.0").unwrap();
        let mut state = AppState::new();
        state.node = Some(NodeState {
            version: "22.19.0".to_string(),
            installed_at: Utc::now(),
            mirror: "https://example.test".to_string(),
        });
        state.dsh.current = Some("0.2.0".to_string());

        let status =
            build_runtime_status_snapshot(StateStatus::Loaded(Box::new(state)), &runtime, &dsh);

        assert_eq!(status.phase, "idle");
        assert_eq!(status.node_version.as_deref(), Some("22.19.0"));
        assert_eq!(status.dsh_version.as_deref(), Some("0.2.0"));
    }

    #[test]
    fn status_enters_full_repair_when_managed_node_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("node-runtime");
        let dsh = temp.path().join("dsh");
        std::fs::create_dir_all(&runtime).unwrap();
        write_dsh_entry(&dsh, "0.2.0");
        crate::dsh::write_current_pointer(&dsh, "0.2.0").unwrap();
        let mut state = AppState::new();
        state.node = Some(NodeState {
            version: "22.19.0".to_string(),
            installed_at: Utc::now(),
            mirror: "https://example.test".to_string(),
        });
        state.dsh.current = Some("0.2.0".to_string());

        let status =
            build_runtime_status_snapshot(StateStatus::Loaded(Box::new(state)), &runtime, &dsh);

        assert_eq!(status.phase, "first_run");
        assert_eq!(status.node_version, None);
        assert_eq!(status.dsh_version, None);
    }
}
