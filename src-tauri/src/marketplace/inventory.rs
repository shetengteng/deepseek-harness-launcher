use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde::Deserialize;

use crate::error::{LauncherError, Result};

use super::types::InventoryPlugin;

pub fn profiles_root() -> Result<PathBuf> {
    let home = std::env::var_os("DSH_HOME")
        .map(PathBuf::from)
        .or_else(|| BaseDirs::new().map(|dirs| dirs.home_dir().join(".dsh")))
        .ok_or_else(|| LauncherError::PathResolve {
            what: "dsh_home",
            cause: "DSH_HOME is unset and home directory is unavailable".to_string(),
        })?;
    Ok(home.join("profiles"))
}

pub fn list_profiles() -> Result<Vec<String>> {
    list_profiles_in(&profiles_root()?)
}

pub fn list_profiles_in(root: &Path) -> Result<Vec<String>> {
    let mut profiles = vec!["web".to_string()];
    match std::fs::read_dir(root) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if valid_profile_name(&name) && !profiles.contains(&name) {
                    profiles.push(name);
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(LauncherError::Io(error)),
    }
    profiles.sort();
    Ok(profiles)
}

pub fn read_profile(profile: &str) -> Result<Vec<InventoryPlugin>> {
    let profiles = list_profiles()?;
    validate_available_profile(profile, &profiles)?;
    read_profile_in(&profiles_root()?, profile)
}

pub fn read_profile_in(root: &Path, profile: &str) -> Result<Vec<InventoryPlugin>> {
    if !valid_profile_name(profile) {
        return Err(profile_error("profile name is invalid"));
    }
    let manifest_path = root.join(profile).join("package.json");
    let bytes = match std::fs::read(&manifest_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(LauncherError::Io(error)),
    };
    let manifest: ProfileManifest = serde_json::from_slice(&bytes)
        .map_err(|error| profile_error(&format!("profile manifest is invalid: {error}")))?;
    Ok(manifest
        .dependencies
        .into_iter()
        .map(|(name, source)| InventoryPlugin {
            installation_id: name.clone(),
            name,
            source,
        })
        .collect())
}

pub fn validate_available_profile(profile: &str, profiles: &[String]) -> Result<()> {
    (valid_profile_name(profile) && profiles.iter().any(|candidate| candidate == profile))
        .then_some(())
        .ok_or_else(|| profile_error("profile is unavailable"))
}

pub fn valid_profile_name(profile: &str) -> bool {
    !profile.is_empty()
        && profile.len() <= 64
        && profile
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && profile
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[derive(Deserialize)]
struct ProfileManifest {
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
}

fn profile_error(message: &str) -> LauncherError {
    LauncherError::Marketplace(format!("profile {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_actual_profile_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let profile = temp.path().join("web");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(profile.join("package.json"), r#"{"dependencies":{"dsh-plugin":"github:owner/repo"},"dsh":{"profile":{"bundles":["in-box"]}}}"#).unwrap();
        let inventory = read_profile_in(temp.path(), "web").unwrap();
        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].installation_id, "dsh-plugin");
    }

    #[test]
    fn profile_names_are_constrained() {
        assert!(valid_profile_name("web"));
        assert!(valid_profile_name("work-1"));
        assert!(!valid_profile_name("../web"));
        assert!(!valid_profile_name("web profile"));
    }
}
