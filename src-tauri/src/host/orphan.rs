//! 清理不受当前 supervisor 管理的残留 `dsh web` 进程。
//!
//! App 崩溃、release profile `panic = "abort"`（绕过 Drop）或被强杀时，
//! supervisor 来不及终止子进程，dsh 成为无人监管的孤儿进程（macOS 上被
//! launchd 收养）并继续持有 session 写锁，新实例 resume 该 session 会报
//! `SessionAlreadyOwnedError`。重启前按命令行特征扫描托管进程并清理这些残留。

use std::ffi::OsString;
use std::path::Path;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::dsh::DSH_ENTRY_REL;
use crate::error::{LauncherError, Result};

/// dsh 入口在 Windows 命令行里的反斜杠形式（`spawn_dsh_web` 传入的
/// `cli_entry` 由 `PathBuf::join` 构造，Windows 上是 `\` 分隔）。
const DSH_ENTRY_REL_WINDOWS: &str = "lib\\bin.js";

/// 命令行特征匹配托管 dsh web 进程。三个特征都是我们 spawn 时的固定
/// 参数（设计 §M1.3）：
/// - 托管 node 二进制路径（`<data_dir>/node-runtime/` 下）
/// - 托管 dsh 入口 `<data_dir>/dsh/<version>/.../lib/bin.js`（版本号取自
///   进程命令行而非 current 指针，旧版本残留同样命中）
/// - `--no-open`（比 `web` 单词更特异：用户手动 `dsh web` 不带该参数，不会被误杀）
///
/// 用整条命令行子串匹配而非逐参数匹配：macOS/Linux 的 argv 是内核结构，
/// 路径含空格无损；Windows 上 sysinfo 把命令行字符串按空格切分，含空格
/// 的安装路径（如 `C:\Users\John Smith\...`）会被碎化、引号残留在片段里，
/// 逐参数匹配失效，而碎化片段 join 回来后子串仍完整出现。
fn is_managed_dsh_web(cmd: &[OsString], node_runtime: &Path, dsh_dir: &Path) -> bool {
    if cmd.is_empty() {
        return false;
    }
    let joined = cmd
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let has_node = joined.contains(node_runtime.to_string_lossy().as_ref());
    let has_entry = joined.contains(dsh_dir.to_string_lossy().as_ref())
        && (joined.contains(DSH_ENTRY_REL) || joined.contains(DSH_ENTRY_REL_WINDOWS));
    has_node && has_entry && joined.contains("--no-open")
}

fn refreshed_system() -> System {
    let mut system = System::new();
    // 默认 refresh 不拉取命令行（macOS 上 cmd 为空），必须显式要求。
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything().with_cmd(UpdateKind::Always),
    );
    system
}

fn stale_candidates(
    system: &System,
    node_runtime: &Path,
    dsh_dir: &Path,
    exclude_pid: Option<u32>,
) -> Vec<u32> {
    let own_pid = std::process::id();
    system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let pid = pid.as_u32();
            (pid != own_pid
                && Some(pid) != exclude_pid
                && is_managed_dsh_web(process.cmd(), node_runtime, dsh_dir))
            .then_some(pid)
        })
        .collect()
}

/// 只扫不杀，供测试轮询进程表。
#[cfg(test)]
#[cfg(unix)]
fn scan_stale_sync(node_runtime: &Path, dsh_dir: &Path, exclude_pid: Option<u32>) -> Vec<u32> {
    stale_candidates(&refreshed_system(), node_runtime, dsh_dir, exclude_pid)
}

/// SIGKILL 直接终止：残留进程持锁的意义只在它活着，优雅退出与否不影响锁释放。
pub fn kill_stale_sync(node_runtime: &Path, dsh_dir: &Path, exclude_pid: Option<u32>) -> Vec<u32> {
    let system = refreshed_system();
    stale_candidates(&system, node_runtime, dsh_dir, exclude_pid)
        .into_iter()
        .filter(|&pid| {
            let killed = system
                .process(sysinfo::Pid::from_u32(pid))
                .map(|process| process.kill())
                .unwrap_or(false);
            if killed {
                tracing::info!(pid, "killed stale dsh web process");
            } else {
                tracing::warn!(pid, "failed to kill stale dsh web process");
            }
            killed
        })
        .collect()
}

