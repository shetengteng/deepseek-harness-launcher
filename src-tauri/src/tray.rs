use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::{
    app::{ExitCoordinator, MAIN_WINDOW_LABEL},
    commands, state,
};

const TRAY_ID: &str = "launcher-tray";
const TRAY_STATUS_ID: &str = "tray-status";
const TRAY_SHOW_WINDOW_ID: &str = "show-main-window";
const TRAY_OPEN_DSH_IN_BROWSER_ID: &str = "open-dsh-in-browser";
const TRAY_RESTART_HOST_ID: &str = "restart-dsh";
const TRAY_CHECK_UPDATES_ID: &str = "check-latest-dsh-version";
const TRAY_OPEN_SETTINGS_ID: &str = "open-settings";
const TRAY_ABOUT_ID: &str = "about";
const TRAY_EXPORT_DIAGNOSTICS_ID: &str = "export-diagnostics";
const TRAY_QUIT_ID: &str = "quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostTrayStatus {
    Ready,
    Starting,
    Restarting,
    Running,
    Stopping,
    Stopped,
    Recovering,
    Crashed,
    Failed,
}

struct TrayStatusMenuItem {
    menu_item: MenuItem<tauri::Wry>,
}

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let title = MenuItem::with_id(
        app,
        "tray-title",
        format!("DeepSeek Harness Launcher · v{}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?;
    let status = MenuItem::with_id(app, TRAY_STATUS_ID, status_label(), false, None::<&str>)?;
    if !app.manage(TrayStatusMenuItem {
        menu_item: status.clone(),
    }) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "tray status menu item is already registered",
        )
        .into());
    }
    let show_window = MenuItem::with_id(
        app,
        TRAY_SHOW_WINDOW_ID,
        "显示主窗口",
        true,
        Some("CmdOrCtrl+1"),
    )?;
    let open_in_browser = MenuItem::with_id(
        app,
        TRAY_OPEN_DSH_IN_BROWSER_ID,
        "在浏览器中打开 DeepSeek Harness",
        true,
        None::<&str>,
    )?;
    let restart = MenuItem::with_id(
        app,
        TRAY_RESTART_HOST_ID,
        "重启 DeepSeek Harness",
        true,
        None::<&str>,
    )?;
    let check_updates = MenuItem::with_id(
        app,
        TRAY_CHECK_UPDATES_ID,
        "检查 DeepSeek Harness 更新",
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, TRAY_OPEN_SETTINGS_ID, "设置…", true, None::<&str>)?;
    let about = MenuItem::with_id(
        app,
        TRAY_ABOUT_ID,
        "关于 DeepSeek Harness Launcher",
        true,
        None::<&str>,
    )?;
    let diagnostics = MenuItem::with_id(
        app,
        TRAY_EXPORT_DIAGNOSTICS_ID,
        "导出诊断日志",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "退出", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(
        app,
        &[
            &title,
            &status,
            &PredefinedMenuItem::separator(app)?,
            &show_window,
            &open_in_browser,
            &restart,
            &check_updates,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &about,
            &diagnostics,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon())
        .icon_as_template(cfg!(target_os = "macos"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            TRAY_SHOW_WINDOW_ID => show_main_window(app),
            TRAY_OPEN_DSH_IN_BROWSER_ID => open_dsh_in_browser(app.clone()),
            TRAY_RESTART_HOST_ID => restart_host_from_tray(app.clone()),
            TRAY_CHECK_UPDATES_ID => emit_event(app, "tray-check-dsh-update"),
            TRAY_OPEN_SETTINGS_ID => emit_event(app, "tray-open-settings"),
            TRAY_ABOUT_ID => emit_event(app, "tray-open-about"),
            TRAY_EXPORT_DIAGNOSTICS_ID => emit_event(app, "tray-export-diagnostics"),
            TRAY_QUIT_ID => request_exit(app.clone()),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn open_dsh_in_browser(app: AppHandle) {
    let origin = app
        .state::<commands::SharedState>()
        .navigation
        .current_dsh_origin();
    let Some(origin) = origin else {
        tracing::warn!("cannot open DeepSeek Harness in browser before the host is ready");
        show_main_window(&app);
        return;
    };
    if let Err(error) = tauri_plugin_opener::open_url(origin, None::<&str>) {
        tracing::error!(%error, "failed to open DeepSeek Harness in browser");
    }
}

fn status_label() -> String {
    match state::AppState::load() {
        Ok(state::StateStatus::Loaded(state)) => {
            host_status_label(HostTrayStatus::Ready, state.dsh.current.as_deref())
        }
        _ => "等待首次配置".to_string(),
    }
}

fn selected_dsh_version() -> Option<String> {
    match state::AppState::load() {
        Ok(state::StateStatus::Loaded(state)) => state.dsh.current,
        _ => None,
    }
}

fn host_status_label(status: HostTrayStatus, version: Option<&str>) -> String {
    match status {
        HostTrayStatus::Ready => version
            .map(|version| format!("DeepSeek Harness 已就绪 · {version}"))
            .unwrap_or_else(|| "DeepSeek Harness 尚未安装".to_string()),
        HostTrayStatus::Starting => "DeepSeek Harness 正在启动…".to_string(),
        HostTrayStatus::Restarting => "DeepSeek Harness 正在重启…".to_string(),
        HostTrayStatus::Running => version
            .map(|version| format!("DeepSeek Harness 运行中 · {version}"))
            .unwrap_or_else(|| "DeepSeek Harness 运行中".to_string()),
        HostTrayStatus::Stopping => "DeepSeek Harness 正在停止…".to_string(),
        HostTrayStatus::Stopped => "DeepSeek Harness 已停止".to_string(),
        HostTrayStatus::Recovering => "DeepSeek Harness 已崩溃，正在恢复…".to_string(),
        HostTrayStatus::Crashed => "DeepSeek Harness 已崩溃，等待处理".to_string(),
        HostTrayStatus::Failed => "DeepSeek Harness 启动失败".to_string(),
    }
}

pub(crate) fn set_host_status(app: &AppHandle, status: HostTrayStatus) {
    let Some(menu) = app.try_state::<TrayStatusMenuItem>() else {
        tracing::warn!(?status, "tray status menu item is unavailable");
        return;
    };
    if let Err(error) = menu
        .menu_item
        .set_text(host_status_label(status, selected_dsh_version().as_deref()))
    {
        tracing::warn!(?status, %error, "failed to update tray status");
    }
}

#[cfg(all(test, target_os = "macos"))]
const TRAY_ICON_ASSET: &str = "./icons/tray-icon-macos.png";
#[cfg(all(test, target_os = "windows"))]
const TRAY_ICON_ASSET: &str = "./icons/tray-icon-windows.png";
#[cfg(all(test, target_os = "linux"))]
const TRAY_ICON_ASSET: &str = "./icons/tray-icon-linux.png";

#[cfg(target_os = "macos")]
fn tray_icon() -> Image<'static> {
    tauri::include_image!("./icons/tray-icon-macos.png")
}

#[cfg(target_os = "windows")]
fn tray_icon() -> Image<'static> {
    tauri::include_image!("./icons/tray-icon-windows.png")
}

#[cfg(target_os = "linux")]
fn tray_icon() -> Image<'static> {
    tauri::include_image!("./icons/tray-icon-linux.png")
}

pub(crate) fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        tracing::warn!("main window is unavailable");
        return;
    };
    if let Err(error) = window.unminimize() {
        tracing::warn!(%error, "failed to unminimize main window");
    }
    if let Err(error) = window.show() {
        tracing::warn!(%error, "failed to show main window");
    }
    if let Err(error) = window.set_focus() {
        tracing::warn!(%error, "failed to focus main window");
    }
}
fn emit_event(app: &AppHandle, event: &str) {
    if let Err(error) = app.emit(event, ()) {
        tracing::warn!(%event, %error, "failed to emit tray event");
    }
    show_main_window(app);
}
fn restart_host_from_tray(app: AppHandle) {
    set_host_status(&app, HostTrayStatus::Restarting);
    tauri::async_runtime::spawn(async move {
        let state = app.state::<commands::SharedState>();
        state.navigation.clear_dsh_origin();
        let supervisor = state.supervisor.clone();
        match commands::restart_host_inner(&supervisor).await {
            Ok(origin) => {
                app.state::<commands::SharedState>()
                    .navigation
                    .activate_dsh_origin(&origin);
                set_host_status(&app, HostTrayStatus::Running);
                if let Err(error) = app.emit("tray-host-restarted", origin) {
                    tracing::warn!(%error, "failed to emit tray restart event");
                }
            }
            Err(error) => {
                set_host_status(&app, HostTrayStatus::Failed);
                tracing::error!(%error, "failed to restart DeepSeek Harness from tray");
                let _ = app.emit("tray-host-restart-failed", error.to_string());
            }
        }
    });
}
pub(crate) fn request_exit(app: AppHandle) {
    if !app.state::<ExitCoordinator>().request() {
        return;
    }
    set_host_status(&app, HostTrayStatus::Stopping);
    tauri::async_runtime::spawn(async move {
        let supervisor = app.state::<commands::SharedState>().supervisor.clone();
        let shutdown = supervisor.shutdown().await;
        shutdown.await_completion().await;
        set_host_status(&app, HostTrayStatus::Stopped);
        app.exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_status_labels_cover_the_host_lifecycle() {
        assert_eq!(
            host_status_label(HostTrayStatus::Ready, Some("0.3.0")),
            "DeepSeek Harness 已就绪 · 0.3.0"
        );
        assert_eq!(
            host_status_label(HostTrayStatus::Starting, None),
            "DeepSeek Harness 正在启动…"
        );
        assert_eq!(
            host_status_label(HostTrayStatus::Running, Some("0.3.0")),
            "DeepSeek Harness 运行中 · 0.3.0"
        );
        assert_eq!(
            host_status_label(HostTrayStatus::Stopping, None),
            "DeepSeek Harness 正在停止…"
        );
        assert_eq!(
            host_status_label(HostTrayStatus::Crashed, None),
            "DeepSeek Harness 已崩溃，等待处理"
        );
        assert_eq!(
            host_status_label(HostTrayStatus::Failed, None),
            "DeepSeek Harness 启动失败"
        );
        assert_eq!(
            host_status_label(HostTrayStatus::Restarting, Some("0.3.0")),
            "DeepSeek Harness 正在重启…"
        );
        assert_eq!(
            host_status_label(HostTrayStatus::Stopped, Some("0.3.0")),
            "DeepSeek Harness 已停止"
        );
        assert_eq!(
            host_status_label(HostTrayStatus::Recovering, Some("0.3.0")),
            "DeepSeek Harness 已崩溃，正在恢复…"
        );
    }

    #[test]
    fn tray_status_transition_sequence_keeps_the_selected_version_context() {
        let version = Some("0.3.0");
        let sequence = [
            (HostTrayStatus::Starting, "DeepSeek Harness 正在启动…"),
            (HostTrayStatus::Running, "DeepSeek Harness 运行中 · 0.3.0"),
            (HostTrayStatus::Stopping, "DeepSeek Harness 正在停止…"),
            (HostTrayStatus::Stopped, "DeepSeek Harness 已停止"),
            (
                HostTrayStatus::Recovering,
                "DeepSeek Harness 已崩溃，正在恢复…",
            ),
            (HostTrayStatus::Crashed, "DeepSeek Harness 已崩溃，等待处理"),
            (HostTrayStatus::Failed, "DeepSeek Harness 启动失败"),
        ];
        for (status, expected) in sequence {
            assert_eq!(host_status_label(status, version), expected);
        }
    }

    #[test]
    fn tray_icon_asset_is_specific_to_the_current_platform() {
        #[cfg(target_os = "macos")]
        assert_eq!(TRAY_ICON_ASSET, "./icons/tray-icon-macos.png");
        #[cfg(target_os = "windows")]
        assert_eq!(TRAY_ICON_ASSET, "./icons/tray-icon-windows.png");
        #[cfg(target_os = "linux")]
        assert_eq!(TRAY_ICON_ASSET, "./icons/tray-icon-linux.png");
    }
}
