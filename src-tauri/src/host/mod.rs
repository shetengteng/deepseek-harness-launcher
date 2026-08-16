//! Host 监管子模块。
//!
//! 对应设计 §3 和 M1.3 计划，参照
//! [host-supervisor.ts](../../../deepseek-harness-desktop/apps/desktop/src/host-supervisor.ts) 的 Rust 等价物。
//!
//! 模块组织：
//!  - [`readiness`]：增量解析 `dsh web: <url>` 就绪行，校验 loopback + 显式端口。
//!  - [`supervisor`]：`HostSupervisor` 持有 `tokio::process::Child`，管理 spawn/shutdown/超时杀进程。
//!  - [`lifecycle`]：`spawnDshWeb` 等价物，过滤环境变量后 spawn `node --expose-internals <cli_entry> web`。

pub mod lifecycle;
pub mod readiness;
pub mod supervisor;

pub use lifecycle::{filtered_env, spawn_dsh_web, SpawnDshWebOptions};
pub use readiness::{Origin, ReadinessError, ReadinessParser, READINESS_PREFIX};
pub use supervisor::{
    HostExitDetail, HostSupervisor, HostSupervisorConfig, HostSupervisorError, LogCallback,
    Shutdown, UnexpectedExitCallback, DEFAULT_READINESS_TIMEOUT, DEFAULT_SHUTDOWN_TIMEOUT,
};
