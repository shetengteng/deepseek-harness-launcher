use std::collections::BTreeSet;

use reqwest::{Client, StatusCode};
use serde_json::Value;
use url::Url;

use crate::error::{LauncherError, Result};

use super::types::{CatalogPlugin, InstallSpec, Popularity};

pub const PROVIDER_ID: &str = "dsh-1024store";
pub const PROVIDER_LABEL: &str = "DSH 1024Store";
// The provider endpoint is a release-time setting so the application never
// accepts an arbitrary catalogue URL from the UI or persisted preferences.
const CATALOG_ENDPOINT: Option<&str> = option_env!("DSH1024_CATALOG_ENDPOINT");

pub enum ProviderResponse {
    NotModified,
    Snapshot(ProviderSnapshot),
}

pub struct ProviderSnapshot {
    pub etag: Option<String>,
    pub response_version: Option<String>,
    pub plugins: Vec<CatalogPlugin>,
}

pub struct Dsh1024Provider {
    client: Client,
    endpoint: Url,
}

impl Dsh1024Provider {
    pub fn new() -> Result<Self> {
        let endpoint = CATALOG_ENDPOINT.ok_or_else(|| {
            LauncherError::Marketplace(
                "catalog unavailable: the release has no configured provider endpoint".to_string(),
            )
        })?;
        Self::with_endpoint(endpoint)
    }

    pub fn with_endpoint(endpoint: &str) -> Result<Self> {
        let endpoint = Url::parse(endpoint).map_err(|error| {
            LauncherError::Marketplace(format!("catalog endpoint is invalid: {error}"))
        })?;
        if endpoint.scheme() != "https" || !is_allowed_host(endpoint.host_str()) {
            return Err(LauncherError::Marketplace(
                "catalog endpoint is outside the provider allowlist".to_string(),
            ));
        }
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|error| {
                LauncherError::Marketplace(format!("catalog client failed: {error}"))
            })?;
        Ok(Self { client, endpoint })
    }

    pub async fn fetch(&self, etag: Option<&str>) -> Result<ProviderResponse> {
        let mut request = self
            .client
            .get(self.endpoint.clone())
            .header("Accept", "application/json");
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
                "catalog unavailable: provider returned {}",
                response.status()
            )));
        }
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let value = response.json::<Value>().await.map_err(|error| {
            LauncherError::Marketplace(format!("catalog response is not valid JSON: {error}"))
        })?;
        let (plugins, response_version) = normalize_snapshot(value)?;
        Ok(ProviderResponse::Snapshot(ProviderSnapshot {
            etag,
            response_version,
            plugins,
        }))
    }
}

fn is_allowed_host(host: Option<&str>) -> bool {
    matches!(host, Some("dsh.tools"))
}

pub fn normalize_snapshot(value: Value) -> Result<(Vec<CatalogPlugin>, Option<String>)> {
    let response_version = value
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|value| value.len() <= 64);
    let entries = match &value {
        Value::Array(entries) => entries,
        Value::Object(object) => object
            .get("plugins")
            .or_else(|| object.get("data"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                LauncherError::Marketplace("catalog response has no plugin list".to_string())
            })?,
        _ => {
            return Err(LauncherError::Marketplace(
                "catalog response has no plugin list".to_string(),
            ))
        }
    };
    if entries.len() > 5_000 {
        return Err(LauncherError::Marketplace(
            "catalog contains too many plugins".to_string(),
        ));
    }
    let mut plugins = Vec::with_capacity(entries.len());
    let mut ids = BTreeSet::new();
    for entry in entries {
        let plugin = normalize_plugin(entry)?;
        if !ids.insert(plugin.id.clone()) {
            return Err(LauncherError::Marketplace(
                "catalog contains duplicate plugin IDs".to_string(),
            ));
        }
        plugins.push(plugin);
    }
    Ok((plugins, response_version))
}

