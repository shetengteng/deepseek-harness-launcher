//! Launcher 错误类型。
//!
//! 对应设计 §11.1：所有用户可见错误都要可序列化到前端，结构为 `{ kind, message, data }`。
//! Tauri 命令的返回类型 `Result<T, LauncherError>` 会通过 `impl Serialize` 把错误序列化成 JSON。

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LauncherError {
    /// `state.json` 损坏：JSON 解析失败或字段缺失。
    #[error("state file is corrupt at {path}: {cause}")]
    StateCorrupt { path: PathBuf, cause: String },

    /// `state.json` 的 `schema_version` 不支持（通常意味着更高版本写入过）。
    #[error("unsupported state schema version: {0} (expected {EXPECTED_SCHEMA_VERSION})")]
    UnsupportedSchemaVersion(u8),

    /// `state.json` 在迁移过程中失败。
    #[error("state migration failed at step {step}: {cause}")]
    StateMigration { step: String, cause: String },

    /// 系统 IO 错误（读写文件、目录创建等）。
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON 序列化/反序列化失败。
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// 路径解析失败：用户目录无法确定（极端环境）。
    #[error("could not resolve {what} directory: {cause}")]
    PathResolve { what: &'static str, cause: String },

    /// 日志系统初始化失败。
    #[error("logging initialization failed: {0}")]
    LoggingInit(String),

    /// Host 监管器错误：子进程 spawn 失败、就绪超时、意外退出等。
    #[error("host supervisor error: {0}")]
    Host(String),

    /// 镜像源错误：自定义源校验失败、探活失败等（设计 §M2.1）。
    #[error("mirror error: {0}")]
    Mirror(String),

    /// Node 版本解析失败（设计 §M2.4）。
    #[error("node version error: {0}")]
    NodeVersion(String),

    /// Node 下载/校验错误（设计 §M2.2）。
    #[error("node download error: {0}")]
    NodeDownload(String),

    /// dsh registry 查询错误：HTTP 失败、JSON 解析失败、版本不存在等（设计 §M3.1）。
    #[error("dsh registry error: {0}")]
    DshRegistry(String),

    /// dsh 安装错误：npm install 失败、完整性校验失败、原子切换失败等（设计 §M3.2）。
    #[error("dsh install error: {0}")]
    DshInstall(String),

    /// dsh 版本切换错误：指针写入失败、版本未安装、回滚无 known_good 等（设计 §M3.3）。
    #[error("dsh version error: {0}")]
    DshVersion(String),

    /// dsh 未安装：state.dsh.current 为 None、current 指针不存在、cli entry 文件缺失。
    /// 前端拿到此错误后应提示用户走首启向导 / 安装 dsh 流程。
    #[error("dsh not installed: {reason}")]
    DshNotInstalled { reason: String },

    /// Node 运行时未安装：node-runtime/VERSION 文件不存在。
    /// 前端拿到此错误后应提示用户走首启向导安装 Node。
    #[error("node not installed: {reason}")]
    NodeNotInstalled { reason: String },
}

/// 当前 state.json schema 版本。`AppState::load` 会校验该值。
pub const EXPECTED_SCHEMA_VERSION: u8 = 1;

/// 序列化到前端的错误结构。对应设计 §11.1。
#[derive(Debug, serde::Serialize)]
pub struct SerializableError {
    pub kind: &'static str,
    pub message: String,
    /// 用户可见的中文文案（含可操作提示）。
    pub user_message: String,
    pub data: Option<serde_json::Value>,
}

impl LauncherError {
    /// 用户可见的中文文案 + 可操作提示（设计 §11.1）。
    /// 前端 ErrorDialog 优先展示此文案，`message` 保留给诊断。
    pub fn user_message(&self) -> String {
        match self {
            Self::StateCorrupt { .. } => {
                "状态文件损坏。请在设置中导出诊断信息后重置应用数据。".to_string()
            }
            Self::UnsupportedSchemaVersion(_) => {
                "状态文件由更高版本写入，无法识别。请升级启动器后再试。".to_string()
            }
            Self::StateMigration { .. } => {
                "状态迁移失败。请导出诊断信息并重置应用数据。".to_string()
            }
            Self::Io(e) => format!("系统 IO 错误：{e}。请检查磁盘权限与剩余空间后重试。"),
            Self::Serialization(_) => {
                "数据解析失败。请重试，若持续失败请导出诊断信息。".to_string()
            }
            Self::PathResolve { .. } => {
                "无法解析用户目录。请检查 HOME 环境变量是否设置。".to_string()
            }
            Self::LoggingInit(_) => "日志初始化失败。请检查日志目录写权限。".to_string(),
            Self::Host(inner) => host_user_message(inner),
            Self::Mirror(inner) => mirror_user_message(inner),
            Self::NodeVersion(inner) => {
                format!("Node 版本不满足要求（{inner}）。请在设置中升级 Node。")
            }
            Self::NodeDownload(inner) => node_download_user_message(inner),
            Self::DshRegistry(inner) => {
                if inner.contains("connect") || inner.contains("timeout") || inner.contains("dns") {
                    "无法连接网络。请检查网络连接或镜像源设置后重试。".to_string()
                } else {
                    "查询 dsh 版本信息失败。请稍后重试，或检查 registry 地址。".to_string()
                }
            }
            Self::DshInstall(inner) => {
                if inner.contains("integrity") || inner.contains("sha") {
                    "dsh 安装包完整性校验失败。请重试，若持续失败请更换镜像源。".to_string()
                } else {
                    "dsh 安装失败。请重试，若持续失败请导出诊断信息。".to_string()
                }
            }
            Self::DshVersion(inner) => {
                if inner.contains("no known_good") {
                    "没有可回滚的稳定版本。请重新安装 dsh。".to_string()
                } else {
                    "dsh 版本切换失败。请重试。".to_string()
                }
            }
            Self::DshNotInstalled { .. } => "dsh 尚未安装。请完成首次启动向导。".to_string(),
            Self::NodeNotInstalled { .. } => {
                "Node 运行时尚未安装。请完成首次启动向导。".to_string()
            }
        }
    }

    fn kind_str(&self) -> &'static str {
        match self {
            Self::StateCorrupt { .. } => "state_corrupt",
            Self::UnsupportedSchemaVersion(_) => "unsupported_schema_version",
            Self::StateMigration { .. } => "state_migration",
            Self::Io(_) => "io",
            Self::Serialization(_) => "serialization",
            Self::PathResolve { .. } => "path_resolve",
            Self::LoggingInit(_) => "logging_init",
            Self::Host(_) => "host",
            Self::Mirror(_) => "mirror",
            Self::NodeVersion(_) => "node_version",
            Self::NodeDownload(_) => "node_download",
            Self::DshRegistry(_) => "dsh_registry",
            Self::DshInstall(_) => "dsh_install",
            Self::DshVersion(_) => "dsh_version",
            Self::DshNotInstalled { .. } => "dsh_not_installed",
            Self::NodeNotInstalled { .. } => "node_not_installed",
        }
    }

    fn data(&self) -> Option<serde_json::Value> {
        match self {
            Self::StateCorrupt { path, .. } => Some(serde_json::json!({
                "path": path.to_string_lossy(),
            })),
            Self::UnsupportedSchemaVersion(v) => Some(serde_json::json!({
                "version": v,
                "expected": EXPECTED_SCHEMA_VERSION,
            })),
            Self::StateMigration { step, .. } => Some(serde_json::json!({
                "step": step,
            })),
            Self::PathResolve { what, .. } => Some(serde_json::json!({
                "what": what,
            })),
            Self::DshNotInstalled { reason } => Some(serde_json::json!({
                "what": "dsh",
                "reason": reason,
            })),
            Self::NodeNotInstalled { reason } => Some(serde_json::json!({
                "what": "node",
                "reason": reason,
            })),
            Self::Io(_)
            | Self::Serialization(_)
            | Self::LoggingInit(_)
            | Self::Host(_)
            | Self::Mirror(_)
            | Self::NodeVersion(_)
            | Self::NodeDownload(_)
            | Self::DshRegistry(_)
            | Self::DshInstall(_)
            | Self::DshVersion(_) => None,
        }
    }
}

