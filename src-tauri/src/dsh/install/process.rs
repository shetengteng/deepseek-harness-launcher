use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::error::{LauncherError, Result};

use super::{
    default_log, verify_install, write_package_json, DshInstallCancellation, InstallDshOptions,
};

pub async fn run_npm_install(opts: &InstallDshOptions) -> Result<String> {
    run_npm_install_cancellable(opts, None).await
}

async fn run_npm_install_cancellable(
    opts: &InstallDshOptions,
    cancellation: Option<&DshInstallCancellation>,
) -> Result<String> {
    let cwd = opts.version_dir();
    if !opts.package_json_path().exists() {
        return Err(LauncherError::DshInstall(format!(
            "package.json not found at {} (call write_package_json first)",
            opts.package_json_path().display()
        )));
    }
    if let Some(cancellation) = cancellation {
        cancellation.check()?;
    }

    let npm = opts.npm_script.as_ref().ok_or_else(|| {
        LauncherError::DshInstall(
            "managed npm is required; launcher does not use system npm".to_string(),
        )
    })?;
    let mut cmd = Command::new(&opts.node_executable);
    cmd.arg(npm);

    cmd.current_dir(&cwd)
        .args([
            "install",
            "--prod",
            "--registry",
            &opts.registry,
            "--no-audit",
            "--no-fund",
            "--loglevel",
            "info",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    apply_npm_install_env(&mut cmd, opts)?;

    let log = opts.log.clone().unwrap_or_else(default_log);
    let mut child = cmd.spawn().map_err(|error| {
        LauncherError::DshInstall(format!(
            "spawn npm install failed: {error} (node={}, npm={:?})",
            opts.node_executable.display(),
            opts.npm_script
        ))
    })?;
    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");

    let stdout_task = stream_install_output(stdout, log.clone());
    let stderr_task = stream_install_output(stderr, log);
    let status = wait_for_install(&mut child, opts.timeout_secs, cancellation).await;
    let _ = stdout_task.await;
    let _ = stderr_task.await;
    let status = status?;

    if !status.success() {
        return Err(LauncherError::DshInstall(format!(
            "npm install failed: exit code {:?} (see logs for details)",
            status.code()
        )));
    }
    Ok(String::new())
}

fn apply_npm_install_env(cmd: &mut Command, opts: &InstallDshOptions) -> Result<()> {
    cmd.env_clear();
    cmd.env("PATH", path_with_managed_node(&opts.node_executable)?);
    for key in ["HOME", "USERPROFILE"] {
        if let Some(value) = std::env::var_os(key) {
            cmd.env(key, value);
        }
    }
    #[cfg(windows)]
    apply_windows_process_env(cmd);
    cmd.env("npm_config_scripts_prepend_node_path", "true");
    if let Some(cache) = &opts.npm_cache {
        std::fs::create_dir_all(cache).map_err(LauncherError::Io)?;
        cmd.env("npm_config_cache", cache);
    }
    if let Some(userconfig) = &opts.npm_userconfig {
        cmd.env("npm_config_userconfig", userconfig);
        cmd.env(
            "npm_config_globalconfig",
            userconfig.with_file_name("npmrc-global"),
        );
    }
    Ok(())
}

#[cfg(windows)]
fn apply_windows_process_env(cmd: &mut Command) {
    const KEYS: &[&str] = &[
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "PATHEXT",
        "TEMP",
        "TMP",
        "HOMEDRIVE",
        "HOMEPATH",
        "APPDATA",
        "LOCALAPPDATA",
        "SYSTEMDRIVE",
    ];
    for (key, value) in std::env::vars_os() {
        let Some(name) = key.to_str() else {
            continue;
        };
        if KEYS.iter().any(|wanted| name.eq_ignore_ascii_case(wanted)) {
            cmd.env(key, value);
        }
    }
}

/// 只给这次 npm 子进程用：托管 Node 目录 + 系统工具链。
/// 不继承用户 PATH，避免 nvm / Homebrew 的 node、npm 混进来。
pub(super) fn path_with_managed_node(node_executable: &Path) -> Result<std::ffi::OsString> {
    let mut entries = Vec::new();
    if let Some(dir) = node_executable.parent() {
        if !dir.as_os_str().is_empty() {
            entries.push(dir.to_path_buf());
        }
    }
    entries.extend(os_toolchain_dirs());
    std::env::join_paths(entries).map_err(|error| {
        LauncherError::DshInstall(format!("join PATH for npm install failed: {error}"))
    })
}

fn os_toolchain_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut dirs = Vec::new();
        if let Some(root) =
            std::env::var_os("SystemRoot").or_else(|| std::env::var_os("SYSTEMROOT"))
        {
            let root = PathBuf::from(root);
            dirs.push(root.join("System32"));
            dirs.push(root);
        }
        dirs
    }
    #[cfg(not(windows))]
    {
        vec![
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/usr/sbin"),
            PathBuf::from("/sbin"),
        ]
    }
}

