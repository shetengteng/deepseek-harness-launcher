use std::collections::HashMap;
use std::path::Path;

use crate::error::{LauncherError, Result};
use crate::host::SpawnDshWebOptions;

pub(crate) async fn build_spawn_options() -> Result<SpawnDshWebOptions> {
    build_spawn_options_in(
        &crate::paths::node_runtime_dir()?,
        &crate::paths::dsh_dir()?,
    )
    .await
}

async fn build_spawn_options_in(
    node_runtime_dir: &Path,
    dsh_dir: &Path,
) -> Result<SpawnDshWebOptions> {
    use crate::dsh::{read_current_pointer, DSH_ENTRY_REL};
    use crate::node::install::{current_node_dir_in, current_node_version_in, verify_node_binary};

    let node_dir = current_node_dir_in(node_runtime_dir).map_err(|error| match error {
        LauncherError::NodeDownload(message) if message.contains("read VERSION file failed") => {
            LauncherError::NodeNotInstalled {
                reason: "node-runtime/VERSION not found; first-run wizard not completed"
                    .to_string(),
            }
        }
        other => other,
    })?;
    let node_version = current_node_version_in(node_runtime_dir).map_err(|error| {
        LauncherError::NodeNotInstalled {
            reason: format!("managed Node VERSION is invalid: {error}"),
        }
    })?;
    verify_node_binary(&node_dir, &node_version)
        .await
        .map_err(|error| LauncherError::NodeNotInstalled {
            reason: format!("managed Node validation failed: {error}"),
        })?;
    let node_executable = crate::node::install::node_bin_path(&node_dir);
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
    if !cli_entry.is_file() {
        return Err(LauncherError::DshNotInstalled {
            reason: format!(
                "dsh cli entry not found: {} (version {current_version} may be broken)",
                cli_entry.display()
            ),
        });
    }
    let data_dir = dsh_dir.parent().ok_or_else(|| LauncherError::PathResolve {
        what: "data_dir",
        cause: format!("dsh directory has no parent: {}", dsh_dir.display()),
    })?;
    let managed_bin = crate::cli_shim::prepare_runtime_support(data_dir)?;
    let mut env = crate::host::filtered_env();
    prepend_path(&mut env, &managed_bin)?;
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

fn prepend_path(env: &mut HashMap<String, String>, directory: &Path) -> Result<()> {
    let mut entries = vec![directory.to_path_buf()];
    if let Some(existing) = take_path_entry(env) {
        entries.extend(std::env::split_paths(&existing));
    }
    let path = std::env::join_paths(entries).map_err(|error| LauncherError::PathResolve {
        what: "PATH",
        cause: error.to_string(),
    })?;
    env.insert("PATH".to_string(), path.to_string_lossy().into_owned());
    Ok(())
}

/// 取出并移除已有的 PATH 条目。
/// Windows 键名不区分大小写，环境块里的实际键可能是 `Path`，精确匹配会漏掉
/// 并在环境块里留下 `Path`/`PATH` 两个条目。
#[cfg(windows)]
fn take_path_entry(env: &mut HashMap<String, String>) -> Option<String> {
    let key = env.keys().find(|k| k.eq_ignore_ascii_case("PATH")).cloned();
    key.and_then(|k| env.remove(&k))
}

#[cfg(not(windows))]
fn take_path_entry(env: &mut HashMap<String, String>) -> Option<String> {
    env.remove("PATH")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::LauncherError;

    use super::super::test_support::write_dsh_entry;
    #[cfg(unix)]
    use super::super::test_support::write_node_runtime;

    #[tokio::test]
    async fn spawn_options_require_the_managed_node_binary() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("node-runtime");
        let dsh = temp.path().join("dsh");
        std::fs::create_dir_all(&runtime).unwrap();
        write_dsh_entry(&dsh, "0.2.0");
        crate::dsh::write_current_pointer(&dsh, "0.2.0").unwrap();

        let error = build_spawn_options_in(&runtime, &dsh).await.unwrap_err();

        assert!(matches!(error, LauncherError::NodeNotInstalled { .. }));
    }

    #[test]
    #[cfg(windows)]
    fn prepend_path_replaces_casing_variants() {
        let mut env = HashMap::new();
        env.insert("Path".to_string(), "C:\\Windows\\System32".to_string());
        prepend_path(&mut env, Path::new("C:\\managed\\bin")).unwrap();
        // 环境块里只允许一个 PATH 键，且 managed bin 在最前。
        assert_eq!(env.len(), 1);
        let path = env.get("PATH").expect("unified PATH key");
        assert!(path.starts_with("C:\\managed\\bin"));
        assert!(path.contains("C:\\Windows\\System32"));
    }

    #[test]
    fn prepend_path_prepends_without_existing_entry() {
        let dir = if cfg!(windows) {
            "C:\\managed\\bin"
        } else {
            "/managed/bin"
        };
        let mut env = HashMap::new();
        prepend_path(&mut env, Path::new(dir)).unwrap();
        let path = env.get("PATH").expect("PATH key");
        assert!(path.starts_with(dir));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_options_use_the_current_dsh_version_as_working_directory() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("node-runtime");
        let dsh = temp.path().join("dsh");
        std::fs::create_dir_all(&runtime).unwrap();
        write_node_runtime(&runtime, "22.19.0");
        write_dsh_entry(&dsh, "0.2.0");
        crate::dsh::write_current_pointer(&dsh, "0.2.0").unwrap();

        let options = build_spawn_options_in(&runtime, &dsh).await.unwrap();

        assert_eq!(options.cwd, dsh.join("0.2.0"));
        assert!(options
            .cli_entry
            .ends_with("node_modules/@deepseek-ai/dsh/lib/bin.js"));
        assert!(options
            .env
            .get("PATH")
            .is_some_and(|path| path.starts_with(&temp.path().join("bin").display().to_string())));
        assert!(temp.path().join("bin/pnpm").is_file());
    }
}
