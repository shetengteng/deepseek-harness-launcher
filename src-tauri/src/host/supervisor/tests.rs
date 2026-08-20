use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
#[cfg(unix)]
use std::time::Duration;

use tokio::sync::Mutex;

use super::output::append_output;
use super::*;
use crate::host::readiness::MAX_STARTUP_OUTPUT_CHARS;
use crate::host::SpawnDshWebOptions;

#[cfg(unix)]
fn mock_host_options(temp: &tempfile::TempDir, mode: &str) -> SpawnDshWebOptions {
    let script = temp.path().join("mock-host.sh");
    write_executable(
        &script,
        "#!/bin/sh\ncase \"$DSH_TEST_MODE\" in\n  ready) printf 'dsh web: http://127.0.0.1:43123/\\n'; while true; do sleep 1; done ;;\n  exit) printf 'dsh web: http://127.0.0.1:43123/\\n'; exit 17 ;;\n  timeout) sleep 30 ;;\nesac\n",
    );
    SpawnDshWebOptions {
        node_executable: script,
        cli_entry: PathBuf::from("/tmp/mock-dsh-entry.js"),
        cwd: temp.path().to_path_buf(),
        env: HashMap::from([("DSH_TEST_MODE".to_string(), mode.to_string())]),
        electron_run_as_node: false,
    }
}

/// Close and rename before exec. Linux returns ETXTBSY if the inode is still open for write.
#[cfg(unix)]
fn write_executable(path: &std::path::Path, contents: &str) {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("script name");
    let temporary = path.with_file_name(format!(".{name}.tmp"));
    {
        let mut file = std::fs::File::create(&temporary).expect("create mock host");
        file.write_all(contents.as_bytes())
            .expect("write mock host");
        file.sync_all().expect("sync mock host");
    }
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755))
        .expect("chmod mock host");
    std::fs::rename(temporary, path).expect("install mock host");
}

#[tokio::test]
async fn shutdown_without_start_is_noop() {
    let supervisor = HostSupervisor::new(HostSupervisorConfig::default());
    supervisor.shutdown().await.await_completion().await;
    assert!(supervisor.shutdown_flag.load(Ordering::Acquire));
}

#[tokio::test]
async fn start_after_shutdown_errors() {
    let supervisor = Arc::new(HostSupervisor::new(HostSupervisorConfig::default()));
    supervisor.shutdown().await.await_completion().await;
    let options = SpawnDshWebOptions {
        node_executable: PathBuf::from("node"),
        cli_entry: PathBuf::from("/tmp/dsh/bin.js"),
        cwd: std::env::current_dir().unwrap(),
        env: HashMap::new(),
        electron_run_as_node: false,
    };
    assert!(matches!(
        supervisor.start(&options).await.unwrap_err(),
        HostSupervisorError::AlreadyShutdown
    ));
}

#[tokio::test]
async fn set_exit_handler_stores_callback() {
    let supervisor = HostSupervisor::new(HostSupervisorConfig::default());
    let called = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&called);
    supervisor.set_exit_handler(Arc::new(move |_| {
        flag.store(true, Ordering::Release);
    }));
    let callback = supervisor.exit_handler.read().unwrap().clone().unwrap();
    callback(HostExitDetail {
        code: Some(1),
        signal: None,
    });
    assert!(called.load(Ordering::Acquire));
}

#[tokio::test]
async fn append_output_truncates_head_when_over_limit() {
    let buffer = Arc::new(Mutex::new(String::new()));
    append_output(&buffer, &"x".repeat(MAX_STARTUP_OUTPUT_CHARS + 100)).await;
    assert_eq!(buffer.lock().await.len(), MAX_STARTUP_OUTPUT_CHARS);
}

#[tokio::test]
async fn append_output_respects_char_boundary() {
    let buffer = Arc::new(Mutex::new(String::new()));
    let input = "中".repeat(MAX_STARTUP_OUTPUT_CHARS / 3 + 100);
    append_output(&buffer, &input).await;
    let output = buffer.lock().await.clone();
    assert!(output.len() <= MAX_STARTUP_OUTPUT_CHARS + 3);
    assert!(output.is_char_boundary(output.len()));
}

#[tokio::test]
async fn each_startup_parser_accepts_its_own_random_port() {
    let first = new_startup_parser();
    first
        .push("dsh web: http://127.0.0.1:49808/\n")
        .await
        .expect("first startup readiness");

    let restarted = new_startup_parser();
    let origin = restarted
        .push("dsh web: http://127.0.0.1:52819/\n")
        .await
        .expect("restarted host readiness")
        .expect("restarted host origin");

    assert_eq!(origin.as_str(), "http://127.0.0.1:52819");
}

#[cfg(unix)]
#[tokio::test]
async fn mock_host_ready_then_shutdown_has_no_unexpected_exit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let supervisor = Arc::new(HostSupervisor::new(HostSupervisorConfig::default()));
    let called = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&called);
    supervisor.set_exit_handler(Arc::new(move |_| {
        flag.store(true, Ordering::Release);
    }));

    let origin = supervisor
        .start(&mock_host_options(&temp, "ready"))
        .await
        .expect("mock host ready");
    assert_eq!(origin.as_str(), "http://127.0.0.1:43123");
    supervisor.shutdown().await.await_completion().await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!called.load(Ordering::Acquire));
}

#[cfg(unix)]
#[tokio::test]
async fn mock_host_readiness_timeout_kills_child() {
    let temp = tempfile::tempdir().expect("tempdir");
    let config = HostSupervisorConfig {
        readiness_timeout: Duration::from_millis(40),
        ..HostSupervisorConfig::default()
    };
    let supervisor = Arc::new(HostSupervisor::new(config));
    let error = supervisor
        .start(&mock_host_options(&temp, "timeout"))
        .await
        .expect_err("mock host should time out");
    assert!(matches!(error, HostSupervisorError::ReadinessTimeout(_)));
}

#[cfg(unix)]
#[tokio::test]
async fn mock_host_unexpected_exit_invokes_handler() {
    let temp = tempfile::tempdir().expect("tempdir");
    let supervisor = Arc::new(HostSupervisor::new(HostSupervisorConfig::default()));
    let called = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&called);
    supervisor.set_exit_handler(Arc::new(move |detail| {
        assert_eq!(detail.code, Some(17));
        flag.store(true, Ordering::Release);
    }));
    supervisor
        .start(&mock_host_options(&temp, "exit"))
        .await
        .expect("mock host ready before exit");

    tokio::time::timeout(Duration::from_secs(1), async {
        while !called.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("unexpected exit callback");
}

#[cfg(unix)]
#[tokio::test]
async fn mock_host_restart_does_not_report_intentional_exit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let supervisor = Arc::new(HostSupervisor::new(HostSupervisorConfig::default()));
    let called = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&called);
    supervisor.set_exit_handler(Arc::new(move |_| {
        flag.store(true, Ordering::Release);
    }));
    let options = mock_host_options(&temp, "ready");
    supervisor.start(&options).await.expect("first start");
    let restarted = supervisor.restart(&options).await.expect("restart");
    assert_eq!(restarted.as_str(), "http://127.0.0.1:43123");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!called.load(Ordering::Acquire));
    supervisor.shutdown().await.await_completion().await;
}

#[cfg(unix)]
#[test]
fn write_executable_can_be_spawned_immediately() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("echo.sh");
    write_executable(&path, "#!/bin/sh\nprintf ok\n");
    let output = std::process::Command::new(&path).output().expect("spawn");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"ok");
}
