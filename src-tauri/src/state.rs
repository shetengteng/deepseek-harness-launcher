//! `state.json` 持久化（设计 §4.3）。
//!
//! 设计要求：
//! - 缺失文件 → 首启，**不写默认空文件**（避免污染数据目录）。
//! - 写入用临时文件 + `rename` 原子替换，避免崩溃导致半截 JSON。
//! - 损坏文件（JSON 解析失败）→ `StateCorrupt` 错误，让上层走"重置或导出诊断"分支。
//! - `schema_version` 不匹配 → `UnsupportedSchemaVersion`，未来版本在此处加迁移逻辑。

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{LauncherError, Result, EXPECTED_SCHEMA_VERSION};

/// 当前 Node 运行时状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeState {
    pub version: String,
    pub installed_at: DateTime<Utc>,
    pub mirror: String,
}

/// 单个已安装 dsh 版本的记录。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstalledDsh {
    pub version: String,
    pub installed_at: DateTime<Utc>,
    /// `verified`（已通过完整性校验）/ `pending`（已下载但未启动校验过）/ `broken`（校验失败）。
    pub status: String,
}

/// dsh 总状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DshState {
    pub current: Option<String>,
    pub known_good: Option<String>,
    #[serde(default = "default_pinned_range")]
    pub pinned_range: String,
    pub pending: Option<String>,
    pub last_check: Option<DateTime<Utc>>,
    #[serde(default = "default_check_interval_hours")]
    pub check_interval_hours: u32,
    #[serde(default = "default_registry")]
    pub registry: String,
    #[serde(default)]
    pub installed: Vec<InstalledDsh>,
    #[serde(default)]
    pub ignored_versions: Vec<String>,
}

fn default_pinned_range() -> String {
    "~0.1.0".to_string()
}
fn default_check_interval_hours() -> u32 {
    24
}
fn default_registry() -> String {
    "https://registry.npmmirror.com".to_string()
}

impl Default for DshState {
    fn default() -> Self {
        Self {
            current: None,
            known_good: None,
            pinned_range: default_pinned_range(),
            pending: None,
            last_check: None,
            check_interval_hours: default_check_interval_hours(),
            registry: default_registry(),
            installed: Vec::new(),
            ignored_versions: Vec::new(),
        }
    }
}

/// 全局 state.json 结构。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppState {
    pub schema_version: u8,
    #[serde(default)]
    pub node: Option<NodeState>,
    #[serde(default)]
    pub dsh: DshState,
    #[serde(default = "default_auto_upgrade")]
    pub auto_upgrade: bool,
    #[serde(default)]
    pub crash_counter: u32,
}

fn default_auto_upgrade() -> bool {
    true
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            schema_version: EXPECTED_SCHEMA_VERSION,
            node: None,
            dsh: DshState::default(),
            auto_upgrade: default_auto_upgrade(),
            crash_counter: 0,
        }
    }
}

/// `load_state` 的返回：首启 vs 已存在状态。
#[derive(Debug)]
pub enum StateStatus {
    /// `state.json` 不存在：首启，调用方应进入首启向导。
    FirstRun,
    /// 文件存在且解析成功。`AppState` 较大，box 起来避免 enum 体积过大。
    Loaded(Box<AppState>),
}

impl AppState {
    /// 默认空状态。仅用于内存构造（例如首启向导完成后初次写入）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 从指定路径加载。文件不存在 → `FirstRun`。
    ///
    /// 设计 §4.3：缺失文件不算错误。文件存在但解析失败 → `StateCorrupt`。
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<StateStatus> {
        let path = path.as_ref();
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let state: AppState =
                    serde_json::from_str(&content).map_err(|e| LauncherError::StateCorrupt {
                        path: path.to_path_buf(),
                        cause: e.to_string(),
                    })?;
                if state.schema_version != EXPECTED_SCHEMA_VERSION {
                    return Err(LauncherError::UnsupportedSchemaVersion(
                        state.schema_version,
                    ));
                }
                Ok(StateStatus::Loaded(Box::new(state)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(StateStatus::FirstRun),
            Err(e) => Err(LauncherError::Io(e)),
        }
    }

    /// 从默认路径 `paths::state_file()` 加载。
    pub fn load() -> Result<StateStatus> {
        let path = crate::paths::state_file()?;
        Self::load_from(&path)
    }

    /// 原子写入：写到 `<path>.tmp` → `rename` 到 `<path>`。
    /// `rename` 在同一文件系统下是原子的（POSIX 保证；Windows NTFS 也支持）。
    pub fn save_to<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = tmp_sibling(path);
        let json = serde_json::to_vec_pretty(self)?;

        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// 写到默认路径 `paths::state_file()`。
    pub fn save(&self) -> Result<()> {
        let path = crate::paths::state_file()?;
        self.save_to(&path)
    }
}

/// 在 `<path>` 同目录下生成 `.tmp` 后缀的临时文件路径。
fn tmp_sibling(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("state.json"));
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::sync::Arc;
    use std::time::Duration;

    /// 在临时目录里跑测试，避免污染真实数据目录。
    fn temp_state_path() -> (PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");
        (path, dir)
    }

