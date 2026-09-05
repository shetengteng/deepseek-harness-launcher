use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Manager, RunEvent, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::{commands, logging, navigation, node::NodeDownloadOperations, paths, tray};

#[cfg(target_os = "macos")]
use crate::platform;

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";

pub(crate) struct ExitCoordinator {
    requested: AtomicBool,
}

impl ExitCoordinator {
    pub(crate) fn request(&self) -> bool {
        self.requested
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub(crate) fn requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "macos")]
    if let Err(error) = platform::ensure_self_signed_macos() {
        eprintln!("warning: self-sign check failed: {error:?}");
    }
    let _guards = match logging::init() {
        Ok(guards) => guards,
        Err(error) => {
            eprintln!("fatal: logging init failed: {error:?}");
            panic!("logging init failed: {error:?}");
        }
    };
    if let Err(error) = paths::ensure_dirs() {
        eprintln!("warning: ensure_dirs failed: {error:?}");
    }
    #[cfg(not(dev))]
    let frontend_port =
        portpicker::pick_unused_port().expect("failed to pick a loopback port for the frontend");

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(navigation::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    #[cfg(not(dev))]
    let builder = builder.plugin(
        tauri_plugin_localhost::Builder::new(frontend_port)
            .host("127.0.0.1")
            .build(),
    );

    builder
        .manage(commands::SharedState::new())
        .manage(crate::dsh::DshInstallOperations::default())
        .manage(NodeDownloadOperations::default())
        .manage(ExitCoordinator {
            requested: AtomicBool::new(false),
        })
        .setup(move |app| {
            // 前端 origin 必须与 dsh（--host 127.0.0.1）同 site，否则 dsh 颁发的
            // SameSite=Strict cookie 会被 WebKit 当作跨站 iframe cookie 丢弃
            // （详见 design/dsh-auth-cookie-samesite-fix.md）。dev 的 origin 直接
            // 取自 tauri.conf.json 的 devUrl，避免两处重复定义。
            #[cfg(dev)]
            let (frontend_url, window_url) = (
                app.config()
                    .build
                    .dev_url
                    .clone()
                    .expect("devUrl must be set in tauri.conf.json"),
                WebviewUrl::App(std::path::PathBuf::from("/")),
            );
            #[cfg(not(dev))]
            let (frontend_url, window_url) = {
                let addr = format!("127.0.0.1:{frontend_port}");
                if !wait_for_loopback_server(&addr) {
                    report_frontend_server_failure(app);
                    return Ok(());
                }
                let url = Url::parse(&format!("http://{addr}"))
                    .expect("frontend loopback origin is a valid URL");
                (url.clone(), WebviewUrl::External(url))
            };

            // 前端运行在回环 origin（非内置协议）上：remote 上下文的 IPC 全部走
            // ACL 检查，应用自身命令也必须显式授权，否则启动即
            // "Command launcher_status not allowed by ACL"。capability 同时覆盖
            // local（dev 下相对 devUrl 判定为 local）与 remote（回环 URL）上下文。
            register_loopback_capability(app, &frontend_url)?;

            // 必须先放行前端 origin，再创建窗口：首次导航也经过 navigation policy。
            app.state::<commands::SharedState>()
                .navigation
                .activate_launcher_origin(frontend_url.as_str());

            let handle = app.handle().clone();
            app.state::<commands::SharedState>()
                .supervisor
                .set_exit_handler(std::sync::Arc::new(move |detail| {
                    commands::spawn_crash_recovery(handle.clone(), detail)
                }));
            tray::setup(app)?;
            let window = WebviewWindowBuilder::new(app, MAIN_WINDOW_LABEL, window_url)
                .title("DeepSeek Harness Launcher")
                .inner_size(1280.0, 840.0)
                .min_inner_size(960.0, 600.0)
                .theme(Some(tauri::Theme::Light))
                .build()?;
            commands::settings::apply_persisted_window_theme(&window);
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<ExitCoordinator>().requested() {
                    return;
                }
                api.prevent_close();
                if let Err(error) = window.hide() {
                    tracing::warn!(%error, "failed to hide main window");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::about::get_about_info,
            commands::host::launcher_status,
            commands::host::start_host,
            commands::host::restart_host,
            commands::host::restart_host_after_dsh_update,
            commands::host::shutdown_host,
            commands::bootstrap::list_mirrors,
            commands::bootstrap::probe_mirrors_command,
            commands::bootstrap::validate_custom_mirror_command,
            commands::bootstrap::resolve_bootstrap_plan_command,
            commands::bootstrap::get_latest_dsh_version_command,
            commands::bootstrap::check_dsh_update_command,
            commands::node_install::install_node_command,
            commands::node_install::upgrade_node_command,
            commands::node_install::upgrade_node_for_dsh_update_command,
            commands::node_install::rollback_node_upgrade_command,
            commands::node_install::get_node_update_target_command,
            commands::node_install::cancel_node_install_command,
            commands::dsh_install::install_dsh_command,
            commands::dsh_install::cancel_dsh_install_command,
            commands::settings::get_dsh_state,
            commands::settings::get_theme_command,
            commands::settings::set_node_mirror_command,
            commands::settings::set_registry_command,
            commands::settings::set_theme_command,
            commands::cli_shim::install_dsh_cli_command,
            commands::cli_shim::get_dsh_cli_status,
            commands::cli_shim::uninstall_dsh_cli_command,
            commands::plugin_command::run_plugin_command,
            commands::plugin_command::list_profile_plugins,
            commands::maintenance::rollback_dsh_command,
            commands::maintenance::exit_app_command,
            commands::diagnostics::export_diagnostics,
            commands::maintenance::uninstall_managed_runtime,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = &event
            {
                tray::show_main_window(app);
            }
            if let RunEvent::ExitRequested { api, .. } = event {
                if app.state::<ExitCoordinator>().requested() {
                    return;
                }
                api.prevent_exit();
                tray::request_exit(app.clone());
            }
        });
}

// 前端服务器在插件自己的线程里启动（bind 失败只会在该线程 panic）；
// 窗口加载前确认端口已可连，失败时返回 false 由调用方给出可操作的错误。
#[cfg(not(dev))]
fn wait_for_loopback_server(addr: &str) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if std::net::TcpStream::connect(addr).is_ok() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    tracing::error!(addr, "frontend loopback server did not accept connections in time");
    false
}

