use std::sync::Mutex;

use tokio::sync::watch;

use crate::error::{LauncherError, Result};

#[derive(Default)]
pub struct NodeDownloadOperations {
    active: Mutex<Option<ActiveOperation>>,
}

struct ActiveOperation {
    id: String,
    cancelled: watch::Sender<()>,
}

impl NodeDownloadOperations {
    pub fn register(&self, operation_id: &str) -> Result<ActiveDownload<'_>> {
        if operation_id.trim().is_empty() {
            return Err(LauncherError::NodeDownload(
                "operation_id is required to install Node".to_string(),
            ));
        }

        let mut active = self
            .active
            .lock()
            .expect("node download operations mutex poisoned");
        if active.is_some() {
            return Err(LauncherError::NodeDownload(
                "another Node archive download is already in progress".to_string(),
            ));
        }

        let (cancelled, receiver) = watch::channel(());
        *active = Some(ActiveOperation {
            id: operation_id.to_string(),
            cancelled,
        });
        Ok(ActiveDownload {
            operations: self,
            operation_id: operation_id.to_string(),
            receiver,
        })
    }

    pub fn cancel(&self, operation_id: &str) -> bool {
        let active = self
            .active
            .lock()
            .expect("node download operations mutex poisoned");
        let Some(active) = active.as_ref().filter(|active| active.id == operation_id) else {
            return false;
        };
        active.cancelled.send_replace(());
        true
    }

    #[cfg(test)]
    fn active_operation_id(&self) -> Option<String> {
        self.active
            .lock()
            .expect("node download operations mutex poisoned")
            .as_ref()
            .map(|active| active.id.clone())
    }
}

pub struct ActiveDownload<'a> {
    operations: &'a NodeDownloadOperations,
    operation_id: String,
    receiver: watch::Receiver<()>,
}

impl ActiveDownload<'_> {
    pub fn cancellation(&mut self) -> DownloadCancellation<'_> {
        DownloadCancellation {
            operation_id: &self.operation_id,
            receiver: &mut self.receiver,
        }
    }
}

impl Drop for ActiveDownload<'_> {
    fn drop(&mut self) {
        let mut active = self
            .operations
            .active
            .lock()
            .expect("node download operations mutex poisoned");
        if active
            .as_ref()
            .is_some_and(|active| active.id == self.operation_id)
        {
            *active = None;
        }
    }
}

pub struct DownloadCancellation<'a> {
    pub(crate) operation_id: &'a str,
    pub(crate) receiver: &'a mut watch::Receiver<()>,
}

impl DownloadCancellation<'_> {
    pub(crate) fn error(&self) -> LauncherError {
        LauncherError::NodeInstallCancelled {
            operation_id: self.operation_id.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_matches_only_the_active_operation() {
        let operations = NodeDownloadOperations::default();
        let active = operations.register("operation-1").unwrap();

        assert!(!operations.cancel("other-operation"));
        assert!(operations.cancel("operation-1"));
        assert!(active.receiver.has_changed().unwrap());
    }

    #[test]
    fn dropping_active_download_unregisters_the_operation() {
        let operations = NodeDownloadOperations::default();
        let active = operations.register("operation-1").unwrap();

        assert_eq!(
            operations.active_operation_id().as_deref(),
            Some("operation-1")
        );
        drop(active);
        assert_eq!(operations.active_operation_id(), None);
        assert!(!operations.cancel("operation-1"));
    }
}
