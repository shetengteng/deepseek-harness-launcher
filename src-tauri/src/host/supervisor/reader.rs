use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{ChildStderr, ChildStdout};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use super::output::append_output;
use super::LogCallback;
use crate::host::{Origin, ReadinessError, ReadinessParser};

pub(super) fn start_output_tasks(
    stdout: ChildStdout,
    stderr: ChildStderr,
    parser: ReadinessParser,
    output: Arc<Mutex<String>>,
    log: Option<LogCallback>,
) -> (
    oneshot::Receiver<Result<Origin, ReadinessError>>,
    JoinHandle<()>,
    JoinHandle<()>,
) {
    let (ready_tx, ready_rx) = oneshot::channel();
    let ready_tx = std::sync::Mutex::new(Some(ready_tx));
    let stdout_output = Arc::clone(&output);
    let stdout_log = log.clone();
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut ready_sent = false;
        while let Ok(Some(line)) = lines.next_line().await {
            let chunk = format!("{line}\n");
            append_output(&stdout_output, &chunk).await;
            if let Some(log) = stdout_log.as_ref() {
                log(&chunk);
            }
            if !ready_sent {
                match parser.push(&chunk).await {
                    Ok(Some(origin)) => send_ready(&ready_tx, Ok(origin)),
                    Err(error) => send_ready(&ready_tx, Err(error)),
                    Ok(None) => continue,
                }
                ready_sent = true;
            }
        }
        if !ready_sent {
            send_ready(&ready_tx, parser.finalize().await);
        }
    });

    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let chunk = format!("{line}\n");
            append_output(&output, &chunk).await;
            if let Some(log) = log.as_ref() {
                log(&chunk);
            }
        }
    });

    (ready_rx, stdout_task, stderr_task)
}

fn send_ready(
    sender: &std::sync::Mutex<Option<oneshot::Sender<Result<Origin, ReadinessError>>>>,
    result: Result<Origin, ReadinessError>,
) {
    if let Some(sender) = sender.lock().ok().and_then(|mut guard| guard.take()) {
        let _ = sender.send(result);
    }
}
