//! dsh 版本切换、回滚与版本目录维护。

#[path = "version/inventory.rs"]
mod inventory;
#[path = "version/pointers.rs"]
mod pointers;
#[path = "version/state.rs"]
mod state;

pub use inventory::{is_version_installed, list_installed_versions, prune_old_versions};
pub use pointers::{
    read_current_pointer, read_known_good_pointer, write_current_pointer, write_known_good_pointer,
    CURRENT_POINTER_NAME, KNOWN_GOOD_POINTER_NAME,
};
pub use state::{promote_to_current, rollback_to_known_good, switch};

#[cfg(test)]
#[path = "version/tests.rs"]
mod tests;