fn stream_install_output(
    stream: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    log: super::LogCallback,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stream).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            log(&line);
        }
    })
}

async fn wait_for_install(
    child: &mut tokio::process::Child,
    timeout_secs: u64,
    cancellation: Option<&DshInstallCancellation>,
) -> Result<std::process::ExitStatus> {
    if let Some(cancellation) = cancellation {
        return wait_for_install_cancellable(child, timeout_secs, cancellation).await;
    }

    if timeout_secs == 0 {
        return child.wait().await.map_err(|error| {
            LauncherError::DshInstall(format!("npm install wait failed: {error}"))
        });
    }

    match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), child.wait()).await {
        Ok(status) => status.map_err(|error| {
            LauncherError::DshInstall(format!("npm install wait failed: {error}"))
        }),
        Err(_) => {
            let _ = child.kill().await;
            Err(LauncherError::DshInstall(format!(
                "npm install timed out after {timeout_secs}s"
            )))
        }
    }
}

pub async fn install_dsh(opts: &InstallDshOptions) -> Result<()> {
    install_dsh_cancellable(opts, None).await
}

pub async fn install_dsh_cancellable(
    opts: &InstallDshOptions,
    cancellation: Option<&DshInstallCancellation>,
) -> Result<()> {
    let version_dir = opts.version_dir();
    write_package_json(opts)?;

    let install_result = run_npm_install_cancellable(opts, cancellation).await;
    let verify_result = if install_result.is_ok() {
        verify_install(opts)
    } else {
        Ok(())
    };

    match (&install_result, &verify_result) {
        (Ok(_), Ok(_)) => {
            tracing::info!(version = %opts.version, "dsh installed successfully");
            Ok(())
        }
        (Err(error), _) => {
            let _ = std::fs::remove_dir_all(&version_dir);
            Err(LauncherError::DshInstall(format!(
                "install failed: {error} (version dir cleaned: {})",
                version_dir.display()
            )))
        }
        (Ok(_), Err(error)) => {
            let _ = std::fs::remove_dir_all(&version_dir);
            Err(LauncherError::DshInstall(format!(
                "verify failed: {error} (version dir cleaned: {})",
                version_dir.display()
            )))
        }
    }
}

async fn wait_for_install_cancellable(
    child: &mut tokio::process::Child,
    timeout_secs: u64,
    cancellation: &DshInstallCancellation,
) -> Result<std::process::ExitStatus> {
    enum WaitOutcome {
        Finished(std::io::Result<std::process::ExitStatus>),
        Cancelled,
        TimedOut,
    }

    let outcome = if timeout_secs == 0 {
        tokio::select! {
            status = child.wait() => WaitOutcome::Finished(status),
            _ = cancellation.cancelled() => WaitOutcome::Cancelled,
        }
    } else {
        tokio::select! {
            status = child.wait() => WaitOutcome::Finished(status),
            _ = tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)) => WaitOutcome::TimedOut,
            _ = cancellation.cancelled() => WaitOutcome::Cancelled,
        }
    };

    match outcome {
        WaitOutcome::Finished(status) => status.map_err(|error| {
            LauncherError::DshInstall(format!("npm install wait failed: {error}"))
        }),
        WaitOutcome::Cancelled => cancel_install(child, cancellation).await,
        WaitOutcome::TimedOut => {
            let _ = child.kill().await;
            Err(LauncherError::DshInstall(format!(
                "npm install timed out after {timeout_secs}s"
            )))
        }
    }
}

async fn cancel_install(
    child: &mut tokio::process::Child,
    cancellation: &DshInstallCancellation,
) -> Result<std::process::ExitStatus> {
    let _ = child.kill().await;
    let _ = child.wait().await;
    Err(cancellation.error())
}
