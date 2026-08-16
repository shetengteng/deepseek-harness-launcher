//! dsh 托管模块（设计 §M3）。
//!
//! - `registry`：npm registry 查询 + 缓存（设计 §M3.1）
//! - `integrity`：完整性校验（设计 §M3.2）
//! - `install`：npm install 封装（设计 §M3.2，PR-013）
//! - `version`：版本切换与回滚（设计 §M3.3，PR-014）
//! - `upgrade`：升级编排（设计 §M3.4，PR-015）

pub mod install;
pub mod integrity;
pub mod registry;
pub mod upgrade;
pub mod version;

pub use install::{
    cli_entry, default_log, install_dsh, options_from_manifest, read_cli_entry, run_npm_install,
    verify_install, write_package_json, InstallDshOptions, LogCallback, DSH_PACKAGE_NAME,
};
pub use integrity::{
    parse_integrity, sha512, verify_entry_exists, verify_installation, verify_tarball_integrity,
    DSH_ENTRY_REL,
};
pub use registry::{
    default_client, fetch_dist_tags, fetch_package_manifest, fetch_package_manifest_with_client,
    fetch_package_metadata, fetch_package_metadata_with_client, manifest_url, metadata_url,
    DistInfo, DistTags, EnginesField, PackageManifest, PackageMetadata, RegistryCache,
    DEFAULT_REGISTRY_NPMJS, DEFAULT_REGISTRY_NPMMIRROR,
    DSH_PACKAGE_NAME as REGISTRY_DSH_PACKAGE_NAME, REGISTRY_CACHE_TTL,
};
pub use upgrade::{
    check_for_upgrade, prepare_upgrade, resolve_node_target, NodeBlock, UpgradeCandidate,
    UpgradeCheck,
};
pub use version::{
    clear_pending, ignore_version, is_version_installed, list_installed_versions,
    promote_to_current, prune_old_versions, read_current_pointer, read_known_good_pointer,
    rollback_to_known_good, set_pending, switch, write_current_pointer, write_known_good_pointer,
    CURRENT_POINTER_NAME, KNOWN_GOOD_POINTER_NAME,
};
