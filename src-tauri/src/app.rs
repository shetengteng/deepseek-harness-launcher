use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Manager, RunEvent, WindowEvent};

use crate::{commands, logging, node::NodeDownloadOperations, paths, platform, tray};

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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::SharedState::new())
        .manage(NodeDownloadOperations::default())
        .manage(ExitCoordinator {
            requested: AtomicBool::new(false),
        })
        .setup(|app| {
            let handle = app.handle().clone();
            app.state::<commands::SharedState>()
                .supervisor
                .set_exit_handler(std::sync::Arc::new(move |detail| {
                    commands::spawn_crash_recovery(handle.clone(), detail)
                }));
            tray::setup(app)?;
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
            commands::host::launcher_status,
            commands::host::start_host,
            commands::host::restart_host,
            commands::host::shutdown_host,
            commands::bootstrap::list_mirrors,
            commands::bootstrap::probe_mirrors_command,
            commands::bootstrap::validate_custom_mirror_command,
            commands::bootstrap::resolve_bootstrap_plan_command,
            commands::bootstrap::get_latest_dsh_version_command,
            commands::node_install::install_node_command,
            commands::node_install::cancel_node_install_command,
            commands::dsh_install::install_dsh_command,
            commands::settings::get_dsh_state,
            commands::settings::set_node_mirror_command,
            commands::settings::set_registry_command,
            commands::maintenance::rollback_dsh_command,
            commands::diagnostics::export_diagnostics,
            commands::maintenance::uninstall_managed_runtime,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::ExitRequested { api, .. } = event {
                if app.state::<ExitCoordinator>().requested() {
                    return;
                }
                api.prevent_exit();
                tray::request_exit(app.clone());
            }
        });
}
