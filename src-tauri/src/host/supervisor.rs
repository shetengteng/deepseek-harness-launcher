//! `HostSupervisor`：单所有者 Host 进程监管器。
//!
//! 对应 [host-supervisor.ts](../../../deepseek-harness-desktop/apps/desktop/src/host-supervisor.ts)
//! 的 `createHostSupervisor`。
//!
//! 职责：
//! - `start()`：spawn 子进程，监听 stdout 解析就绪行，超时杀进程。
//! - `shutdown()`：SIGKILL 子进程，等待退出。
//! - `on_unexpected_exit`：跑起来后非主动关闭的退出。
//!
//! 与 TS 版的差异：
//! - Rust 用 `tokio::process::Child`，stdout/stderr 通过 `tokio::io::AsyncReadExt` 拉取。
//! - 用 `tokio::sync::OnceCell<Origin>` 缓存成功结果（失败不缓存，允许重试）。
//! - TS 版用 `kill('SIGTERM')` / `kill('SIGKILL')`；Rust 用 `child.kill()`（POSIX: SIGKILL），
//!   要做优雅终止得用 `nix` 或自己 `signal`，本 PR 简化为直接 kill。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::{oneshot, Mutex, OnceCell};
use tokio::time::timeout;
// `OnceCell` 仍用于 `start_cell`；`shutdown` 改用 `AtomicBool` + `take()` 实现幂等。

use super::lifecycle::{spawn_dsh_web, SpawnDshWebOptions};
use super::readiness::{Origin, ReadinessError, ReadinessParser, MAX_STARTUP_OUTPUT_CHARS};

/// 默认就绪超时 90 秒（与 host-supervisor.ts 对齐）。
pub const DEFAULT_READINESS_TIMEOUT: Duration = Duration::from_millis(90_000);
/// 默认优雅关闭超时 5 秒。当前实现直接 SIGKILL，此值暂未使用，保留以对齐 TS 签名。
pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(5_000);

/// 子进程退出详情。`code` 为进程 exit code，`signal` 仅 POSIX 有意义。
#[derive(Debug, Clone)]
pub struct HostExitDetail {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

/// Host 退出后的回调类型。`detail` 提供 exit code / signal。
pub type UnexpectedExitCallback = Arc<dyn Fn(HostExitDetail) + Send + Sync>;

/// supervisor 错误：对齐 host-supervisor.ts 抛出的 Error。
#[derive(Debug, Error)]
pub enum HostSupervisorError {
    #[error("desktop Host cannot start after shutdown")]
    AlreadyShutdown,
    #[error("desktop Host readiness timed out after {0:?}")]
    ReadinessTimeout(Duration),
    #[error("desktop Host failed to spawn: {0}")]
    SpawnFailed(String),
    #[error("desktop Host exited before readiness (code {code:?}, signal {signal:?})")]
    ExitedBeforeReadiness {
        code: Option<i32>,
        signal: Option<i32>,
    },
    #[error("{0}")]
    Readiness(#[from] ReadinessError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

/// supervisor 的关闭句柄。drop 时不做任何事（必须显式 `await`）。
pub struct Shutdown {
    inner: tokio::task::JoinHandle<()>,
}

impl Shutdown {
    /// 等待关闭流程完成。
    pub async fn await_completion(self) {
        let _ = self.inner.await;
    }
}

/// 诊断日志回调：每段 stdout/stderr 都会调用。
pub type LogCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// `HostSupervisor` 配置。
#[derive(Clone)]
pub struct HostSupervisorConfig {
    pub readiness_timeout: Duration,
    pub shutdown_timeout: Duration,
    /// 可选的诊断日志回调。每段 stdout/stderr 都会调用。
    pub log: Option<LogCallback>,
    /// Host 跑起来后非主动关闭的退出回调。
    pub on_unexpected_exit: Option<UnexpectedExitCallback>,
}

impl Default for HostSupervisorConfig {
    fn default() -> Self {
        Self {
            readiness_timeout: DEFAULT_READINESS_TIMEOUT,
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
            log: None,
            on_unexpected_exit: None,
        }
    }
}

struct SupervisorInner {
    child: Option<Child>,
    state: SupervisorState,
}

#[derive(Default, PartialEq, Copy, Clone)]
enum SupervisorState {
    #[default]
    Starting,
    Ready,
    Shutdown,
}

/// 单所有者 Host 监管器。`start` / `shutdown` 都可重复调用，行为幂等。
pub struct HostSupervisor {
    inner: Arc<Mutex<SupervisorInner>>,
    start_cell: OnceCell<Origin>,
    shutdown_flag: Arc<AtomicBool>,
    parser: ReadinessParser,
    output: Arc<Mutex<String>>,
    config: HostSupervisorConfig,
}

impl HostSupervisor {
    pub fn new(config: HostSupervisorConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SupervisorInner {
                child: None,
                state: SupervisorState::Starting,
            })),
            start_cell: OnceCell::new(),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            parser: ReadinessParser::new(),
            output: Arc::new(Mutex::new(String::new())),
            config,
        }
    }

