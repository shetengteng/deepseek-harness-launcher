//! `spawnDshWeb` 等价物：用托管 Node 运行时启动 `dsh web`。
//!
//! 对应 [host-supervisor.ts](../../../deepseek-harness-desktop/apps/desktop/src/host-supervisor.ts)
//! 的 `spawnDshWeb` 与 `SpawnDshWebOptions`。
//!
//! 设计 §M1.3 要求：
//! - 命令：`<node> --expose-internals <cli_entry> web --host 127.0.0.1 --port 0`
//! - `stdio: ['ignore', 'pipe', 'pipe']`，`windowsHide: true`
//! - 环境变量过滤：剥离 `RUST_*`、`TAURI_*`、`npm_*`，只透传 `DSH_*`、`PATH`、`HOME`/`USERPROFILE`、`LANG`、`LC_*`
//! - M1 阶段 `nodeExecutable` 直接读 `state.node.binary` 或回退系统 `PATH` 上的 `node`；
//!   `cliEntry` 读环境变量 `DSH_CLI_ENTRY`（开发者手动指定）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

use crate::error::{LauncherError, Result};

/// 启动 `dsh web` 子进程的参数。
#[derive(Debug, Clone)]
pub struct SpawnDshWebOptions {
    /// Node 可执行文件路径。M1 阶段从 `state.node.binary` 或 PATH 上的 `node` 解析。
    pub node_executable: PathBuf,
    /// dsh CLI 入口 JS 文件（`lib/bin.js`）。M1 阶段从 `DSH_CLI_ENTRY` 环境变量读取。
    pub cli_entry: PathBuf,
    /// 工作目录：Host 进程及其派生 session / 工具的 cwd。
    pub cwd: PathBuf,
    /// 已过滤的环境变量（调用方负责用 [`filtered_env`] 生成）。
    pub env: HashMap<String, String>,
    /// Electron 走 Node 模式时设为 `true`，会注入 `ELECTRON_RUN_AS_NODE=1`。
    /// 当前 launcher 不打包 Electron，恒为 `false`，保留字段对齐 TS 签名。
    pub electron_run_as_node: bool,
}

impl SpawnDshWebOptions {
    /// 解析 M1 阶段的默认 `node_executable`：
    /// - 优先用 `state.node.binary`（PR-005 落地后会填）；
    /// - 否则回退 `PATH` 上的 `node`。
    ///
    /// 当前 PR 不读 state，仅返回 `Ok("node")`，调用方（commands 层）负责把 state 注入。
    pub fn default_node_executable() -> PathBuf {
        PathBuf::from("node")
    }

    /// 从环境变量 `DSH_CLI_ENTRY` 解析 cli_entry。
    /// M1 阶段要求开发者手动指定，未设置时返回 `Err`。
    pub fn cli_entry_from_env() -> Result<PathBuf> {
        std::env::var("DSH_CLI_ENTRY")
            .map(PathBuf::from)
            .map_err(|_| LauncherError::PathResolve {
                what: "cli_entry",
                cause: "DSH_CLI_ENTRY environment variable is not set".to_string(),
            })
    }
}

/// 从当前进程环境变量生成 Host 子进程的过滤后环境。
///
/// 设计 §M1.3：剥离 `RUST_*`、`TAURI_*`、`npm_*`，只透传：
/// - `DSH_*`（dsh 自身配置）
/// - `PATH`（子进程找 node_modules、git 等依赖）
/// - `HOME` / `USERPROFILE`（用户目录解析）
/// - `LANG` / `LC_*`（locale）
pub fn filtered_env() -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (k, v) in std::env::vars() {
        if is_passthrough(&k) {
            out.insert(k, v);
        }
    }
    out
}

fn is_passthrough(key: &str) -> bool {
    if key.starts_with("RUST_")
        || key.starts_with("TAURI_")
        || key.starts_with("npm_")
        // 显式拒绝 npm_config_*、npm_lifecycle_* 等 npm 注入变量（不以上划线开头）
        || key.starts_with("npm_config_")
        || key.starts_with("npm_lifecycle_")
        || key.starts_with("npm_package_")
    {
        return false;
    }
    if key == "PATH"
        || key == "HOME"
        || key == "USERPROFILE"
        || key == "LANG"
        || key == "DSH_CLI_ENTRY" // dsh 自身入口变量也透传，方便子进程再 spawn
        || key.starts_with("DSH_")
        || key.starts_with("LC_")
    {
        return true;
    }
    false
}

/// Spawn `dsh web` 子进程，返回 `tokio::process::Child`。
///
/// - `stdio`：stdin ignore、stdout pipe、stderr pipe（供 supervisor 监听就绪行与诊断）
/// - `windows_hide`：true（Windows 上不弹控制台窗口）
/// - `kill_on_drop`：true（supervisor 提前 drop Child 时杀进程，避免泄漏）
pub fn spawn_dsh_web(opts: &SpawnDshWebOptions) -> Result<Child> {
    let mut cmd = Command::new(&opts.node_executable);
    cmd.current_dir(&opts.cwd);
    cmd.args([
        "--expose-internals",
        opts.cli_entry
            .to_str()
            .ok_or_else(|| LauncherError::PathResolve {
                what: "cli_entry",
                cause: format!("path is not valid UTF-8: {}", opts.cli_entry.display()),
            })?,
        "web",
        "--host",
        "127.0.0.1",
        "--port",
        "0",
    ]);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.kill_on_drop(true);

    // 注入环境变量
    cmd.env_clear();
    for (k, v) in &opts.env {
        cmd.env(k, v);
    }
    if opts.electron_run_as_node {
        cmd.env("ELECTRON_RUN_AS_NODE", "1");
    }

    spawn_command(&mut cmd).map_err(LauncherError::Io)
}