// 设计 §11.1：用户可见错误必须可操作。与其让窗口加载死 URL 得到白屏，
// 不如弹出错误说明并退出，用户重试或导出诊断。
#[cfg(not(dev))]
fn report_frontend_server_failure(app: &tauri::App) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

    let handle = app.handle().clone();
    app.dialog()
        .message("无法启动本地前端服务，请重新启动应用。若持续出现，请通过设置导出诊断日志并反馈。")
        .title("DeepSeek Harness Launcher")
        .kind(MessageDialogKind::Error)
        .show(move |_| {
            handle.state::<ExitCoordinator>().request();
            handle.exit(1);
        });
}

// 授予回环前端窗口全部 IPC 权限（build.rs 为每个命令生成 allow-<command>
// 权限）。capability 默认 local=true 且 remote 指向前端 origin，dev 与生产
// 上下文都由此单一来源授权，不再保留静态 capabilities 文件。
fn register_loopback_capability(app: &tauri::App, frontend_url: &Url) -> tauri::Result<()> {
    let mut capability = tauri::ipc::CapabilityBuilder::new("loopback-frontend")
        .remote(frontend_url.to_string())
        .window(MAIN_WINDOW_LABEL)
        .permission("core:default")
        .permission("opener:allow-open-url")
        .permission("opener:allow-default-urls")
        .permission("dialog:default");
    for command in crate::ipc_commands::IPC_COMMANDS {
        capability = capability.permission(format!("allow-{}", command.replace('_', "-")));
    }
    app.add_capability(capability)
}