fn normalize_plugin(value: &Value) -> Result<CatalogPlugin> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("plugin entry is not an object"))?;
    let install = object
        .get("installSpec")
        .or_else(|| object.get("install_spec"));
    let owner = string_field(install, "owner").or_else(|| string_field(Some(value), "owner"));
    let repository = string_field(install, "repository")
        .or_else(|| string_field(install, "repo"))
        .or_else(|| string_field(Some(value), "repository"))
        .or_else(|| string_field(Some(value), "repo"));
    let (owner, repository) = match (owner, repository) {
        (Some(owner), Some(repository)) => (owner.to_owned(), repository.to_owned()),
        _ => split_repository(
            string_field(Some(value), "id").or_else(|| string_field(Some(value), "repository")),
        )?,
    };
    validate_repository_part(&owner, "owner")?;
    validate_repository_part(&repository, "repository")?;
    let subdirectory = string_field(install, "subdirectory")
        .or_else(|| string_field(install, "path"))
        .map(str::to_owned);
    if let Some(subdirectory) = &subdirectory {
        validate_subdirectory(subdirectory)?;
    }
    let reference = string_field(install, "ref")
        .or_else(|| string_field(install, "reference"))
        .map(str::to_owned);
    if let Some(reference) = &reference {
        validate_reference(reference)?;
    }
    let expected_id = match &subdirectory {
        Some(subdirectory) => format!("{owner}/{repository}/{subdirectory}"),
        None => format!("{owner}/{repository}"),
    };
    let id = string_field(Some(value), "id").unwrap_or(&expected_id);
    if id != expected_id {
        return Err(invalid("plugin ID does not match its installation spec"));
    }
    let name = required_text(object.get("name"), "name", 160)?;
    let description = text_or_empty(object.get("description"), 5_000)?;
    let category = optional_text(object.get("category"), 100)?;
    let tags = string_list(object.get("tags"), 32, 80)?;
    let popularity_value = object.get("popularity").unwrap_or(value);
    let popularity = Popularity {
        marketplace_rank: number_field(popularity_value, "marketplaceRank")
            .or_else(|| number_field(popularity_value, "marketplace_rank"))
            .map(|value| u32::try_from(value).map_err(|_| invalid("marketplace rank is too large")))
            .transpose()?,
        ranking_updated_at: optional_timestamp(popularity_value, "rankingUpdatedAt")
            .or_else(|| optional_timestamp(popularity_value, "ranking_updated_at")),
        github_stars: number_field(popularity_value, "githubStars")
            .or_else(|| number_field(popularity_value, "github_stars")),
        stars_fetched_at: optional_timestamp(popularity_value, "starsFetchedAt")
            .or_else(|| optional_timestamp(popularity_value, "stars_fetched_at")),
    };
    Ok(CatalogPlugin {
        id: expected_id,
        name,
        repository_url: format!("https://github.com/{owner}/{repository}"),
        install_spec: InstallSpec {
            owner,
            repository,
            subdirectory,
            reference,
        },
        description,
        category,
        tags,
        source_updated_at: optional_timestamp(value, "sourceUpdatedAt")
            .or_else(|| optional_timestamp(value, "source_updated_at"))
            .or_else(|| optional_timestamp(value, "updatedAt")),
        validated_at: optional_timestamp(value, "validatedAt")
            .or_else(|| optional_timestamp(value, "validated_at")),
        popularity,
    })
}

fn string_field<'a>(value: Option<&'a Value>, key: &str) -> Option<&'a str> {
    value?.get(key)?.as_str()
}

fn number_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64()
}

fn split_repository(value: Option<&str>) -> Result<(String, String)> {
    let value = value.ok_or_else(|| invalid("plugin has no repository"))?;
    let mut parts = value.split('/');
    let owner = parts
        .next()
        .ok_or_else(|| invalid("plugin repository is invalid"))?;
    let repository = parts
        .next()
        .ok_or_else(|| invalid("plugin repository is invalid"))?;
    if parts.next().is_some() {
        return Err(invalid("plugin repository is invalid"));
    }
    Ok((owner.to_owned(), repository.to_owned()))
}

