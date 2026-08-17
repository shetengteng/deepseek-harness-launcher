use std::path::Path;

use crate::error::{LauncherError, Result};
use crate::host::SpawnDshWebOptions;

pub(crate) fn build_spawn_options() -> Result<SpawnDshWebOptions> {
    build_spawn_options_in(
        &crate::paths::node_runtime_dir()?,
        &crate::paths::dsh_dir()?,
    )
}

fn build_spawn_options_in(node_runtime_dir: &Path, dsh_dir: &Path) -> Result<SpawnDshWebOptions> {
    use crate::dsh::{read_current_pointer, DSH_ENTRY_REL};
    use crate::node::install::current_node_dir_in;

    let node_dir = current_node_dir_in(node_runtime_dir).map_err(|error| match error {
        LauncherError::NodeDownload(message) if message.contains("read VERSION file failed") => {
            LauncherError::NodeNotInstalled {
                reason: "node-runtime/VERSION not found; first-run wizard not completed"
                    .to_string(),
            }
        }
        other => other,
    })?;
    let node_executable = crate::node::install::node_bin_path(&node_dir);
    if !node_executable.is_file() {
        return Err(LauncherError::NodeNotInstalled {
            reason: format!(
                "managed Node executable not found: {}",
                node_executable.display()
            ),
        });
    }
    let current_version = read_current_pointer(dsh_dir)
        .map_err(|error| LauncherError::PathResolve {
            what: "dsh_current_pointer",
            cause: error.to_string(),
        })?
        .ok_or_else(|| LauncherError::DshNotInstalled {
            reason: "dsh/current pointer not set; first-run wizard not completed".to_string(),
        })?;
    let version_dir = dsh_dir.join(&current_version);
    let cli_entry = version_dir
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join(DSH_ENTRY_REL);
    if !cli_entry.exists() {
        return Err(LauncherError::DshNotInstalled {
            reason: format!(
                "dsh cli entry not found: {} (version {current_version} may be broken)",
                cli_entry.display()
            ),
        });
    }
    let mut env = crate::host::filtered_env();
    env.insert(
        "DSH_CLI_ENTRY".to_string(),
        cli_entry.to_string_lossy().into_owned(),
    );
    Ok(SpawnDshWebOptions {
        node_executable,
        cli_entry,
        cwd: version_dir,
        env,
        electron_run_as_node: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::LauncherError;

    use super::super::test_support::{write_dsh_entry, write_node_runtime};

    #[test]
    fn spawn_options_require_the_managed_node_binary() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("node-runtime");
        let dsh = temp.path().join("dsh");
        std::fs::create_dir_all(&runtime).unwrap();
        write_dsh_entry(&dsh, "0.2.0");
        crate::dsh::write_current_pointer(&dsh, "0.2.0").unwrap();

        let error = build_spawn_options_in(&runtime, &dsh).unwrap_err();

        assert!(matches!(error, LauncherError::NodeNotInstalled { .. }));
    }

    #[test]
    fn spawn_options_use_the_current_dsh_version_as_working_directory() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("node-runtime");
        let dsh = temp.path().join("dsh");
        std::fs::create_dir_all(&runtime).unwrap();
        write_node_runtime(&runtime, "22.19.0");
        write_dsh_entry(&dsh, "0.2.0");
        crate::dsh::write_current_pointer(&dsh, "0.2.0").unwrap();

        let options = build_spawn_options_in(&runtime, &dsh).unwrap();

        assert_eq!(options.cwd, dsh.join("0.2.0"));
        assert!(options
            .cli_entry
            .ends_with("node_modules/@deepseek-ai/dsh/lib/bin.js"));
    }
}
