use std::path::Path;

use chrono::Utc;

use crate::error::{LauncherError, Result};
use crate::state::{AppState, DshState, InstalledDsh};

use super::pointers::{write_current_pointer, write_known_good_pointer};

pub fn switch(dsh_dir: &Path, version: &str) -> Result<()> {
    ensure_version_dir(dsh_dir, version, "switch")?;
    write_current_pointer(dsh_dir, version)
}

pub fn promote_to_current(state: &mut AppState, dsh_dir: &Path, version: &str) -> Result<()> {
    ensure_version_dir(dsh_dir, version, "promote")?;
    let old_current = state.dsh.current.clone();

    if let Some(old) = &old_current {
        if old != version {
            write_known_good_pointer(dsh_dir, old)?;
        }
    }
    write_current_pointer(dsh_dir, version)?;

    state.dsh.known_good = old_current.filter(|old| old != version);
    state.dsh.current = Some(version.to_string());
    set_installed_status(&mut state.dsh, version, "verified");
    Ok(())
}

pub fn rollback_to_known_good(state: &mut AppState, dsh_dir: &Path) -> Result<String> {
    let known_good = state.dsh.known_good.clone().ok_or_else(|| {
        LauncherError::DshVersion("cannot rollback: no known_good version available".to_string())
    })?;
    ensure_version_dir(dsh_dir, &known_good, "rollback: known_good")?;
    let failed_version = state.dsh.current.clone();

    write_current_pointer(dsh_dir, &known_good)?;
    state.dsh.current = Some(known_good.clone());
    state.dsh.known_good = None;

    if let Some(failed) = failed_version {
        set_installed_status(&mut state.dsh, &failed, "broken");
    }
    set_installed_status(&mut state.dsh, &known_good, "verified");
    Ok(known_good)
}

fn ensure_version_dir(dsh_dir: &Path, version: &str, operation: &str) -> Result<()> {
    let version_dir = dsh_dir.join(version);
    if version_dir.exists() {
        return Ok(());
    }
    let message = match operation {
        "switch" => format!(
            "cannot switch: version dir not found: {}",
            version_dir.display()
        ),
        "promote" => format!(
            "cannot promote: version dir not found: {}",
            version_dir.display()
        ),
        _ => format!(
            "cannot rollback: known_good dir not found: {}",
            version_dir.display()
        ),
    };
    Err(LauncherError::DshVersion(message))
}

fn set_installed_status(dsh: &mut DshState, version: &str, status: &str) {
    if let Some(item) = dsh
        .installed
        .iter_mut()
        .find(|item| item.version == version)
    {
        item.status = status.to_string();
        return;
    }
    dsh.installed.push(InstalledDsh {
        version: version.to_string(),
        installed_at: Utc::now(),
        status: status.to_string(),
    });
}