fn required_text(value: Option<&Value>, field: &str, max: usize) -> Result<String> {
    optional_text(value, max)?.ok_or_else(|| invalid(&format!("plugin {field} is missing")))
}

fn text_or_empty(value: Option<&Value>, max: usize) -> Result<String> {
    Ok(optional_text(value, max)?.unwrap_or_default())
}

fn optional_text(value: Option<&Value>, max: usize) -> Result<Option<String>> {
    let Some(value) = value else { return Ok(None) };
    let value = value
        .as_str()
        .ok_or_else(|| invalid("plugin text field is invalid"))?;
    if value.len() > max || value.chars().any(char::is_control) {
        return Err(invalid("plugin text field is invalid"));
    }
    Ok(Some(value.to_owned()))
}

fn string_list(
    value: Option<&Value>,
    max_items: usize,
    max_item_len: usize,
) -> Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| invalid("plugin tags are invalid"))?;
    if values.len() > max_items {
        return Err(invalid("plugin has too many tags"));
    }
    values
        .iter()
        .map(|value| {
            let tag = value
                .as_str()
                .ok_or_else(|| invalid("plugin tag is invalid"))?;
            if tag.len() > max_item_len || tag.chars().any(char::is_control) {
                return Err(invalid("plugin tag is invalid"));
            }
            Ok(tag.to_owned())
        })
        .collect()
}

fn optional_timestamp(value: &Value, key: &str) -> Option<String> {
    let value = value.get(key)?.as_str()?;
    (value.len() <= 64 && !value.chars().any(char::is_control)).then(|| value.to_owned())
}

fn validate_repository_part(value: &str, field: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= 100
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    valid
        .then_some(())
        .ok_or_else(|| invalid(&format!("plugin {field} is invalid")))
}

fn validate_subdirectory(value: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= 300
        && !value.starts_with('/')
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'));
    valid
        .then_some(())
        .ok_or_else(|| invalid("plugin subdirectory is invalid"))
}

fn validate_reference(value: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'));
    valid
        .then_some(())
        .ok_or_else(|| invalid("plugin ref is invalid"))
}

fn invalid(message: &str) -> LauncherError {
    LauncherError::Marketplace(format!("catalog validation failed: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plugin() -> Value {
        json!({
            "id": "owner/repo/packages/plugin",
            "name": "plugin",
            "description": "A valid plugin.",
            "installSpec": { "owner": "owner", "repository": "repo", "subdirectory": "packages/plugin", "ref": "v1.2.3" },
            "repositoryUrl": "https://malicious.example/not-used",
            "popularity": { "marketplaceRank": 8, "githubStars": 312 }
        })
    }

    #[test]
    fn normalizes_only_the_validated_install_spec() {
        let (plugins, _) =
            normalize_snapshot(json!({ "version": "v1", "plugins": [plugin()] })).unwrap();
        let plugin = &plugins[0];
        assert_eq!(plugin.repository_url, "https://github.com/owner/repo");
        assert_eq!(
            plugin.install_spec.subdirectory.as_deref(),
            Some("packages/plugin")
        );
        assert_eq!(plugin.popularity.marketplace_rank, Some(8));
        assert_eq!(plugin.popularity.github_stars, Some(312));
    }

    #[test]
    fn rejects_a_malicious_or_inconsistent_repository() {
        let mut entry = plugin();
        entry["installSpec"]["owner"] = json!("owner;rm");
        assert!(normalize_snapshot(json!({ "plugins": [entry] })).is_err());

        let mut entry = plugin();
        entry["id"] = json!("other/repository");
        assert!(normalize_snapshot(json!({ "plugins": [entry] })).is_err());
    }

    #[test]
    fn keeps_rank_and_stars_as_separate_fields() {
        let mut entry = plugin();
        entry["popularity"] = json!({ "githubStars": 20 });
        let (plugins, _) = normalize_snapshot(json!({ "plugins": [entry] })).unwrap();
        assert_eq!(plugins[0].popularity.marketplace_rank, None);
        assert_eq!(plugins[0].popularity.github_stars, Some(20));
    }
}
