mod fields;
mod internal;
mod normalize;
mod source;

use reqwest::{Client, RequestBuilder, StatusCode};

use crate::error::{LauncherError, Result};

use super::types::CatalogPlugin;
use normalize::normalize_registry_snapshot;

pub const PROVIDER_ID: &str = "awesome-dsh-registry-v1";
pub const PROVIDER_LABEL: &str = "awesome-dsh-plugin 社区目录";
pub const PROVIDER_URL: &str = "https://awesome-dsh-plugin.com/plugins.json";
const MAX_REGISTRY_BYTES: usize = 8 * 1024 * 1024;

pub enum ProviderResponse {
    NotModified,
    Snapshot(ProviderSnapshot),
}

pub struct ProviderSnapshot {
    pub etag: Option<String>,
    pub response_version: Option<String>,
    pub catalog_updated_at: Option<String>,
    pub catalog_count: Option<u32>,
    pub plugins: Vec<CatalogPlugin>,
}

pub struct RegistryProvider {
    client: Client,
}

impl RegistryProvider {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|error| {
                LauncherError::Marketplace(format!("catalog client failed: {error}"))
            })?;
        Ok(Self { client })
    }

    pub async fn fetch(&self, etag: Option<&str>) -> Result<ProviderResponse> {
        self.send(self.client.get(PROVIDER_URL), etag).await
    }

    async fn send(
        &self,
        mut request: RequestBuilder,
        etag: Option<&str>,
    ) -> Result<ProviderResponse> {
        request = request
            .header("Accept", "application/json")
            .header("User-Agent", "deepseek-harness-launcher/0.1");
        if let Some(etag) = etag {
            request = request.header("If-None-Match", etag);
        }
        let response = request
            .send()
            .await
            .map_err(|error| LauncherError::Marketplace(format!("catalog unavailable: {error}")))?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(ProviderResponse::NotModified);
        }
        if !response.status().is_success() {
            return Err(LauncherError::Marketplace(format!(
                "catalog unavailable: registry returned {}",
                response.status()
            )));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_REGISTRY_BYTES as u64)
        {
            return Err(LauncherError::Marketplace(
                "catalog response is too large".to_string(),
            ));
        }
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.bytes().await.map_err(|error| {
            LauncherError::Marketplace(format!("catalog response could not be read: {error}"))
        })?;
        if body.len() > MAX_REGISTRY_BYTES {
            return Err(LauncherError::Marketplace(
                "catalog response is too large".to_string(),
            ));
        }
        let value = serde_json::from_slice::<serde_json::Value>(&body).map_err(|error| {
            LauncherError::Marketplace(format!("catalog response is not valid JSON: {error}"))
        })?;
        let (plugins, response_version, catalog_updated_at, catalog_count) =
            normalize_registry_snapshot(&value)?;
        Ok(ProviderResponse::Snapshot(ProviderSnapshot {
            etag,
            response_version,
            catalog_updated_at,
            catalog_count,
            plugins,
        }))
    }
}

#[cfg(test)]
mod tests;
