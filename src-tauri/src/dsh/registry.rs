//! npm registry 查询 + 缓存（设计 §M3.1 / PR-012）。
//!
//! 设计要求：
//! - `fetch_dist_tags(registry) -> DistTags`：`GET {registry}/@deepseek-ai/dsh`，解析 `dist-tags.latest`
//! - `fetch_package_manifest(registry, version) -> PackageManifest`：读 `engines.node`、`dist.integrity`、`dist.tarball`
//! - npm registry 列表：`registry.npmjs.org`、`registry.npmmirror.com`，允许用户自定义
//! - 缓存 5 分钟，避免短时多次查询
//! - 网络错误不污染缓存（只写成功响应）

use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::error::{LauncherError, Result};

/// npm 官方源。
pub const DEFAULT_REGISTRY_NPMJS: &str = "https://registry.npmjs.org";
/// 阿里源（state.json 默认值，对应 §M3.1）。
pub const DEFAULT_REGISTRY_NPMMIRROR: &str = "https://registry.npmmirror.com";

/// 缓存 TTL：5 分钟（设计 §M3.1）。
pub const REGISTRY_CACHE_TTL: Duration = Duration::from_secs(5 * 60);

/// dsh 包名（scope 写死，不允许用户改）。
pub const DSH_PACKAGE_NAME: &str = "@deepseek-ai/dsh";

/// `dist-tags` 对象。目前只关心 `latest`，其他 tag 解析后丢弃。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct DistTags {
    #[serde(rename = "latest")]
    pub latest: String,
    /// 其他 tag（如 `next`、`rc`）。解析失败时为空。
    #[serde(flatten)]
    pub others: serde_json::Map<String, serde_json::Value>,
}

/// 单个版本的 `dist` 对象。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DistInfo {
    /// 形如 `sha512-<base64>`，npm 标准 integrity 字段。
    #[serde(rename = "integrity")]
    pub integrity: String,
    /// 完整 tarball URL（npm 返回绝对地址）。
    #[serde(rename = "tarball")]
    pub tarball: String,
}

/// 单个版本的 manifest。从 registry 的 `versions[<version>]` 取出。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PackageManifest {
    pub version: String,
    /// `engines.node` 范围，例如 `>=22.0.0`。缺失视为无约束。
    #[serde(default)]
    pub engines: EnginesField,
    pub dist: DistInfo,
}

/// `engines` 对象。npm 允许缺失。
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct EnginesField {
    #[serde(default, rename = "node")]
    pub node: String,
    #[serde(flatten)]
    pub others: serde_json::Map<String, serde_json::Value>,
}

/// 完整的包元数据：`GET {registry}/@deepseek-ai/dsh` 的响应。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    #[serde(rename = "dist-tags")]
    pub dist_tags: DistTags,
    pub versions: std::collections::BTreeMap<String, serde_json::Value>,
}