const SPAWN_BUSY_ATTEMPTS: u32 = 8;

// Linux exec of a file still open for write returns ETXTBSY (26).
fn spawn_command(cmd: &mut Command) -> std::io::Result<Child> {
    let mut last_error = None;
    for attempt in 0..SPAWN_BUSY_ATTEMPTS {
        match cmd.spawn() {
            Ok(child) => return Ok(child),
            Err(error) if is_executable_busy(&error) => {
                last_error = Some(error);
                std::thread::sleep(Duration::from_millis(20 * u64::from(attempt + 1)));
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.expect("ETXTBSY retry"))
}

fn is_executable_busy(error: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(26)
    }
    #[cfg(not(unix))]
    {
        let _ = error;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_includes_path_and_home() {
        assert!(is_passthrough("PATH"));
        assert!(is_passthrough("HOME"));
        assert!(is_passthrough("USERPROFILE"));
        assert!(is_passthrough("LANG"));
    }

    #[test]
    fn passthrough_includes_dsh_prefix() {
        assert!(is_passthrough("DSH_FOO"));
        assert!(is_passthrough("DSH_BAR"));
    }

    #[test]
    fn passthrough_includes_lc_prefix() {
        assert!(is_passthrough("LC_ALL"));
        assert!(is_passthrough("LC_CTYPE"));
    }

    #[test]
    fn passthrough_excludes_rust_prefix() {
        assert!(!is_passthrough("RUST_BACKTRACE"));
        assert!(!is_passthrough("RUSTC_COLOR"));
    }

    #[test]
    fn passthrough_excludes_tauri_prefix() {
        assert!(!is_passthrough("TAURI_DEV"));
        assert!(!is_passthrough("TAURI_PLATFORM"));
    }

    #[test]
    fn passthrough_excludes_npm_prefix() {
        assert!(!is_passthrough("npm_config_user_agent"));
        assert!(!is_passthrough("npm_lifecycle_event"));
        assert!(!is_passthrough("npm_package_name"));
        // npm_*（带下划线）也被拒绝
        assert!(!is_passthrough("npm_user_agent"));
    }

    #[test]
    fn filtered_env_drops_disallowed_keys() {
        std::env::set_var("RUST_TEST_FILTERED", "1");
        std::env::set_var("TAURI_TEST_FILTERED", "1");
        std::env::set_var("DSH_TEST_FILTERED", "1");
        let env = filtered_env();
        assert!(!env.contains_key("RUST_TEST_FILTERED"));
        assert!(!env.contains_key("TAURI_TEST_FILTERED"));
        assert!(env.contains_key("DSH_TEST_FILTERED"));
        std::env::remove_var("RUST_TEST_FILTERED");
        std::env::remove_var("TAURI_TEST_FILTERED");
        std::env::remove_var("DSH_TEST_FILTERED");
    }

    // 环境变量测试用互斥锁串行化，避免并行测试竞争 DSH_CLI_ENTRY
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn cli_entry_from_env_errors_when_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        // 暂存旧值，避免污染其他测试
        let old = std::env::var("DSH_CLI_ENTRY").ok();
        std::env::remove_var("DSH_CLI_ENTRY");
        let err = SpawnDshWebOptions::cli_entry_from_env().unwrap_err();
        assert!(matches!(err, LauncherError::PathResolve { what, .. } if what == "cli_entry"));
        if let Some(v) = old {
            std::env::set_var("DSH_CLI_ENTRY", v);
        }
    }

    #[test]
    fn cli_entry_from_env_returns_path_when_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old = std::env::var("DSH_CLI_ENTRY").ok();
        std::env::set_var("DSH_CLI_ENTRY", "/tmp/dsh/bin.js");
        let p = SpawnDshWebOptions::cli_entry_from_env().expect("ok");
        assert_eq!(p, PathBuf::from("/tmp/dsh/bin.js"));
        match old {
            Some(v) => std::env::set_var("DSH_CLI_ENTRY", v),
            None => std::env::remove_var("DSH_CLI_ENTRY"),
        }
    }

    #[test]
    fn executable_busy_matches_unix_etxtbsy() {
        let busy = std::io::Error::from_raw_os_error(26);
        let missing = std::io::Error::from_raw_os_error(2);
        #[cfg(unix)]
        {
            assert!(is_executable_busy(&busy));
            assert!(!is_executable_busy(&missing));
        }
        #[cfg(not(unix))]
        {
            assert!(!is_executable_busy(&busy));
            assert!(!is_executable_busy(&missing));
        }
    }
}