/// 给 Tauri 命令做错误序列化：`Result<T, LauncherError>` 自动转成前端可读的 `{ kind, message, data }`。
impl serde::Serialize for LauncherError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        SerializableError {
            kind: self.kind_str(),
            message: self.to_string(),
            user_message: self.user_message(),
            data: self.data(),
        }
        .serialize(serializer)
    }
}

// ─── 用户文案辅助（设计 §11.1：按内部错误特征细分提示） ───

fn host_user_message(inner: &str) -> String {
    if inner.contains("timed out") || inner.contains("readiness") {
        "dsh 启动超时（90 秒内未就绪）。请重试；若持续失败请导出诊断信息。".to_string()
    } else if inner.contains("spawn") || inner.contains("Operation not permitted") {
        "无法启动 dsh 子进程。请重启应用；若持续失败请导出诊断信息。".to_string()
    } else if inner.contains("exited before readiness") {
        "dsh 启动后立即退出。请查看日志或回滚到稳定版本。".to_string()
    } else {
        "dsh 运行异常。请重试；若持续失败请导出诊断信息。".to_string()
    }
}

fn mirror_user_message(inner: &str) -> String {
    if inner.contains("connect") || inner.contains("timeout") || inner.contains("dns") {
        "无法连接网络。请检查网络连接后重试。".to_string()
    } else {
        "镜像源不可用。请更换镜像源后重试。".to_string()
    }
}