impl PackageMetadata {
    /// 取指定版本的 manifest。版本不存在 → `Err(DshRegistry)`。
    pub fn manifest_for(&self, version: &str) -> Result<PackageManifest> {
        let raw = self.versions.get(version).ok_or_else(|| {
            LauncherError::DshRegistry(format!(
                "version {version} not found in {} (available: {})",
                self.name,
                self.versions.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        })?;
        let manifest: PackageManifest = serde_json::from_value(raw.clone())
            .map_err(|e| LauncherError::DshRegistry(e.to_string()))?;
        Ok(manifest)
    }
}

/// registry 查询缓存。Key = (registry, key)。Value = (data, fetched_at)。
///
/// 设计要求：5 分钟 TTL；网络错误不写入缓存。
/// 使用 `RwLock` 因为读多写少（每次查询先读 cache，命中直接返回）。
#[derive(Debug, Clone)]
pub struct RegistryCache {
    metadata: Arc<RwLock<std::collections::HashMap<String, (PackageMetadata, Instant)>>>,
}

impl Default for RegistryCache {
    fn default() -> Self {
        Self {
            metadata: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl RegistryCache {
    /// 创建空缓存。
    pub fn new() -> Self {
        Self::default()
    }

    /// 读 metadata 缓存。命中且未过期 → 返回 clone。
    pub async fn get_metadata(&self, registry: &str) -> Option<PackageMetadata> {
        let map = self.metadata.read().await;
        if let Some((data, fetched_at)) = map.get(registry) {
            if fetched_at.elapsed() < REGISTRY_CACHE_TTL {
                return Some(data.clone());
            }
        }
        None
    }

    /// 写入 metadata 缓存（仅在 HTTP 成功 + JSON 解析成功后调用）。
    pub async fn put_metadata(&self, registry: &str, data: PackageMetadata) {
        let mut map = self.metadata.write().await;
        map.insert(registry.to_string(), (data, Instant::now()));
    }

    /// 清空缓存。用于"立即检查更新"按钮强制刷新。
    pub async fn invalidate(&self) {
        let mut map = self.metadata.write().await;
        map.clear();
    }

    /// 当前缓存条目数（测试用）。
    pub async fn len(&self) -> usize {
        self.metadata.read().await.len()
    }

    /// 是否为空（测试用）。
    pub async fn is_empty(&self) -> bool {
        self.metadata.read().await.is_empty()
    }
}

/// 默认 HTTP client。10s 超时，rustls TLS，跟随重定向。
pub fn default_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("deepseek-harness-launcher/0.1 (registry)")
        .build()
        .expect("reqwest client build should not fail")
}

/// 拼接 registry 的包元数据 URL：`{registry}/@deepseek-ai/dsh`。
///
/// 注意：npm 期望 scope 名不带编码（`@` 在 path 中是合法的）。
pub fn metadata_url(registry: &str) -> String {
    let base = registry.trim_end_matches('/');
    format!("{base}/{DSH_PACKAGE_NAME}")
}

/// 拼接单版本 manifest URL：`{registry}/@deepseek-ai/dsh/{version}`。
pub fn manifest_url(registry: &str, version: &str) -> String {
    let base = registry.trim_end_matches('/');
    format!("{base}/{DSH_PACKAGE_NAME}/{version}")
}

/// 拉取完整包元数据，附带缓存。命中缓存时不发网络请求。
///
/// 设计 §M3.1：缓存 5 分钟；HTTP 失败 / JSON 解析失败 → `Err`，不写缓存。
pub async fn fetch_package_metadata(
    registry: &str,
    cache: &RegistryCache,
    client: &Client,
) -> Result<PackageMetadata> {
    // 1. 查缓存
    if let Some(cached) = cache.get_metadata(registry).await {
        tracing::debug!(registry, "registry cache hit");
        return Ok(cached);
    }

    // 2. 拉远端
    let data = fetch_package_metadata_with_client(registry, client).await?;

    // 3. 写缓存（只在成功后写）
    cache.put_metadata(registry, data.clone()).await;

    Ok(data)
}

/// 不带缓存的元数据拉取。供测试验证请求次数。
pub async fn fetch_package_metadata_with_client(
    registry: &str,
    client: &Client,
) -> Result<PackageMetadata> {
    let url = metadata_url(registry);
    tracing::debug!(url = %url, "fetching package metadata");

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| LauncherError::DshRegistry(format!("HTTP GET {url} failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(LauncherError::DshRegistry(format!(
            "registry returned {} for {url}",
            resp.status()
        )));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| LauncherError::DshRegistry(format!("read body failed: {e}")))?;

    let metadata: PackageMetadata = serde_json::from_str(&body)
        .map_err(|e| LauncherError::DshRegistry(format!("parse package metadata failed: {e}")))?;

    if metadata.name != DSH_PACKAGE_NAME {
        return Err(LauncherError::DshRegistry(format!(
            "registry returned wrong package name: expected {DSH_PACKAGE_NAME}, got {}",
            metadata.name
        )));
    }

    Ok(metadata)
}

/// 从 registry 取 `dist-tags`（缓存命中时不发网络）。
pub async fn fetch_dist_tags(
    registry: &str,
    cache: &RegistryCache,
    client: &Client,
) -> Result<DistTags> {
    let metadata = fetch_package_metadata(registry, cache, client).await?;
    Ok(metadata.dist_tags)
}

/// 取指定版本的 manifest（缓存命中时不发网络）。
pub async fn fetch_package_manifest(
    registry: &str,
    version: &str,
    cache: &RegistryCache,
    client: &Client,
) -> Result<PackageManifest> {
    let metadata = fetch_package_metadata(registry, cache, client).await?;
    metadata.manifest_for(version)
}

/// 不带缓存的 manifest 拉取。直接命中单版本端点，避免下载完整元数据。
///
/// 适用于：用户在 UI 上明确选了某个版本，不需要全量列表。
pub async fn fetch_package_manifest_with_client(
    registry: &str,
    version: &str,
    client: &Client,
) -> Result<PackageManifest> {
    let url = manifest_url(registry, version);
    tracing::debug!(url = %url, %version, "fetching single version manifest");

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| LauncherError::DshRegistry(format!("HTTP GET {url} failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(LauncherError::DshRegistry(format!(
            "registry returned {} for {url}",
            resp.status()
        )));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| LauncherError::DshRegistry(format!("read body failed: {e}")))?;

    let manifest: PackageManifest = serde_json::from_str(&body)
        .map_err(|e| LauncherError::DshRegistry(format!("parse manifest failed: {e}")))?;

    if manifest.version != version {
        return Err(LauncherError::DshRegistry(format!(
            "registry returned version {} but requested {version}",
            manifest.version
        )));
    }

    Ok(manifest)
}
