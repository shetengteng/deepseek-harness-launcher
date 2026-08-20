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

    let mut cmd = if let Some(npm) = &opts.npm_script {
        let mut command = Command::new(&opts.node_executable);
        command.arg(npm);
        command
    } else {
        Command::new("npm")
    };

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

    cmd.env_clear();
    cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
    if let Ok(home) = std::env::var("HOME") {
        cmd.env("HOME", home);
    } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
        cmd.env("USERPROFILE", user_profile);
    }

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
