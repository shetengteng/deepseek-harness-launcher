#[cfg(target_os = "macos")]
const DOCK_ICON_PNG: &[u8] = include_bytes!("../icons/icon.png");

#[cfg(target_os = "macos")]
pub(crate) fn apply_transparent_dock_icon() -> Result<(), String> {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(mtm);
    let data = NSData::with_bytes(DOCK_ICON_PNG);
    let icon = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or_else(|| "failed to decode dock icon PNG".to_string())?;
    unsafe { app.setApplicationIconImage(Some(&icon)) };
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn ensure_self_signed_macos() -> Result<(), String> {
    use std::process::{Command, Stdio};

    let executable = std::env::current_exe().map_err(|error| format!("current_exe: {error}"))?;
    let probe = Command::new("codesign")
        .arg("-dvvv")
        .arg(&executable)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("failed to run codesign -dvvv: {error}"))?;
    if !String::from_utf8_lossy(&probe.stderr).contains("linker-signed") {
        return Ok(());
    }
    eprintln!("deepseek-harness-launcher: binary is linker-signed, re-signing for macOS AMFI compatibility");
    let sign = Command::new("codesign")
        .arg("--force")
        .arg("--sign")
        .arg("-")
        .arg(&executable)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("failed to run codesign --force: {error}"))?;
    if !sign.status.success() {
        return Err(format!(
            "codesign --force failed (exit {}): {}",
            sign.status,
            String::from_utf8_lossy(&sign.stderr).trim()
        ));
    }
    eprintln!("deepseek-harness-launcher: re-signed successfully, please restart the app for changes to take effect");
    eprintln!("deepseek-harness-launcher: AMFI diagnostic - testing spawn after self-sign...");
    match Command::new("/bin/echo")
        .arg("amfi-diagnostic-ok")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => eprintln!(
            "deepseek-harness-launcher: AMFI diagnostic - spawn /bin/echo SUCCESS: stdout={:?}",
            String::from_utf8_lossy(&output.stdout).trim()
        ),
        Err(error) => {
            eprintln!("deepseek-harness-launcher: AMFI diagnostic - spawn /bin/echo FAILED: {error} (kind={:?})", error.kind());
            eprintln!("deepseek-harness-launcher: AMFI requires restart - current process cannot spawn after self-sign");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    const DOCK_ICON_PNG: &[u8] = include_bytes!("../icons/icon.png");

    #[test]
    fn dock_icon_png_is_rgba() {
        assert_eq!(&DOCK_ICON_PNG[12..16], b"IHDR");
        assert_eq!(
            DOCK_ICON_PNG[25], 6,
            "icon.png must stay RGBA so the packaged Dock icon can keep a transparent canvas"
        );
    }
}