/// 重启前清理残留 dsh 进程。目录解析失败或扫描线程异常时返回 `Err`，
/// 调用方记日志后继续重启，不因清理失败阻塞用户。
pub async fn kill_stale_dsh_processes(exclude_pid: Option<u32>) -> Result<Vec<u32>> {
    let node_runtime = crate::paths::node_runtime_dir()?;
    let dsh_dir = crate::paths::dsh_dir()?;
    tokio::task::spawn_blocking(move || kill_stale_sync(&node_runtime, &dsh_dir, exclude_pid))
        .await
        .map_err(|error| LauncherError::Host(format!("stale dsh process scan aborted: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // 匹配是纯词法路径比较，无需真实文件系统。
    fn fake_root() -> PathBuf {
        PathBuf::from("/Users/u/Library/Application Support/deepseek-harness-launcher")
    }

    fn dirs(root: &Path) -> (PathBuf, PathBuf) {
        (root.join("node-runtime"), root.join("dsh"))
    }

    fn managed_argv(root: &Path, version: &str) -> Vec<OsString> {
        vec![
            root.join("node-runtime/node-v24.18.1/bin/node")
                .into_os_string(),
            OsString::from("--expose-internals"),
            root.join(format!(
                "dsh/{version}/node_modules/@deepseek-ai/dsh/lib/bin.js"
            ))
            .into_os_string(),
            OsString::from("web"),
            OsString::from("--host"),
            OsString::from("127.0.0.1"),
            OsString::from("--port"),
            OsString::from("0"),
            OsString::from("--no-open"),
        ]
    }

    #[test]
    fn matches_real_invocation_for_any_installed_version() {
        let root = fake_root();
        let (node_runtime, dsh_dir) = dirs(&root);
        for version in ["0.1.5-rc.2", "0.2.0"] {
            assert!(
                is_managed_dsh_web(&managed_argv(&root, version), &node_runtime, &dsh_dir),
                "should match managed dsh web process (version {version})"
            );
        }
    }

    #[test]
    fn rejects_system_node_running_dsh() {
        let root = fake_root();
        let (node_runtime, dsh_dir) = dirs(&root);
        let mut argv = managed_argv(&root, "0.2.0");
        argv[0] = OsString::from("/usr/local/bin/node");
        assert!(!is_managed_dsh_web(&argv, &node_runtime, &dsh_dir));
    }

    #[test]
    fn rejects_invocation_without_no_open_flag() {
        let root = fake_root();
        let (node_runtime, dsh_dir) = dirs(&root);
        let argv = vec![
            root.join("node-runtime/node-v24.18.1/bin/node")
                .into_os_string(),
            OsString::from("--expose-internals"),
            root.join("dsh/0.2.0/node_modules/@deepseek-ai/dsh/lib/bin.js")
                .into_os_string(),
            OsString::from("repl"),
        ];
        assert!(!is_managed_dsh_web(&argv, &node_runtime, &dsh_dir));
    }

    /// 模拟 Windows sysinfo 的行为：命令行是单条字符串，按空格切分后
    /// 含空格的路径被碎化、引号残留在片段首尾。join 回来后子串匹配必须
    /// 仍然命中。
    #[test]
    fn matches_when_spacey_paths_are_fragmented_by_space_splitting() {
        let root = fake_root();
        let (node_runtime, dsh_dir) = dirs(&root);
        let node = root.join("node-runtime/node-v24.18.1/bin/node");
        let entry = root.join("dsh/0.2.0/node_modules/@deepseek-ai/dsh/lib/bin.js");
        let cmd_line = format!(
            "\"{}\" --expose-internals \"{}\" web --no-open",
            node.display(),
            entry.display()
        );
        let fragmented: Vec<OsString> = cmd_line.split(' ').map(OsString::from).collect();
        assert!(
            is_managed_dsh_web(&fragmented, &node_runtime, &dsh_dir),
            "fragmented command line should still match: {cmd_line}"
        );
    }

    #[test]
    fn rejects_dsh_entry_outside_managed_dsh_dir() {
        let root = fake_root();
        let (node_runtime, dsh_dir) = dirs(&root);
        let argv = vec![
            root.join("node-runtime/node-v24.18.1/bin/node")
                .into_os_string(),
            OsString::from("--expose-internals"),
            PathBuf::from("/tmp/other-dsh/node_modules/@deepseek-ai/dsh/lib/bin.js")
                .into_os_string(),
            OsString::from("web"),
            OsString::from("--no-open"),
        ];
        assert!(!is_managed_dsh_web(&argv, &node_runtime, &dsh_dir));
    }

    #[test]
    fn rejects_managed_node_running_unrelated_script() {
        let root = fake_root();
        let (node_runtime, dsh_dir) = dirs(&root);
        let argv = vec![
            root.join("node-runtime/node-v24.18.1/bin/node")
                .into_os_string(),
            OsString::from("server.js"),
            OsString::from("web"),
            OsString::from("--no-open"),
        ];
        assert!(!is_managed_dsh_web(&argv, &node_runtime, &dsh_dir));
    }

    #[test]
    fn rejects_empty_argv() {
        let root = fake_root();
        let (node_runtime, dsh_dir) = dirs(&root);
        assert!(!is_managed_dsh_web(&[], &node_runtime, &dsh_dir));
    }

    /// Windows 真实形态：反斜杠路径、含空格的用户名、引号包裹、
    /// `lib\bin.js` 分隔符。非 Windows 平台跑不了（`\` 在 unix 上不是
    /// 路径分隔符），依赖上述跨平台碎化测试兜底等价逻辑。
    #[cfg(windows)]
    mod windows {
        use super::*;

        fn windows_argv() -> Vec<OsString> {
            let cmd_line = "\"C:\\Users\\John Smith\\AppData\\Roaming\\io.deepseek\\DeepSeek\\deepseek-harness-launcher\\node-runtime\\node-v24.18.1\\node.exe\" --expose-internals \"C:\\Users\\John Smith\\AppData\\Roaming\\io.deepseek\\DeepSeek\\deepseek-harness-launcher\\dsh\\0.1.5-rc.2\\node_modules\\@deepseek-ai\\dsh\\lib\\bin.js\" web --host 127.0.0.1 --port 0 --no-open";
            cmd_line.split(' ').map(OsString::from).collect()
        }

        fn windows_root() -> PathBuf {
            PathBuf::from(
                r"C:\Users\John Smith\AppData\Roaming\io.deepseek\DeepSeek\deepseek-harness-launcher",
            )
        }

        #[test]
        fn matches_windows_style_command_line() {
            let root = windows_root();
            let (node_runtime, dsh_dir) = dirs(&root);
            assert!(is_managed_dsh_web(&windows_argv(), &node_runtime, &dsh_dir));
        }

        #[test]
        fn rejects_foreign_installation_root() {
            let foreign = PathBuf::from(r"D:\other-launcher");
            assert!(!is_managed_dsh_web(
                &windows_argv(),
                &foreign.join("node-runtime"),
                &foreign.join("dsh")
            ));
        }
    }

    #[cfg(unix)]
    mod integration {
        use super::*;
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        /// `/bin/sh` 的 `-c` 把命令串之后的参数一律当位置参数，不会像
        /// `yes`/`sleep` 那样被 getopt 当未知选项直接退出；`arg0` 把
        /// argv[0] 伪造成托管 node 路径，命令行特征与真实 dsh web 一致。
        fn spawn_stale(root: &Path) -> std::process::Child {
            let bin_dir = root.join("node-runtime/node-v24.18.1/bin");
            std::fs::create_dir_all(&bin_dir).expect("mkdir node bin");
            let fake_node = bin_dir.join("node");
            let entry_dir = root.join("dsh/9.9.9/node_modules/@deepseek-ai/dsh/lib");
            std::fs::create_dir_all(&entry_dir).expect("mkdir dsh lib");
            let entry = entry_dir.join("bin.js");
            std::fs::write(&entry, b"").expect("touch entry");
            Command::new("/bin/sh")
                .arg0(&fake_node)
                .args([
                    "-c",
                    "while :; do sleep 1; done",
                    "--expose-internals",
                    entry.to_str().expect("utf8 entry"),
                    "web",
                    "--host",
                    "127.0.0.1",
                    "--port",
                    "0",
                    "--no-open",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn stale mock")
        }

        #[test]
        fn kills_orphans_and_spares_bystanders() {
            let temp = tempfile::tempdir().expect("tempdir");
            let root = temp.path();
            let (node_runtime, dsh_dir) = dirs(root);
            let mut stale = spawn_stale(root);
            let mut bystander = Command::new("/bin/sleep")
                .arg("30")
                .spawn()
                .expect("spawn bystander");
            let stale_pid = stale.id();
            let bystander_pid = bystander.id();

            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let scanned = scan_stale_sync(&node_runtime, &dsh_dir, None);
                assert!(
                    !scanned.contains(&bystander_pid),
                    "bystander must not match stale criteria"
                );
                if scanned.contains(&stale_pid) {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "stale process never appeared in the process table"
                );
                std::thread::sleep(Duration::from_millis(100));
            }

            let excluded = kill_stale_sync(&node_runtime, &dsh_dir, Some(stale_pid));
            assert!(
                excluded.is_empty(),
                "excluded pid must survive: {excluded:?}"
            );

            let killed = kill_stale_sync(&node_runtime, &dsh_dir, None);
            assert_eq!(killed, vec![stale_pid]);

            let status = stale.wait().expect("wait stale");
            assert!(!status.success(), "SIGKILL should terminate the orphan");
            assert!(
                bystander.try_wait().expect("try_wait bystander").is_none(),
                "bystander must stay alive"
            );
            bystander.kill().expect("cleanup bystander");
            let _ = bystander.wait();
        }
    }
}
