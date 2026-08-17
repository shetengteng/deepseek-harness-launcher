use std::sync::Mutex;

use tokio::sync::watch;

use crate::error::{LauncherError, Result};

#[derive(Default)]
pub struct DshInstallOperations {
    active: Mutex<Option<ActiveOperation>>,
}

struct ActiveOperation {
    id: String,
    cancelled: watch::Sender<()>,
}

impl DshInstallOperations {
    pub fn register(&self, operation_id: &str) -> Result<ActiveDshInstall<'_>> {
        if operation_id.trim().is_empty() {
            return Err(LauncherError::DshInstall(
                "operation_id is required to install dsh".to_string(),
            ));
        }

        let mut active = self
            .active
            .lock()
            .expect("dsh install operations mutex poisoned");
        if active.is_some() {
            return Err(LauncherError::DshInstall(
                "another dsh installation is already running".to_string(),
            ));
        }

        let (cancelled, receiver) = watch::channel(());
        *active = Some(ActiveOperation {
            id: operation_id.to_string(),
            cancelled,
        });
        Ok(ActiveDshInstall {
            operations: self,
            operation_id: operation_id.to_string(),
            cancellation: DshInstallCancellation { receiver },
        })
    }

    pub fn cancel(&self, operation_id: &str) -> bool {
        let active = self
            .active
            .lock()
            .expect("dsh install operations mutex poisoned");
        let Some(active) = active.as_ref().filter(|active| active.id == operation_id) else {
            return false;
        };
        active.cancelled.send_replace(());
        true
    }
}

pub struct ActiveDshInstall<'a> {
    operations: &'a DshInstallOperations,
    operation_id: String,
    cancellation: DshInstallCancellation,
}

impl ActiveDshInstall<'_> {
    pub fn cancellation(&self) -> DshInstallCancellation {
        self.cancellation.clone()
    }
}

impl Drop for ActiveDshInstall<'_> {
    fn drop(&mut self) {
        let mut active = self
            .operations
            .active
            .lock()
            .expect("dsh install operations mutex poisoned");
        if active
            .as_ref()
            .is_some_and(|active| active.id == self.operation_id)
        {
            *active = None;
        }
    }
}

#[derive(Clone)]
pub struct DshInstallCancellation {
    receiver: watch::Receiver<()>,
}

impl DshInstallCancellation {
    pub fn check(&self) -> Result<()> {
        if self.receiver.has_changed().unwrap_or(true) {
            return Err(self.error());
        }
        Ok(())
    }

    pub async fn cancelled(&self) {
        let mut receiver = self.receiver.clone();
        if receiver.has_changed().unwrap_or(true) {
            return;
        }
        let _ = receiver.changed().await;
    }

    pub fn error(&self) -> LauncherError {
        LauncherError::DshInstall("dsh installation was cancelled".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_matches_the_active_installation() {
        let operations = DshInstallOperations::default();
        let active = operations.register("install-1").unwrap();

        assert!(!operations.cancel("other-install"));
        assert!(operations.cancel("install-1"));
        assert!(active.cancellation().check().is_err());
    }

    #[test]
    fn dropping_active_installation_unregisters_it() {
        let operations = DshInstallOperations::default();
        let active = operations.register("install-1").unwrap();

        drop(active);
        assert!(!operations.cancel("install-1"));
    }
}
