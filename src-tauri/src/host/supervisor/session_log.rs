use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use super::LogCallback;

pub(super) fn start_session_log(configured: Option<LogCallback>) -> Option<LogCallback> {
    let file_log = match crate::paths::dsh_log_dir().and_then(|directory| {
        create_session_log(&directory, Utc::now()).map_err(crate::error::LauncherError::Io)
    }) {
        Ok((_, callback)) => Some(callback),
        Err(error) => {
            tracing::warn!(%error, "dsh output will not be persisted for this session");
            None
        }
    };
    combine_logs(configured, file_log)
}

fn create_session_log(
    directory: &Path,
    started_at: DateTime<Utc>,
) -> std::io::Result<(PathBuf, LogCallback)> {
    std::fs::create_dir_all(directory)?;
    let (path, file) = create_unique_session_file(directory, started_at)?;
    let file = Arc::new(Mutex::new(file));
    let callback = Arc::new(move |chunk: &str| write_chunk(&file, chunk));
    Ok((path, callback))
}

fn create_unique_session_file(
    directory: &Path,
    started_at: DateTime<Utc>,
) -> std::io::Result<(PathBuf, File)> {
    let timestamp = started_at.format("%Y%m%dT%H%M%S%.9fZ");
    for sequence in 0..=999 {
        let suffix = (sequence != 0).then(|| format!("-{sequence}"));
        let path = directory.join(format!("dsh-{timestamp}{}.log", suffix.unwrap_or_default()));
        match OpenOptions::new().create_new(true).write(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("could not reserve a unique dsh session log for {timestamp}"),
    ))
}

fn write_chunk(file: &Arc<Mutex<File>>, chunk: &str) {
    let Ok(mut file) = file.lock() else {
        return;
    };
    if let Err(error) = file.write_all(chunk.as_bytes()).and_then(|()| file.flush()) {
        tracing::warn!(%error, "failed to write dsh session log");
    }
}

fn combine_logs(first: Option<LogCallback>, second: Option<LogCallback>) -> Option<LogCallback> {
    match (first, second) {
        (Some(first), Some(second)) => Some(Arc::new(move |chunk| {
            first(chunk);
            second(chunk);
        })),
        (Some(log), None) | (None, Some(log)) => Some(log),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn creates_timestamped_session_log_and_persists_chunks() {
        let directory = tempfile::tempdir().unwrap();
        let started_at = Utc.with_ymd_and_hms(2026, 8, 17, 5, 4, 3).unwrap();
        let (path, log) = create_session_log(directory.path(), started_at).unwrap();

        log("stdout line\n");
        log("stderr line\n");

        assert_eq!(
            path.file_name().unwrap(),
            "dsh-20260817T050403.000000000Z.log"
        );
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "stdout line\nstderr line\n"
        );
    }

    #[test]
    fn combines_configured_and_persistent_logs() {
        let received = Arc::new(Mutex::new(String::new()));
        let received_copy = Arc::clone(&received);
        let configured: LogCallback =
            Arc::new(move |chunk| received_copy.lock().unwrap().push_str(chunk));
        let persistent = Arc::new(|_: &str| {});
        let combined = combine_logs(Some(configured), Some(persistent)).unwrap();

        combined("dsh output");

        assert_eq!(*received.lock().unwrap(), "dsh output");
    }

    #[test]
    fn creates_distinct_logs_for_sessions_with_the_same_timestamp() {
        let directory = tempfile::tempdir().unwrap();
        let started_at = Utc.with_ymd_and_hms(2026, 8, 17, 5, 4, 3).unwrap();

        let (first, _) = create_session_log(directory.path(), started_at).unwrap();
        let (second, _) = create_session_log(directory.path(), started_at).unwrap();

        assert_ne!(first, second);
        assert_eq!(
            second.file_name().unwrap(),
            "dsh-20260817T050403.000000000Z-1.log"
        );
    }

    #[test]
    fn write_failure_does_not_propagate_to_the_host() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("read-only.log");
        std::fs::write(&path, "existing output\n").unwrap();
        let file = Arc::new(Mutex::new(File::open(&path).unwrap()));

        write_chunk(&file, "new output\n");

        assert_eq!(std::fs::read_to_string(path).unwrap(), "existing output\n");
    }
}
