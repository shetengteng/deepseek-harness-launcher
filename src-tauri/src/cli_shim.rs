use std::path::{Path, PathBuf};

use directories::BaseDirs;

use crate::commands::DshCliInstallResult;
use crate::dsh::read_current_pointer;
use crate::error::{LauncherError, Result};
use crate::node::install::{current_node_dir_in, current_node_version_in, node_bin_path};

const SHIM_MARKER: &str = "Managed by DeepSeek Harness Launcher";
const HELPER_NAME: &str = "dsh-cli-shim.cjs";
const DSH_ENTRY_REL: &str = "node_modules/@deepseek-ai/dsh/lib/bin.js";

pub fn install_default() -> Result<DshCliInstallResult> {
    let home = BaseDirs::new()
        .ok_or_else(|| LauncherError::PathResolve {
            what: "home",
            cause: "BaseDirs::new returned None".to_string(),
        })?
        .home_dir()
        .to_path_buf();
    install_in(&crate::paths::data_dir()?, &home.join(".local").join("bin"))
}

fn install_in(data_dir: &Path, command_dir: &Path) -> Result<DshCliInstallResult> {
    verify_managed_runtime(data_dir)?;
    std::fs::create_dir_all(command_dir)?;

    let command_path = command_dir.join(command_name());
    ensure_owned_or_absent(&command_path)?;
    let helper_path = helper_path(data_dir);
    ensure_owned_or_absent(&helper_path)?;
    write_atomic(&helper_path, &helper_contents())?;
    write_atomic(&command_path, &command_contents(data_dir))?;

    Ok(DshCliInstallResult {
        command_path: command_path.to_string_lossy().into_owned(),
        path_instruction: path_instruction(command_dir),
    })
}

