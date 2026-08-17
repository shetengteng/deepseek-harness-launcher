use std::sync::Arc;

use tokio::sync::Mutex;

use crate::host::readiness::MAX_STARTUP_OUTPUT_CHARS;

pub(super) async fn append_output(buffer: &Arc<Mutex<String>>, chunk: &str) {
    let mut guard = buffer.lock().await;
    guard.push_str(chunk);
    let length = guard.len();
    if length <= MAX_STARTUP_OUTPUT_CHARS {
        return;
    }

    let mut boundary = length - MAX_STARTUP_OUTPUT_CHARS;
    while boundary < length && !guard.is_char_boundary(boundary) {
        boundary += 1;
    }
    guard.drain(..boundary);
}
