use tauri::{
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

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let title = MenuItem::with_id(
        app,
        "tray-title",
        format!("DeepSeek Harness Launcher · v{}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?;
    let status = MenuItem::with_id(app, TRAY_STATUS_ID, status_label(), false, None::<&str>)?;
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
    let status_for_events = status.clone();
    let icon = tauri::include_image!("./icons/tray-icon.png");
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(false)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            TRAY_SHOW_WINDOW_ID => show_main_window(app),
            TRAY_OPEN_DSH_IN_BROWSER_ID => open_dsh_in_browser(app.clone()),
            TRAY_RESTART_HOST_ID => restart_host_from_tray(app.clone(), status_for_events.clone()),
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
        Ok(state::StateStatus::Loaded(state)) => state
            .dsh
            .current
            .map(|version| format!("DeepSeek Harness 已就绪 · {version}"))
            .unwrap_or_else(|| "DeepSeek Harness 尚未安装".to_string()),
        _ => "等待首次配置".to_string(),
    }
}
fn running_status_label() -> String {
    match state::AppState::load() {
        Ok(state::StateStatus::Loaded(state)) => state
            .dsh
            .current
            .map(|version| format!("DeepSeek Harness 运行中 · {version}"))
            .unwrap_or_else(|| "DeepSeek Harness 运行中".to_string()),
        _ => "DeepSeek Harness 运行中".to_string(),
    }
}
fn show_main_window(app: &AppHandle) {
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
fn restart_host_from_tray(app: AppHandle, status: MenuItem<tauri::Wry>) {
    if let Err(error) = status.set_text("DeepSeek Harness 正在重启…") {
        tracing::warn!(%error, "failed to update tray status");
    }
    tauri::async_runtime::spawn(async move {
        let state = app.state::<commands::SharedState>();
        state.navigation.clear_dsh_origin();
        let supervisor = state.supervisor.clone();
        match commands::restart_host_inner(&supervisor).await {
            Ok(origin) => {
                app.state::<commands::SharedState>()
                    .navigation
                    .activate_dsh_origin(&origin);
                if let Err(error) = status.set_text(running_status_label()) {
                    tracing::warn!(%error, "failed to update tray status");
                }
                if let Err(error) = app.emit("tray-host-restarted", origin) {
                    tracing::warn!(%error, "failed to emit tray restart event");
                }
            }
            Err(error) => {
                if let Err(menu_error) = status.set_text("DeepSeek Harness 重启失败") {
                    tracing::warn!(%menu_error, "failed to update tray status");
                }
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
    tauri::async_runtime::spawn(async move {
        let supervisor = app.state::<commands::SharedState>().supervisor.clone();
        let shutdown = supervisor.shutdown().await;
        shutdown.await_completion().await;
        app.exit(0);
    });
}
