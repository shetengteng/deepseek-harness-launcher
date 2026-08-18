use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct InstallSpec {
    pub owner: String,
    pub repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subdirectory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl InstallSpec {
    pub fn source(&self) -> String {
        let mut source = format!("github:{}/{}", self.owner, self.repository);
        if let Some(subdirectory) = &self.subdirectory {
            source.push_str("#path:");
            source.push_str(subdirectory);
        }
        source
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Popularity {
    pub marketplace_rank: Option<u32>,
    pub ranking_updated_at: Option<String>,
    pub github_stars: Option<u64>,
    pub stars_fetched_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct CatalogPlugin {
    pub id: String,
    pub name: String,
    pub repository_url: String,
    pub install_spec: InstallSpec,
    pub description: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub source_updated_at: Option<String>,
    pub validated_at: Option<String>,
    pub popularity: Popularity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct InventoryPlugin {
    pub installation_id: String,
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginStatus {
    Available,
    Installed,
    UpdateAvailable,
    Unknown,
    OperationRunning,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MarketplacePlugin {
    pub id: String,
    pub name: String,
    pub repository_url: Option<String>,
    pub install_spec: Option<InstallSpec>,
    pub description: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub source_updated_at: Option<String>,
    pub validated_at: Option<String>,
    pub popularity: Popularity,
    pub status: PluginStatus,
    pub installation_id: Option<String>,
    pub installed_source: Option<String>,
    pub local_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MarketplaceSource {
    pub label: String,
    pub fetched_at: Option<String>,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MarketplaceSnapshot {
    pub source: MarketplaceSource,
    pub plugins: Vec<MarketplacePlugin>,
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceQuery {
    pub query: Option<String>,
    pub category: Option<String>,
    pub installed_only: Option<bool>,
    #[serde(default = "default_sort")]
    pub sort: MarketplaceSort,
    pub profile: String,
}

fn default_sort() -> MarketplaceSort {
    MarketplaceSort::Popularity
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MarketplaceSort {
    Relevance,
    Updated,
    Popularity,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceInstallRequest {
    pub plugin_id: String,
    pub profile: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceRemoveRequest {
    pub installation_id: String,
    pub profile: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceCustomInstallRequest {
    pub command: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MarketplaceOperation {
    pub id: String,
    pub kind: MarketplaceOperationKind,
    pub plugin_id: String,
    pub profile: String,
    pub phase: MarketplaceOperationPhase,
    pub message: String,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceOperationKind {
    Install,
    CustomInstall,
    Remove,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceOperationPhase {
    Preparing,
    Running,
    Verifying,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct MarketplaceOperationEvent {
    pub operation: MarketplaceOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct CatalogCache {
    pub provider_id: String,
    pub etag: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub response_version: Option<String>,
    pub plugins: Vec<CatalogPlugin>,
}
