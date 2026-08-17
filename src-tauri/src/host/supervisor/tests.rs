use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;

use super::output::append_output;
use super::*;
use crate::host::readiness::MAX_STARTUP_OUTPUT_CHARS;
use crate::host::SpawnDshWebOptions;

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
