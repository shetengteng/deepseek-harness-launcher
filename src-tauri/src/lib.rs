//! deepseek-harness-launcher 壳子库根。
//!
//! 模块组织（对应设计 §3）：
//!  - `error`：LauncherError + Serialize，给 Tauri 命令返回统一错误结构。
//!  - `paths`：用户数据/日志/缓存目录解析（设计 §4.2）。
//!  - `state`：state.json 加载/原子写入（设计 §4.3）。
//!  - `logging`：tracing 初始化 + 滚动文件（设计 §11.2）。
//!  - `host`：Host 监管（readiness + supervisor + lifecycle，设计 §M1.3）。
//!  - `commands`：Tauri 命令层（设计 §M1.4）。
//!  - `node`：Node 运行时托管（设计 §M2）。

pub mod commands;
pub mod dsh;
pub mod error;
pub mod host;
pub mod logging;
pub mod node;
pub mod paths;
pub mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // macOS 26 (Tahoe) AMFI 兼容：cargo build 产出的二进制默认是 linker-signed
    // (flags 0x20002)，spawn 子进程会被 AMFI 拒绝并返回 EPERM。
    // 在做任何 spawn 之前，先检测并自我重新签名。
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = ensure_self_signed_macos() {
            eprintln!("warning: self-sign check failed: {e:?}");
            // 不 panic：可能 codesign 不可用或权限不足，后续 spawn 失败时再报错。
        }
    }

    // 初始化日志必须在 Tauri builder 之前，保证后续错误可被记录。
    // WorkerGuard 必须保活到进程退出，否则日志缓冲可能丢失：
    // `_guards` 持有 file_guard + console_guard，作用域到 `run()` 末尾。
    let _guards = match logging::init() {
        Ok(guards) => guards,
        Err(e) => {
            eprintln!("fatal: logging init failed: {e:?}");
            panic!("logging init failed: {e:?}");
        }
    };

    // 确保数据目录存在（首启时创建）。
    if let Err(e) = paths::ensure_dirs() {
        eprintln!("warning: ensure_dirs failed: {e:?}");
        // 不 panic：首启 UI 仍可展示，后续按需重试。
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // PR-019：诊断导出的保存对话框（前端 `@tauri-apps/plugin-dialog` 的 save()）。
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::SharedState::new())
        .setup(|app| {
            // PR-017：注入崩溃恢复回调。supervisor 检测到 Host 意外退出时触发，
            // 自动重启（未达上限）或 emit `host-crash-limit` 让前端弹窗（设计 §5.5）。
            use tauri::Manager;

            let handle = app.handle().clone();
            app.state::<commands::SharedState>()
                .supervisor
                .set_exit_handler(std::sync::Arc::new(move |detail| {
                    commands::spawn_crash_recovery(handle.clone(), detail);
                }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::launcher_status,
            commands::start_host,
            commands::restart_host,
            commands::shutdown_host,
            commands::list_mirrors,
            commands::probe_mirrors_command,
            commands::validate_custom_mirror_command,
            commands::install_node_command,
            commands::install_dsh_command,
            commands::get_dsh_state,
            commands::check_for_upgrade_command,
            commands::prepare_upgrade_command,
            commands::set_pinned_range_command,
            commands::set_auto_upgrade_command,
            commands::set_check_interval_command,
            commands::ignore_version_command,
            commands::unignore_version_command,
            commands::rollback_dsh_command,
            commands::export_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// macOS：检测当前二进制的签名状态，如果是 `linker-signed`（cargo build 默认产出），
/// 用 `codesign --force --sign -` 重新签名自身，将 flags 从 `0x20002` 改为 `0x2(adhoc)`。
///
/// macOS 26 (Tahoe) AMFI 拒绝 `linker-signed` 二进制 spawn 子进程，导致
/// `io error: Operation not permitted (os error 1)`。重新签名后 AMFI 放行。
///
/// 签名成功后不重启进程：AMFI 检查的是子进程的签名，父进程签名变化不影响当前进程的
/// spawn 能力。下一次启动时，二进制已是 adhoc 签名，跳过重新签名。
///
/// 失败处理：codesign 不可用或签名失败时返回 Err，调用方决定是否 panic。
#[cfg(target_os = "macos")]
fn ensure_self_signed_macos() -> Result<(), String> {
    use std::process::{Command, Stdio};

    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;

    // codesign -dvvv 输出到 stderr，退出码 0 表示已签名。
    let probe = Command::new("codesign")
        .arg("-dvvv")
        .arg(&exe)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run codesign -dvvv: {e}"))?;

    let stderr = String::from_utf8_lossy(&probe.stderr);
    // flags=0x20002(adhoc,linker-signed) 表示需要重新签名
    if !stderr.contains("linker-signed") {
        // 已是 adhoc 或正式签名，无需重新签名
        return Ok(());
    }

    eprintln!("deepseek-harness-launcher: binary is linker-signed, re-signing for macOS AMFI compatibility");

    let sign = Command::new("codesign")
        .arg("--force")
        .arg("--sign")
        .arg("-")
        .arg(&exe)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run codesign --force: {e}"))?;

    if !sign.status.success() {
        let err = String::from_utf8_lossy(&sign.stderr);
        return Err(format!(
            "codesign --force failed (exit {}): {}",
            sign.status,
            err.trim()
        ));
    }

    eprintln!("deepseek-harness-launcher: re-signed successfully, please restart the app for changes to take effect");

    // 诊断：自我签名后立即测试 spawn 能力，验证 AMFI 行为
    // 这段代码只在自我签名后执行，用于确认当前进程能否 spawn 子进程
    eprintln!("deepseek-harness-launcher: AMFI diagnostic - testing spawn after self-sign...");
    let test_spawn = Command::new("/bin/echo")
        .arg("amfi-diagnostic-ok")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match test_spawn {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            eprintln!(
                "deepseek-harness-launcher: AMFI diagnostic - spawn /bin/echo SUCCESS: stdout={:?}",
                stdout.trim()
            );
        }
        Err(e) => {
            eprintln!(
                "deepseek-harness-launcher: AMFI diagnostic - spawn /bin/echo FAILED: {} (kind={:?})",
                e,
                e.kind()
            );
            eprintln!("deepseek-harness-launcher: AMFI requires restart - current process cannot spawn after self-sign");
        }
    }

    Ok(())
}
