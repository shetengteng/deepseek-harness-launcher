//! Node 运行时托管（设计 §M2）。
//!
//! 模块组织：
//!  - `mirror`：内置镜像源表 + 探活（设计 §M2.1）
//!  - `download`：流式下载 + SHA 校验 + 进度事件（设计 §M2.2）
//!  - `install`：跨平台解压 + 原子切换（设计 §M2.3）
//!  - `version`：版本解析 + engines 校验（设计 §M2.4）

pub mod disk;
pub mod download;
pub mod install;
pub mod mirror;
pub mod version;

pub use download::{
    download_archive, download_with_retry, fetch_shasums, find_sha_in_shasums, parse_shasums_line,
    verify_sha256, ProgressEvent, PROGRESS_CHUNK_BYTES,
};
pub use install::{
    archive_kind_for_current_platform, current_node_dir, current_node_dir_in, current_node_version,
    current_node_version_in, install_node, install_node_to, node_bin_path, node_npm_path,
    NodeArchiveKind,
};
pub use mirror::{
    pick_default_mirror, probe_mirror, probe_mirrors, validate_custom_mirror, Mirror, MirrorError,
    MirrorId, BUILTIN_MIRRORS, DEFAULT_NODE_INDEX_PATH,
};
pub use version::{
    current_node_satisfies, parse_node_version, satisfies_engines, DEFAULT_NODE_VERSION,
};