fn verify_managed_runtime(data_dir: &Path) -> Result<()> {
    let runtime_dir = data_dir.join("node-runtime");
    let node_version =
        current_node_version_in(&runtime_dir).map_err(|error| LauncherError::NodeNotInstalled {
            reason: error.to_string(),
        })?;
    let node_dir =
        current_node_dir_in(&runtime_dir).map_err(|error| LauncherError::NodeNotInstalled {
            reason: error.to_string(),
        })?;
    let node = node_bin_path(&node_dir);
    if !node.is_file() {
        return Err(LauncherError::NodeNotInstalled {
            reason: format!(
                "managed Node binary missing for {node_version}: {}",
                node.display()
            ),
        });
    }
    #[cfg(unix)]
    if !is_executable(&node)? {
        return Err(LauncherError::NodeNotInstalled {
            reason: format!("managed Node binary is not executable: {}", node.display()),
        });
    }

    let dsh_dir = data_dir.join("dsh");
    let version = read_current_pointer(&dsh_dir)
        .map_err(|error| LauncherError::DshNotInstalled {
            reason: error.to_string(),
        })?
        .ok_or_else(|| LauncherError::DshNotInstalled {
            reason: "dsh/current pointer not set".to_string(),
        })?;
    let entry = dsh_dir.join(version).join(DSH_ENTRY_REL);
    if !entry.is_file() {
        return Err(LauncherError::DshNotInstalled {
            reason: format!("dsh CLI entry missing: {}", entry.display()),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn command_name() -> &'static str {
    "dsh"
}

#[cfg(windows)]
fn command_name() -> &'static str {
    "dsh.cmd"
}

fn helper_path(data_dir: &Path) -> PathBuf {
    data_dir.join(HELPER_NAME)
}

fn ensure_owned_or_absent(path: &Path) -> Result<()> {
    match std::fs::read_to_string(path) {
        Ok(content) if content.contains(SHIM_MARKER) => Ok(()),
        Ok(_) => Err(LauncherError::DshCli(format!(
            "{} already exists and is not managed by this launcher",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(LauncherError::Io(error)),
    }
}

fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| LauncherError::DshCli(format!("invalid shim path: {}", path.display())))?;
    let temporary = path.with_file_name(format!(".{file_name}.tmp.{}", std::process::id()));
    std::fs::write(&temporary, contents)?;
    set_executable(&temporary)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    Ok(std::fs::metadata(path)?.permissions().mode() & 0o111 != 0)
}

#[cfg(windows)]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn command_contents(data_dir: &Path) -> String {
    let data_dir = posix_quote(data_dir);
    format!(
        "#!/bin/sh\n# {SHIM_MARKER}\nset -eu\n\nDATA_DIR={data_dir}\nVERSION_FILE=\"$DATA_DIR/node-runtime/VERSION\"\nif [ ! -f \"$VERSION_FILE\" ]; then\n  printf '%s\\n' 'dsh: managed Node.js is not installed. Open DeepSeek Harness Launcher and complete setup.' >&2\n  exit 127\nfi\nNODE_VERSION=$(tr -d '\\r\\n' < \"$VERSION_FILE\")\nNODE=\"$DATA_DIR/node-runtime/node-v$NODE_VERSION/bin/node\"\nif [ ! -x \"$NODE\" ]; then\n  printf '%s\\n' 'dsh: managed Node.js is unavailable. Reinstall it from DeepSeek Harness Launcher.' >&2\n  exit 127\nfi\nexec \"$NODE\" \"$DATA_DIR/{HELPER_NAME}\" \"$@\"\n"
    )
}

#[cfg(windows)]
fn command_contents(data_dir: &Path) -> String {
    let data_dir = cmd_escape(data_dir);
    format!(
        "@echo off\r\nREM {SHIM_MARKER}\r\nsetlocal DisableDelayedExpansion\r\nset \"DATA_DIR={data_dir}\"\r\nif not exist \"%DATA_DIR%\\node-runtime\\VERSION\" (\r\n  echo dsh: managed Node.js is not installed. Open DeepSeek Harness Launcher and complete setup. 1>&2\r\n  exit /b 127\r\n)\r\nset /p NODE_VERSION=<\"%DATA_DIR%\\node-runtime\\VERSION\"\r\nset \"NODE=%DATA_DIR%\\node-runtime\\node-v%NODE_VERSION%\\node.exe\"\r\nif not exist \"%NODE%\" (\r\n  echo dsh: managed Node.js is unavailable. Reinstall it from DeepSeek Harness Launcher. 1>&2\r\n  exit /b 127\r\n)\r\n\"%NODE%\" \"%DATA_DIR%\\{HELPER_NAME}\" %*\r\nexit /b %ERRORLEVEL%\r\n"
    )
}

fn helper_contents() -> String {
    format!(
        "// {SHIM_MARKER}\n'use strict';\n\nconst fs = require('node:fs');\nconst path = require('node:path');\nconst {{ spawnSync }} = require('node:child_process');\n\nconst dataDir = __dirname;\nconst fail = (message) => {{\n  console.error(`dsh: ${{message}}`);\n  process.exit(1);\n}};\n\nlet version;\ntry {{\n  if (process.platform === 'win32') {{\n    version = JSON.parse(fs.readFileSync(path.join(dataDir, 'dsh', 'current.json'), 'utf8')).version;\n  }} else {{\n    version = path.basename(fs.readlinkSync(path.join(dataDir, 'dsh', 'current')));\n  }}\n}} catch (error) {{\n  fail(`managed dsh is not installed (${{error.message}}). Open DeepSeek Harness Launcher and complete setup.`);\n}}\nif (typeof version !== 'string' || !version) {{\n  fail('managed dsh version pointer is invalid. Reinstall it from DeepSeek Harness Launcher.');\n}}\n\nconst entry = path.join(dataDir, 'dsh', version, '{DSH_ENTRY_REL}');\nif (!fs.existsSync(entry)) {{\n  fail('managed dsh is unavailable. Reinstall it from DeepSeek Harness Launcher.');\n}}\n\nconst child = spawnSync(process.execPath, ['--expose-internals', entry, ...process.argv.slice(2)], {{\n  stdio: 'inherit',\n}});\nif (child.error) fail(child.error.message);\nprocess.exit(child.status ?? 1);\n"
    )
}

#[cfg(unix)]
fn posix_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\"'\"'"))
}

#[cfg(windows)]
fn cmd_escape(path: &Path) -> String {
    path.to_string_lossy().replace('%', "%%")
}

#[cfg(unix)]
fn path_instruction(command_dir: &Path) -> String {
    format!(
        "在 ~/.zprofile 中加入 `export PATH=\"{}:$PATH\"`，然后关闭并重新打开 Terminal。",
        command_dir.display()
    )
}