    /// 启动一次（或返回已缓存的 origin）。失败不缓存，允许重试。
    pub async fn start(&self, opts: &SpawnDshWebOptions) -> Result<Origin, HostSupervisorError> {
        if self.shutdown_flag.load(Ordering::Acquire) {
            return Err(HostSupervisorError::AlreadyShutdown);
        }
        let origin = self
            .start_cell
            .get_or_try_init(|| async { self.start_inner(opts).await })
            .await?
            .clone();
        Ok(origin)
    }

    async fn start_inner(&self, opts: &SpawnDshWebOptions) -> Result<Origin, HostSupervisorError> {
        let mut child =
            spawn_dsh_web(opts).map_err(|e| HostSupervisorError::SpawnFailed(e.to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| HostSupervisorError::Other("stdout not piped".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| HostSupervisorError::Other("stderr not piped".into()))?;

        let (ready_tx, ready_rx) = oneshot::channel::<Result<Origin, ReadinessError>>();
        let ready_tx = std::sync::Mutex::new(Some(ready_tx));

        // stdout 读循环：解析就绪行 + 累积诊断输出
        let stdout_parser = self.parser.clone();
        let stdout_output = Arc::clone(&self.output);
        let stdout_log = self.config.log.clone();
        let stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut ready_sent = false;
            while let Ok(Some(line)) = lines.next_line().await {
                let chunk = format!("{line}\n");
                append_output(&stdout_output, &chunk).await;
                if let Some(log) = stdout_log.as_ref() {
                    log(&chunk);
                }
                if !ready_sent {
                    match stdout_parser.push(&chunk).await {
                        Ok(Some(origin)) => {
                            if let Some(tx) = ready_tx.lock().ok().and_then(|mut g| g.take()) {
                                let _ = tx.send(Ok(origin));
                            }
                            ready_sent = true;
                        }
                        Err(e) => {
                            if let Some(tx) = ready_tx.lock().ok().and_then(|mut g| g.take()) {
                                let _ = tx.send(Err(e));
                            }
                            ready_sent = true;
                        }
                        _ => {}
                    }
                }
            }
            // 流结束：如果还没就绪，尝试 finalize
            if !ready_sent {
                let r = stdout_parser.finalize().await;
                if let Some(tx) = ready_tx.lock().ok().and_then(|mut g| g.take()) {
                    let _ = tx.send(r);
                }
            }
        });

        // stderr 读循环：仅累积诊断
        let stderr_output = Arc::clone(&self.output);
        let stderr_log = self.config.log.clone();
        let stderr_task = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let chunk = format!("{line}\n");
                append_output(&stderr_output, &chunk).await;
                if let Some(log) = stderr_log.as_ref() {
                    log(&chunk);
                }
            }
        });

        // 等待就绪或超时
        let origin = match timeout(self.config.readiness_timeout, ready_rx).await {
            Ok(Ok(Ok(o))) => o,
            Ok(Ok(Err(e))) => {
                let _ = child.kill().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(HostSupervisorError::Readiness(e));
            }
            Ok(Err(_)) => {
                // ready_tx 被 drop（stdout_task panic）
                let _ = child.kill().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(HostSupervisorError::Other(
                    "readiness channel closed unexpectedly".into(),
                ));
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(HostSupervisorError::ReadinessTimeout(
                    self.config.readiness_timeout,
                ));
            }
        };

        // 把 child 移交 inner，spawn exit monitor
        {
            let mut inner = self.inner.lock().await;
            inner.child = Some(child);
            inner.state = SupervisorState::Ready;
        }

        let inner_arc = Arc::clone(&self.inner);
        let cb = self.config.on_unexpected_exit.clone();
        let shutdown_flag = Arc::clone(&self.shutdown_flag);
        let stdout_join = stdout_task;
        let stderr_join = stderr_task;
        tokio::spawn(async move {
            // 等 stdout/stderr 都 EOF（子进程关闭了管道）
            let _ = stdout_join.await;
            let _ = stderr_join.await;
            // 子进程已退出：拿 inner 拿 child 做 wait
            let exit = {
                let mut inner = inner_arc.lock().await;
                if let Some(mut child) = inner.child.take() {
                    match child.wait().await {
                        Ok(status) => HostExitDetail {
                            code: status.code(),
                            #[cfg(unix)]
                            signal: {
                                use std::os::unix::process::ExitStatusExt;
                                status.signal()
                            },
                            #[cfg(not(unix))]
                            signal: None,
                        },
                        Err(_) => HostExitDetail {
                            code: None,
                            signal: None,
                        },
                    }
                } else {
                    // child 已被 shutdown take 走
                    HostExitDetail {
                        code: Some(0),
                        signal: None,
                    }
                }
            };
            if !shutdown_flag.load(Ordering::Acquire) {
                if let Some(cb) = cb {
                    cb(exit);
                }
            }
        });

        Ok(origin)
    }

    /// 关闭一次（幂等）：SIGKILL 子进程，等待退出。
    /// 多次调用安全：child 被 `take()` 后后续调用是 no-op。
    /// 注意：当前简化实现直接 SIGKILL；TS 版的 SIGTERM → 5s → SIGKILL 留到后续接入 nix crate 后实现。
    pub async fn shutdown(&self) -> Shutdown {
        self.shutdown_flag.store(true, Ordering::Release);
        let inner = Arc::clone(&self.inner);
        let join = tokio::spawn(async move {
            let mut guard = inner.lock().await;
            guard.state = SupervisorState::Shutdown;
            if let Some(mut child) = guard.child.take() {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        });
        Shutdown { inner: join }
    }

    /// 当前累积的启动输出。诊断用。
    pub async fn output_snapshot(&self) -> String {
        self.output.lock().await.clone()
    }
}