fn node_download_user_message(inner: &str) -> String {
    let lower = inner.to_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") {
        "下载超时。请检查网络后重试，或更换镜像源。".to_string()
    } else if lower.contains("connect") || lower.contains("dns") {
        "无法连接网络。请检查网络连接或镜像源设置后重试。".to_string()
    } else if lower.contains("sha") || lower.contains("mismatch") {
        "下载文件校验失败（可能被篡改或镜像损坏）。请重试或更换镜像源。".to_string()
    } else if lower.contains("too small") || lower.contains("suspiciously small") {
        "镜像返回了错误页面而非安装包。请更换镜像源后重试。".to_string()
    } else if lower.contains("disk") || lower.contains("no space") {
        "磁盘空间不足。请清理磁盘后重试（至少需要 200MB）。".to_string()
    } else if lower.contains("404") {
        "镜像源上找不到该版本。请确认版本号或更换镜像源。".to_string()
    } else {
        "下载失败。请重试；若持续失败请更换镜像源。".to_string()
    }
}

pub type Result<T> = std::result::Result<T, LauncherError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_and_data_round_trip() {
        let err = LauncherError::StateCorrupt {
            path: PathBuf::from("/tmp/state.json"),
            cause: "unexpected token at line 1 column 1".to_string(),
        };
        assert_eq!(err.kind_str(), "state_corrupt");
        let data = err.data().expect("state_corrupt has data");
        assert_eq!(data["path"], "/tmp/state.json");
    }

    #[test]
    fn unsupported_version_carries_expected() {
        let err = LauncherError::UnsupportedSchemaVersion(99);
        let data = err.data().expect("unsupported_schema_version has data");
        assert_eq!(data["version"], 99);
        assert_eq!(data["expected"], EXPECTED_SCHEMA_VERSION);
    }

    #[test]
    fn io_error_serializes_to_json() {
        let err = LauncherError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
        let json = serde_json::to_value(&err).expect("serialize");
        assert_eq!(json["kind"], "io");
        assert!(json["message"].as_str().unwrap().contains("io error"));
        assert!(json["data"].is_null());
    }

    #[test]
    fn serializable_error_includes_user_message() {
        let err = LauncherError::DshNotInstalled {
            reason: "not installed".to_string(),
        };
        let json = serde_json::to_value(&err).expect("serialize");
        assert!(json["user_message"].as_str().unwrap().contains("尚未安装"));
    }

    #[test]
    fn node_download_timeout_maps_to_network_hint() {
        let err = LauncherError::NodeDownload("GET http://x failed: operation timed out".into());
        assert!(err.user_message().contains("超时"));
    }

    #[test]
    fn node_download_connect_error_maps_to_network_hint() {
        let err = LauncherError::NodeDownload("GET http://x failed: dns error".into());
        assert!(err.user_message().contains("无法连接网络"));
    }

    #[test]
    fn node_download_small_body_maps_to_mirror_hint() {
        let err = LauncherError::NodeDownload(
            "GET http://x returned suspiciously small body (Content-Length: 200 bytes)".into(),
        );
        assert!(err.user_message().contains("错误页面"));
    }

    #[test]
    fn host_readiness_timeout_maps_to_startup_hint() {
        let err = LauncherError::Host("desktop Host readiness timed out after 90s".into());
        assert!(err.user_message().contains("启动超时"));
    }

    #[test]
    fn dsh_version_rollback_without_known_good_hint() {
        let err =
            LauncherError::DshVersion("cannot rollback: no known_good version available".into());
        assert!(err.user_message().contains("回滚"));
    }
}
