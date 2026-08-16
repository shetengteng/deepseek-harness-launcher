//! PR-004 推迟到 PR-005 的集成测试。对应测试设计 §PR-004。
//!
//! 策略：不引入 Tauri mock_app（依赖图复杂），直接调 `HostSupervisor` API 验证幂等性、
//! `AlreadyShutdown` 错误路径、`HostSupervisorConfig` 默认值合理性。
//! 完整的 `start_host` → 真实子进程链路需要 mock `spawn_dsh_web`，超出本集成测试范围，
//! 由 supervisor.rs 的单元测试覆盖（append_output 截断、char boundary 等）。
//!
//! 设计 §PR-004 已注明"集成测试推迟到 PR-005 前端骨架落地后"，这里覆盖的是 supervisor
//! 在 commands 层暴露后仍可见的幂等性契约。

#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use deepseek_harness_launcher_lib::host::{
    HostSupervisor, HostSupervisorConfig, HostSupervisorError,
};

#[tokio::test]
async fn shutdown_is_idempotent() {
    let sup = Arc::new(HostSupervisor::new(HostSupervisorConfig::default()));

    sup.shutdown().await.await_completion().await;
    // 第二次调用应该是 no-op，不 panic
    sup.shutdown().await.await_completion().await;
    // 第三次也安全
    sup.shutdown().await.await_completion().await;
}

#[tokio::test]
async fn start_after_shutdown_returns_already_shutdown() {
    // `start` 需要 `Arc<HostSupervisor>`（exit monitor 持有引用支持崩溃重启，设计 §5.5）
    let sup = Arc::new(HostSupervisor::new(HostSupervisorConfig::default()));
    sup.shutdown().await.await_completion().await;

    // 构造一个明显错误的 options（node 不存在），start 应在 spawn 阶段失败。
    // 但实际上 start 在 spawn 之前就因 shutdown_flag 已 set 而返回 AlreadyShutdown。
    use deepseek_harness_launcher_lib::host::SpawnDshWebOptions;
    use std::collections::HashMap;
    use std::path::PathBuf;
    let opts = SpawnDshWebOptions {
        node_executable: PathBuf::from("/nonexistent/node"),
        cli_entry: PathBuf::from("/nonexistent/cli.js"),
        cwd: std::env::current_dir().unwrap(),
        env: HashMap::new(),
        electron_run_as_node: false,
    };
    let err = sup.start(&opts).await.unwrap_err();
    assert!(matches!(err, HostSupervisorError::AlreadyShutdown));
}

/// 验证 `HostSupervisorError::AlreadyShutdown` 的 `Display` 实现包含 "shutdown" 字样，
/// 前端 ErrorDialog 用此 message 展示。
#[test]
fn already_shutdown_message_contains_shutdown() {
    let e = HostSupervisorError::AlreadyShutdown;
    let s = e.to_string();
    assert!(
        s.to_lowercase().contains("shutdown"),
        "expected message to contain 'shutdown', got: {s}"
    );
}

/// 验证 `HostSupervisorConfig::default()` 的超时配置合理：
/// `readiness_timeout` 不能太短（避免 CI 慢机器误杀），也不能太长（拖慢测试）。
#[test]
fn default_config_has_reasonable_timeouts() {
    let cfg = HostSupervisorConfig::default();
    // 设计文档约定 90s
    assert!(cfg.readiness_timeout >= Duration::from_secs(60));
    // 测试用例不应等超过 5 分钟
    assert!(cfg.readiness_timeout < Duration::from_secs(300));
}
