//! Tauri command facade.
//!
//! Command implementations are grouped by user-facing domain. Re-exports keep
//! the established command names and `lib.rs` registrations stable.

pub(crate) mod about;
pub(crate) mod bootstrap;
pub(crate) mod cli_shim;
pub(crate) mod diagnostics;
pub(crate) mod dsh_install;
pub(crate) mod host;
pub(crate) mod maintenance;
pub(crate) mod node_install;
pub(crate) mod plugin_command;
pub(crate) mod recovery;
pub(crate) mod settings;

pub use about::{get_about_info, AboutInfo};
pub use bootstrap::{
    check_dsh_update_command, get_latest_dsh_version_command, list_mirrors, probe_mirrors_command,
    resolve_bootstrap_plan_command, validate_custom_mirror_command, BootstrapPlanInfo,
    DshUpdateInfo, LatestDshVersion, MirrorInfo,
};
pub use cli_shim::{
    get_dsh_cli_status, install_dsh_cli_command, uninstall_dsh_cli_command, DshCliInstallResult,
    DshCliStatus, DshCliStatusState, DshCliUninstallResult,
};
pub use diagnostics::{export_diagnostics, export_diagnostics_to};
pub use dsh_install::{cancel_dsh_install_command, install_dsh_command, DshInstallProgressPayload};
pub use host::{
    build_status_snapshot, host_platform_arch, launcher_status, restart_host,
    restart_host_after_dsh_update, restart_host_inner, shutdown_host, start_host,
    DshUpgradeRestartResult, SharedState, StatusSnapshot,
};
pub use maintenance::{exit_app_command, rollback_dsh_command, uninstall_managed_runtime};
pub use node_install::{
    cancel_node_install_command, get_node_update_target_command, install_node_command,
    rollback_node_upgrade_command, upgrade_node_command, upgrade_node_for_dsh_update_command,
    InstallNodeArgs, NodeUpdateTarget, NodeUpgradeTransaction,
};
pub use plugin_command::{
    list_profile_plugins, run_plugin_command, InstalledPlugin, PluginCommandResult,
    ProfilePluginList,
};
pub use recovery::{
    decide_after_crash, spawn_crash_recovery, CrashAction, CrashLimitPayload, HostRestartedPayload,
};
pub use settings::{
    get_dsh_state, get_theme_command, set_node_mirror_command, set_registry_command,
    set_theme_command, DshStateSnapshot, InstalledDshInfo,
};
