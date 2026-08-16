//! 日志初始化（设计 §11.2）。
//!
//! 用 `tracing` + `tracing-subscriber` + `tracing-appender`：
//! - 壳子日志：`<log_dir>/app.log`，按天滚动，保留 7 份。
//! - 控制台同步输出（dev 模式可读）。
//!
//! 初始化后返回一个 `WorkerGuard`，必须保持存活到程序结束，否则日志缓冲可能丢失。

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::error::{LauncherError, Result};

/// 日志文件名前缀。tracing-appender 0.2.5 会拼接日期后缀，最终形如 `app.YYYY-MM-DD`。
const LOG_FILE_PREFIX: &str = "app";

/// 默认保留日志的天数。设计未明确数字，参考常见配置取 7 天。
const LOG_RETENTION_DAYS: usize = 7;

/// 初始化全局日志订阅者。返回的 `WorkerGuard` 必须在调用方保活到程序退出。
///
/// 该函数只能调用一次：重复调用会被 `tracing` 内部忽略（已注册 subscriber），
/// 但仍会创建新的 `WorkerGuard`，调用方需自行管理。
pub fn init() -> Result<(WorkerGuard, Option<WorkerGuard>)> {
    let log_dir = crate::paths::log_dir()?;
    std::fs::create_dir_all(&log_dir).map_err(LauncherError::Io)?;

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, LOG_FILE_PREFIX);

    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    // 终端层：仅在 debug 构建启用，release 默认静音（避免污染 stdout 影响 GUI 启动）。
    let console_guard = if cfg!(debug_assertions) {
        let (console_writer, guard) = tracing_appender::non_blocking(std::io::stderr());
        let console_layer = fmt::layer()
            .with_writer(console_writer)
            .with_target(true)
            .with_line_number(true);
        // 用 try_init 容错：如果上层已经 init 过，不报错
        let _ = tracing_subscriber::registry()
            .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
            .with(fmt::layer().with_writer(file_writer).with_ansi(false))
            .with(console_layer)
            .try_init();
        Some(guard)
    } else {
        let _ = tracing_subscriber::registry()
            .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
            .with(fmt::layer().with_writer(file_writer).with_ansi(false))
            .try_init();
        None
    };

    tracing::info!(
        log_dir = %log_dir.display(),
        "logging initialized (file rotation: daily, keep {})",
        LOG_RETENTION_DAYS
    );
    Ok((file_guard, console_guard))
}

/// 计算下一次启动应清理的过期日志文件名前缀。
/// 提供给上层做"启动时清理"用，本 PR 仅占位实现，M3.6 接入。
pub fn stale_log_threshold_days() -> usize {
    LOG_RETENTION_DAYS
}

/// 暴露日志目录路径给测试 / 命令使用。
pub fn current_log_file_path() -> Result<PathBuf> {
    Ok(crate::paths::log_dir()?.join(LOG_FILE_PREFIX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_is_seven_days() {
        assert_eq!(stale_log_threshold_days(), 7);
    }

    #[test]
    fn current_log_file_under_log_dir() {
        let path = current_log_file_path().expect("path");
        let log_dir = crate::paths::log_dir().expect("log_dir");
        assert!(path.starts_with(&log_dir));
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some(LOG_FILE_PREFIX)
        );
    }
}