/// 追加输出到环形缓冲。超过 `MAX_STARTUP_OUTPUT_CHARS` 从头部截断。
async fn append_output(buf: &Arc<Mutex<String>>, chunk: &str) {
    let mut guard = buf.lock().await;
    guard.push_str(chunk);
    let len = guard.len();
    if len > MAX_STARTUP_OUTPUT_CHARS {
        let cut = len - MAX_STARTUP_OUTPUT_CHARS;
        // 从字符边界切，避免切到 UTF-8 中间
        let mut boundary = cut;
        while boundary < len && !guard.is_char_boundary(boundary) {
            boundary += 1;
        }
        let drained: String = guard.drain(..boundary).collect();
        let _ = drained;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn shutdown_without_start_is_noop() {
        let sup = HostSupervisor::new(HostSupervisorConfig::default());
        sup.shutdown().await.await_completion().await;
        // shutdown_flag 应为 true
        assert!(sup.shutdown_flag.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn start_after_shutdown_errors() {
        let sup = HostSupervisor::new(HostSupervisorConfig::default());
        sup.shutdown().await.await_completion().await;
        let opts = SpawnDshWebOptions {
            node_executable: PathBuf::from("node"),
            cli_entry: PathBuf::from("/tmp/dsh/bin.js"),
            cwd: std::env::current_dir().unwrap(),
            env: std::collections::HashMap::new(),
            electron_run_as_node: false,
        };
        let err = sup.start(&opts).await.unwrap_err();
        assert!(matches!(err, HostSupervisorError::AlreadyShutdown));
    }

    #[tokio::test]
    async fn append_output_truncates_head_when_over_limit() {
        let buf = Arc::new(Mutex::new(String::new()));
        let big = "x".repeat(MAX_STARTUP_OUTPUT_CHARS + 100);
        append_output(&buf, &big).await;
        let s = buf.lock().await.clone();
        assert_eq!(s.len(), MAX_STARTUP_OUTPUT_CHARS);
    }

    #[tokio::test]
    async fn append_output_respects_char_boundary() {
        let buf = Arc::new(Mutex::new(String::new()));
        // 中文字符 3 字节，故意制造非边界对齐
        let mut big = String::new();
        for _ in 0..(MAX_STARTUP_OUTPUT_CHARS / 3 + 100) {
            big.push('中');
        }
        append_output(&buf, &big).await;
        let s = buf.lock().await.clone();
        assert!(s.len() <= MAX_STARTUP_OUTPUT_CHARS + 3); // 至多补齐一个字符
        assert!(s.is_char_boundary(s.len()));
    }
}
