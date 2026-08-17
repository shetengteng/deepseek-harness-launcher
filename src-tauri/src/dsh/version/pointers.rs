use std::path::Path;

use crate::error::{LauncherError, Result};

pub const CURRENT_POINTER_NAME: &str = "current";
pub const KNOWN_GOOD_POINTER_NAME: &str = "known-good";

#[cfg(windows)]
const POINTER_JSON_SUFFIX: &str = ".json";

pub fn read_current_pointer(dsh_dir: &Path) -> Result<Option<String>> {
    read_pointer(dsh_dir, CURRENT_POINTER_NAME)
}

pub fn read_known_good_pointer(dsh_dir: &Path) -> Result<Option<String>> {
    read_pointer(dsh_dir, KNOWN_GOOD_POINTER_NAME)
}

pub fn write_current_pointer(dsh_dir: &Path, version: &str) -> Result<()> {
    write_pointer(dsh_dir, CURRENT_POINTER_NAME, version)
}

pub fn write_known_good_pointer(dsh_dir: &Path, version: &str) -> Result<()> {
    write_pointer(dsh_dir, KNOWN_GOOD_POINTER_NAME, version)
}

fn read_pointer(dsh_dir: &Path, name: &str) -> Result<Option<String>> {
    #[cfg(unix)]
    {
        match std::fs::read_link(dsh_dir.join(name)) {
            Ok(target) => Ok(target
                .file_name()
                .and_then(|value| value.to_str())
                .map(str::to_owned)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(LauncherError::Io(error)),
        }
    }

    #[cfg(windows)]
    {
        let path = dsh_dir.join(format!("{name}{POINTER_JSON_SUFFIX}"));
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let value: serde_json::Value = serde_json::from_str(&content)?;
                Ok(value
                    .get("version")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(LauncherError::Io(error)),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (dsh_dir, name);
        Err(LauncherError::DshVersion(
            "unsupported platform for pointer".to_string(),
        ))
    }
}

fn write_pointer(dsh_dir: &Path, name: &str, version: &str) -> Result<()> {
    std::fs::create_dir_all(dsh_dir)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let target = dsh_dir.join(name);
        let temporary = dsh_dir.join(format!(".{name}.tmp.{}", std::process::id()));
        let _ = std::fs::remove_file(&temporary);
        symlink(Path::new(version), &temporary)?;
        std::fs::rename(temporary, target)?;
        Ok(())
    }

    #[cfg(windows)]
    {
        let path = dsh_dir.join(format!("{name}{POINTER_JSON_SUFFIX}"));
        let temporary = dsh_dir.join(format!(".{name}{POINTER_JSON_SUFFIX}.tmp"));
        let content = serde_json::json!({
            "version": version,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });
        std::fs::write(&temporary, serde_json::to_vec_pretty(&content)?)?;
        std::fs::rename(temporary, path)?;
        Ok(())
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (name, version);
        Err(LauncherError::DshVersion(
            "unsupported platform for pointer".to_string(),
        ))
    }
}