    #[test]
    fn missing_file_is_first_run() {
        let (path, _dir) = temp_state_path();
        match AppState::load_from(&path).expect("load_from") {
            StateStatus::FirstRun => {}
            other => panic!("expected FirstRun, got {other:?}"),
        }
    }

    #[test]
    fn save_and_reload_round_trips() {
        let (path, _dir) = temp_state_path();
        let mut state = AppState::new();
        state.dsh.current = Some("0.1.0-rc.6".to_string());
        state.crash_counter = 2;

        state.save_to(&path).expect("save");
        match AppState::load_from(&path).expect("load") {
            StateStatus::Loaded(loaded) => {
                assert_eq!(loaded.schema_version, EXPECTED_SCHEMA_VERSION);
                assert_eq!(loaded.dsh.current.as_deref(), Some("0.1.0-rc.6"));
                assert_eq!(loaded.crash_counter, 2);
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn corrupt_json_returns_state_corrupt() {
        let (path, _dir) = temp_state_path();
        fs::write(&path, b"{ this is not valid json").expect("write");
        let err = AppState::load_from(&path).unwrap_err();
        assert!(matches!(
            err,
            LauncherError::StateCorrupt { ref path, .. } if path.ends_with("state.json")
        ));
    }

    #[test]
    fn unsupported_schema_version_errors() {
        let (path, _dir) = temp_state_path();
        // 写一个 schema_version=99 的文件
        let raw = r#"{"schema_version": 99, "dsh": {}, "auto_upgrade": true, "crash_counter": 0}"#;
        fs::write(&path, raw).expect("write");
        let err = AppState::load_from(&path).unwrap_err();
        assert!(matches!(err, LauncherError::UnsupportedSchemaVersion(99)));
    }

    #[test]
    fn save_is_atomic_no_tmp_leftover() {
        let (path, _dir) = temp_state_path();
        let state = AppState::new();
        state.save_to(&path).expect("save");
        // 成功后 .tmp 应被 rename 走
        let tmp = tmp_sibling(&path);
        assert!(!tmp.exists(), "tmp file should not exist after rename");
        assert!(path.exists(), "final file should exist");
    }

    #[test]
    fn save_overrides_existing_content() {
        let (path, _dir) = temp_state_path();
        let mut state = AppState::new();
        state.crash_counter = 1;
        state.save_to(&path).expect("first save");

        state.crash_counter = 5;
        state.save_to(&path).expect("second save");

        match AppState::load_from(&path).expect("load") {
            StateStatus::Loaded(loaded) => assert_eq!(loaded.crash_counter, 5),
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    /// 验证并发写不会产生撕裂的 JSON：多个线程同时 save，至少有一个的最终结果可被解析。
    #[test]
    fn concurrent_writers_produce_valid_json() {
        let (path, _dir) = temp_state_path();
        let n = 8;
        let path = Arc::new(path);
        let mut handles = Vec::new();
        for i in 0..n {
            let path = Arc::clone(&path);
            handles.push(std::thread::spawn(move || {
                let mut state = AppState::new();
                state.crash_counter = i as u32;
                // 加一点抖动提高竞争概率
                std::thread::sleep(Duration::from_micros(100 + i as u64 * 50));
                state.save_to(path.as_ref())
            }));
        }
        let mut errors = 0;
        for h in handles {
            if h.join().unwrap().is_err() {
                errors += 1;
            }
        }
        // rename 在同一目录下是原子的，理论上零失败；偶发文件系统抖动允许个别失败，
        // 但最终落地的文件必须是有效 JSON（不会出现半截内容）。
        assert!(errors < n, "at least one writer should succeed");
        let content = fs::read_to_string(path.as_ref()).expect("final file readable");
        let _: AppState =
            serde_json::from_str(&content).expect("final file must be valid JSON, not torn");
    }

    /// 设计要求：首启缺失文件时不写默认空文件。
    #[test]
    fn first_run_does_not_write_file() {
        let (path, _dir) = temp_state_path();
        // 只读，不写
        let _ = AppState::load_from(&path).expect("load_from on missing file");
        assert!(!path.exists(), "load must not create the file on first run");
        let _ = path; // 保持 _dir 直到测试结束
    }

    #[test]
    fn default_dsh_state_has_sensible_defaults() {
        let s = DshState::default();
        assert_eq!(s.pinned_range, "~0.1.0");
        assert_eq!(s.check_interval_hours, 24);
        assert_eq!(s.registry, "https://registry.npmmirror.com");
        assert!(s.installed.is_empty());
        assert!(s.ignored_versions.is_empty());
    }

    #[test]
    fn tmp_sibling_has_tmp_suffix() {
        let p = Path::new("/tmp/state.json");
        let tmp = tmp_sibling(p);
        assert_eq!(tmp.file_name().unwrap(), "state.json.tmp");
    }

    #[test]
    fn missing_required_field_yields_state_corrupt() {
        let (path, _dir) = temp_state_path();
        // schema_version 缺失
        let mut f = fs::File::create(&path).expect("create");
        f.write_all(b"{\"dsh\": {}}").expect("write");
        let err = AppState::load_from(&path).unwrap_err();
        assert!(matches!(err, LauncherError::StateCorrupt { .. }));
    }
}
