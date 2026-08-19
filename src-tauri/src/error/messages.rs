use super::LauncherError;

pub fn user_message(error: &LauncherError) -> String {
    match error {
        LauncherError::StateCorrupt { .. } => {
            "状态文件损坏。请在设置中导出诊断信息后重置应用数据。".to_string()
        }
        LauncherError::UnsupportedSchemaVersion(_) => {
            "状态文件由更高版本写入，无法识别。请升级启动器后再试。".to_string()
        }
        LauncherError::StateMigration { .. } => {
            "状态迁移失败。请导出诊断信息并重置应用数据。".to_string()
        }
        LauncherError::Io(error) => {
            format!("系统 IO 错误：{error}。请检查磁盘权限与剩余空间后重试。")
        }
        LauncherError::Serialization(_) => {
            "数据解析失败。请重试，若持续失败请导出诊断信息。".to_string()
        }
        LauncherError::PathResolve { .. } => {
            "无法解析用户目录。请检查 HOME 环境变量是否设置。".to_string()
        }
        LauncherError::LoggingInit(_) => "日志初始化失败。请检查日志目录写权限。".to_string(),
        LauncherError::Host(message) => host_user_message(message),
        LauncherError::Mirror(message) => mirror_user_message(message),
        LauncherError::NodeVersion(message) => {
            format!("Node 版本不满足要求（{message}）。已保留当前 dsh 版本，请在运行时更新后重试。")
        }
        LauncherError::NodeDownload(message) => node_download_user_message(message),
        LauncherError::NodeInstallCancelled { .. } => {
            "已取消 Node 运行时下载。你可以随时重新开始安装。".to_string()
        }
        LauncherError::DshRegistry(message)
            if message.contains("connect")
                || message.contains("timeout")
                || message.contains("dns") =>
        {
            "无法连接网络。请检查网络连接或镜像源设置后重试。".to_string()
        }
        LauncherError::DshRegistry(_) => {
            "查询 dsh 版本信息失败。请稍后重试，或检查 registry 地址。".to_string()
        }
        LauncherError::DshInstall(message)
            if message.contains("integrity") || message.contains("sha") =>
        {
            "dsh 安装包完整性校验失败。请重试，若持续失败请更换镜像源。".to_string()
        }
        LauncherError::DshInstall(_) => {
            "dsh 安装失败。请重试，若持续失败请导出诊断信息。".to_string()
        }
        LauncherError::DshVersion(message) if message.contains("no known_good") => {
            "没有可回滚的稳定版本。请重新安装 dsh。".to_string()
        }
        LauncherError::DshVersion(_) => "dsh 版本切换失败。请重试。".to_string(),
        LauncherError::DshCli(message) if message.contains("already exists") => {
            "目标位置已有其他 dsh 命令。为避免覆盖它，启动器没有做任何修改。".to_string()
        }
        LauncherError::DshCli(_) => {
            "无法安装 dsh 命令。请检查用户目录写权限后重试。".to_string()
        }
        LauncherError::DshPlugin(message) if message.contains("expected:") => {
            "仅支持单条安装或卸载命令：dsh plugin --profile <profile> add|remove <source>。"
                .to_string()
        }
        LauncherError::DshPlugin(message) if message.contains("timed out") => {
            "插件操作超时。请检查网络后重试，或在 dsh 中确认当前状态。".to_string()
        }
        LauncherError::DshPlugin(_) => {
            "插件操作失败。请检查来源与 profile 后重试；详情已写入应用日志。".to_string()
        }
        LauncherError::Theme(_) => "主题设置无效。请选择浅色或黑白主题。".to_string(),
        LauncherError::DshNotInstalled { .. } => "dsh 尚未安装。请完成首次启动向导。".to_string(),
        LauncherError::NodeNotInstalled { .. } => {
            "Node 运行时尚未安装。请完成首次启动向导。".to_string()
        }
        LauncherError::NodeUpgradeRequired {
            dsh_version,
            current_node,
            engines_node,
            suggested_node,
        } => format!(
            "dsh {dsh_version} 需要 Node {engines_node}，当前为 {current_node}。确认后将下载 Node {suggested_node} 并继续更新。"
        ),
    }
}

fn host_user_message(message: &str) -> String {
    if message.contains("timed out") || message.contains("readiness") {
        "dsh 启动超时（90 秒内未就绪）。请重试；若持续失败请导出诊断信息。".to_string()
    } else if message.contains("spawn") || message.contains("Operation not permitted") {
        "无法启动 dsh 子进程。请重启应用；若持续失败请导出诊断信息。".to_string()
    } else if message.contains("exited before readiness") {
        "dsh 启动后立即退出。请查看日志或回滚到稳定版本。".to_string()
    } else {
        "dsh 运行异常。请重试；若持续失败请导出诊断信息。".to_string()
    }
}

fn mirror_user_message(message: &str) -> String {
    if message.contains("connect") || message.contains("timeout") || message.contains("dns") {
        "无法连接网络。请检查网络连接后重试。".to_string()
    } else {
        "镜像源不可用。请更换镜像源后重试。".to_string()
    }
}

fn node_download_user_message(message: &str) -> String {
    let lower = message.to_lowercase();
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
    } else if lower.contains("not permitted") || lower.contains("permission") {
        "没有写入权限。请检查应用数据目录权限（必要时用管理员身份运行）。".to_string()
    } else if lower.contains("404") {
        "镜像源上找不到该版本。请确认版本号或更换镜像源。".to_string()
    } else {
        "下载失败。请重试；若持续失败请更换镜像源。".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn node_timeout_has_actionable_message() {
        assert!(
            user_message(&LauncherError::NodeDownload("timed out".to_string())).contains("超时")
        );
    }
    #[test]
    fn cancelled_node_install_has_a_clear_user_message() {
        assert!(user_message(&LauncherError::NodeInstallCancelled {
            operation_id: "operation-1".to_string(),
        })
        .contains("取消"));
    }

    #[test]
    fn node_upgrade_required_asks_the_user_to_confirm() {
        let message = user_message(&LauncherError::NodeUpgradeRequired {
            dsh_version: "0.2.0".to_string(),
            current_node: "22.19.0".to_string(),
            engines_node: ">=24.0.0".to_string(),
            suggested_node: "24.4.0".to_string(),
        });
        assert!(message.contains("0.2.0"));
        assert!(message.contains("22.19.0"));
        assert!(message.contains("24.4.0"));
        assert!(message.contains("确认"));
    }
    #[test]
    fn host_readiness_timeout_is_distinguished() {
        assert!(
            user_message(&LauncherError::Host("readiness timed out".to_string()))
                .contains("启动超时")
        );
    }
}