#[cfg(windows)]
fn path_instruction(command_dir: &Path) -> String {
    format!(
        "将 {} 加入用户 PATH 后，关闭并重新打开终端。",
        command_dir.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn write_runtime(data_dir: &Path) {
        let runtime = data_dir.join("node-runtime");
        let node = runtime.join("node-v22.19.0").join(if cfg!(windows) {
            "node.exe"
        } else {
            "bin/node"
        });
        std::fs::create_dir_all(node.parent().expect("node parent")).expect("node directory");
        std::fs::write(runtime.join("VERSION"), "22.19.0").expect("version");
        std::fs::write(&node, "node").expect("node binary");
        #[cfg(unix)]
        set_executable(&node).expect("node executable");
        let dsh = data_dir.join("dsh").join("0.1.0").join(DSH_ENTRY_REL);
        std::fs::create_dir_all(dsh.parent().expect("entry parent")).expect("dsh directory");
        std::fs::write(dsh, "dsh").expect("entry");
        crate::dsh::write_current_pointer(&data_dir.join("dsh"), "0.1.0").expect("pointer");
    }

    #[test]
    fn installs_a_version_independent_command_shim() {
        let temp = tempfile::tempdir().expect("tempdir");
        let data = temp.path().join("data");
        let commands = temp.path().join("bin");
        write_runtime(&data);

        let result = install_in(&data, &commands).expect("install shim");
        let command = std::fs::read_to_string(&result.command_path).expect("command");
        let helper = std::fs::read_to_string(helper_path(&data)).expect("helper");

        assert!(command.contains(SHIM_MARKER));
        assert!(command.contains("node-runtime/VERSION"));
        assert!(helper.contains("current.json"));
        assert!(helper.contains("readlinkSync"));
        assert!(result
            .path_instruction
            .contains(&commands.display().to_string()));
        #[cfg(unix)]
        assert_ne!(
            std::fs::metadata(result.command_path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o111,
            0
        );
    }

    #[test]
    fn refuses_to_overwrite_an_unmanaged_dsh_command() {
        let temp = tempfile::tempdir().expect("tempdir");
        let data = temp.path().join("data");
        let commands = temp.path().join("bin");
        write_runtime(&data);
        std::fs::create_dir_all(&commands).expect("command directory");
        std::fs::write(commands.join(command_name()), "#!/bin/sh\necho other dsh\n")
            .expect("foreign command");

        let error = install_in(&data, &commands).expect_err("must not overwrite");

        assert!(
            matches!(error, LauncherError::DshCli(message) if message.contains("already exists"))
        );
    }

    #[test]
    fn requires_the_managed_runtime_before_writing_a_command() {
        let temp = tempfile::tempdir().expect("tempdir");
        let error =
            install_in(temp.path(), &temp.path().join("bin")).expect_err("runtime required");

        assert!(matches!(error, LauncherError::NodeNotInstalled { .. }));
    }

    #[test]
    fn quotes_unix_data_directories() {
        #[cfg(unix)]
        assert_eq!(
            posix_quote(Path::new("/tmp/with ' quote")),
            "'/tmp/with '\"'\"' quote'"
        );
    }

    #[cfg(unix)]
    #[test]
    fn shim_uses_the_current_node_version_and_forwards_arguments() {
        let temp = tempfile::tempdir().expect("tempdir");
        let data = temp.path().join("data");
        let commands = temp.path().join("bin");
        write_runtime(&data);
        let node = data.join("node-runtime/node-v22.20.0/bin/node");
        std::fs::create_dir_all(node.parent().expect("node parent")).expect("node directory");
        std::fs::write(&node, "#!/bin/sh\nprintf '%s\\n' node-v22.20.0 \"$@\"\n")
            .expect("node script");
        set_executable(&node).expect("node executable");
        std::fs::write(data.join("node-runtime/VERSION"), "22.20.0").expect("version");
        let result = install_in(&data, &commands).expect("install shim");

        let output = std::process::Command::new(result.command_path)
            .args(["plugin", "--profile", "web", "add", "example-plugin"])
            .output()
            .expect("run shim");

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).expect("stdout"),
            format!(
                "node-v22.20.0\n{}\nplugin\n--profile\nweb\nadd\nexample-plugin\n",
                helper_path(&data).display()
            )
        );
    }
}
