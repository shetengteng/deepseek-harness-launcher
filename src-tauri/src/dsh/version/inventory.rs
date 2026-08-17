use std::collections::HashSet;
use std::path::Path;

use crate::error::{LauncherError, Result};
use crate::state::AppState;

use super::pointers::{CURRENT_POINTER_NAME, KNOWN_GOOD_POINTER_NAME};

pub fn prune_old_versions(
    state: &AppState,
    dsh_dir: &Path,
    keep_extra: usize,
) -> Result<Vec<String>> {
    let keep = protected_versions(state);
    let extra: HashSet<String> = list_installed_versions(dsh_dir)?
        .into_iter()
        .filter(|version| !keep.contains(version))
        .rev()
        .take(keep_extra)
        .collect();

    if !dsh_dir.exists() {
        return Ok(Vec::new());
    }

    let mut pruned = Vec::new();
    for entry in std::fs::read_dir(dsh_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if is_pointer_name(&name) || keep.contains(&name) || extra.contains(&name) {
            continue;
        }
        std::fs::remove_dir_all(&path).map_err(|error| {
            LauncherError::DshVersion(format!(
                "failed to prune version {}: {error}",
                path.display()
            ))
        })?;
        pruned.push(name);
    }
    Ok(pruned)
}

pub fn list_installed_versions(dsh_dir: &Path) -> Result<Vec<String>> {
    if !dsh_dir.exists() {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();
    for entry in std::fs::read_dir(dsh_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_pointer_name(&name) {
            versions.push(name);
        }
    }
    versions.sort();
    Ok(versions)
}

pub fn is_version_installed(dsh_dir: &Path, version: &str) -> bool {
    dsh_dir.join(version).exists()
}

fn protected_versions(state: &AppState) -> HashSet<String> {
    [state.dsh.current.as_ref(), state.dsh.known_good.as_ref()]
        .into_iter()
        .flatten()
        .cloned()
        .collect()
}

fn is_pointer_name(name: &str) -> bool {
    name == CURRENT_POINTER_NAME
        || name == KNOWN_GOOD_POINTER_NAME
        || name == format!("{CURRENT_POINTER_NAME}.json")
        || name == format!("{KNOWN_GOOD_POINTER_NAME}.json")
}
