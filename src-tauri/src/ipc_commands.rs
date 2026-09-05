// 暴露给前端的全部 IPC 命令，单一事实源：
// build.rs include! 本文件声明 AppManifest 生成 allow-<command> ACL 权限，
// app.rs 由此构造动态 capability。新增命令时同步 generate_handler! 列表。
pub(crate) const IPC_COMMANDS: &[&str] = &[
    // about
    "get_about_info",
    // host
    "launcher_status",
    "start_host",
    "restart_host",
    "restart_host_after_dsh_update",
    "shutdown_host",
    // bootstrap
    "list_mirrors",
    "probe_mirrors_command",
    "validate_custom_mirror_command",
    "resolve_bootstrap_plan_command",
    "get_latest_dsh_version_command",
    "check_dsh_update_command",
    // node_install
    "install_node_command",
    "upgrade_node_command",
    "upgrade_node_for_dsh_update_command",
    "rollback_node_upgrade_command",
    "get_node_update_target_command",
    "cancel_node_install_command",
    // dsh_install
    "install_dsh_command",
    "cancel_dsh_install_command",
    // settings
    "get_dsh_state",
    "get_theme_command",
    "set_node_mirror_command",
    "set_registry_command",
    "set_theme_command",
    // cli_shim
    "install_dsh_cli_command",
    "get_dsh_cli_status",
    "uninstall_dsh_cli_command",
    // plugin_command
    "run_plugin_command",
    "list_profile_plugins",
    // maintenance
    "rollback_dsh_command",
    "exit_app_command",
    "uninstall_managed_runtime",
    // diagnostics
    "export_diagnostics",
];

#[cfg(test)]
mod tests {
    use super::IPC_COMMANDS;

    // generate_handler! 的注册列表无法静态反射；用源码文本比对防止清单漂移，
    // 避免新命令漏登记导致运行时 "not allowed by ACL"。
    #[test]
    fn ipc_commands_cover_every_registered_handler() {
        let source = include_str!("app.rs");
        let block = source
            .split("generate_handler![")
            .nth(1)
            .and_then(|rest| rest.split("])").next())
            .expect("generate_handler! block not found in app.rs");

        let mut handlers: Vec<&str> = block
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .map(|line| {
                line.trim_end_matches(',')
                    .rsplit("::")
                    .next()
                    .unwrap_or(line)
            })
            .collect();
        handlers.sort_unstable();

        let mut commands = IPC_COMMANDS.to_vec();
        commands.sort_unstable();

        assert_eq!(commands, handlers);
    }
}
