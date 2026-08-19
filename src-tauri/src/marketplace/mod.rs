mod cache;
mod catalog;
mod install;
mod inventory;
mod provider;
pub mod types;

use chrono::{DateTime, Utc};
use tauri::{AppHandle, State};

use crate::error::{LauncherError, Result};

pub use install::MarketplaceOperations;
use install::{custom_preview, install_catalog_plugin, install_custom_plugin, remove_plugin};
use provider::{ProviderResponse, RegistryProvider, PROVIDER_ID, PROVIDER_LABEL, PROVIDER_URL};
use types::{
    CatalogCache, CatalogPlugin, MarketplaceCustomInstallRequest, MarketplaceInstallRequest,
    MarketplaceQuery, MarketplaceRemoveRequest, MarketplaceSnapshot, MarketplaceSource,
};

#[tauri::command]
pub async fn marketplace_query(query: MarketplaceQuery) -> Result<MarketplaceSnapshot> {
    let cache = match cache::load_for_provider(PROVIDER_ID)? {
        Some(cache) => {
            if needs_background_refresh(cache.fetched_at, Utc::now()) {
                let refresh_input = cache.clone();
                tokio::spawn(async move {
                    if let Err(error) = refresh_cache(Some(refresh_input)).await {
                        tracing::debug!(%error, "background marketplace refresh failed");
                    }
                });
            }
            cache
        }
        None => refresh_cache(None).await?,
    };
    snapshot(cache, query)
}

fn needs_background_refresh(fetched_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    cache::is_stale(fetched_at, now)
}

fn fallback_to_cached_catalog(
    existing: Option<CatalogCache>,
    error: LauncherError,
) -> Result<CatalogCache> {
    match (&error, existing) {
        (LauncherError::Marketplace(message), Some(cache))
            if message.starts_with("catalog unavailable") =>
        {
            tracing::warn!(%error, "marketplace refresh failed; using the cached catalog");
            Ok(cache)
        }
        _ => Err(error),
    }
}

#[tauri::command]
pub async fn marketplace_refresh() -> Result<MarketplaceSnapshot> {
    let existing = cache::load_for_provider(PROVIDER_ID)?;
    let refreshed = match refresh_cache(existing.clone()).await {
        Ok(cache) => cache,
        Err(error) => fallback_to_cached_catalog(existing, error)?,
    };
    let profile = inventory::list_profiles()?
        .into_iter()
        .next()
        .unwrap_or_else(|| "web".to_string());
    snapshot(
        refreshed,
        MarketplaceQuery {
            query: None,
            category: None,
            installed_only: None,
            sort: types::MarketplaceSort::Relevance,
            profile,
        },
    )
}

#[tauri::command]
pub async fn marketplace_parse_custom_install(
    request: MarketplaceCustomInstallRequest,
) -> Result<install::CustomInstallPreview> {
    custom_preview(&request.command)
}

#[tauri::command]
pub async fn marketplace_install(
    app: AppHandle,
    operations: State<'_, MarketplaceOperations>,
    request: MarketplaceInstallRequest,
) -> Result<types::MarketplaceOperation> {
    let _guard = operations.acquire()?;
    let cache = cache::load_for_provider(PROVIDER_ID)?.ok_or_else(|| {
        LauncherError::Marketplace("catalog is unavailable; refresh before installing".to_string())
    })?;
    let plugin = find_verified_plugin(&cache.plugins, &request.plugin_id).ok_or_else(|| {
        LauncherError::Marketplace("plugin is not present in the verified catalog".to_string())
    })?;
    let source = plugin
        .install_spec
        .as_ref()
        .map(|spec| spec.source())
        .ok_or_else(|| {
            LauncherError::Marketplace("plugin has no verified install source".to_string())
        })?;
    install_catalog_plugin(&app, plugin.id.clone(), request.profile, source).await
}

fn find_verified_plugin<'a>(plugins: &'a [CatalogPlugin], id: &str) -> Option<&'a CatalogPlugin> {
    plugins.iter().find(|plugin| {
        plugin.id == id && plugin.validated_at.is_some() && plugin.install_spec.is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(validated_at: Option<&str>) -> CatalogPlugin {
        CatalogPlugin {
            id: "owner/plugin".to_string(),
            name: "plugin".to_string(),
            repository_url: Some("https://github.com/owner/plugin".to_string()),
            install_spec: Some(types::InstallSpec {
                owner: "owner".to_string(),
                repository: "plugin".to_string(),
                subdirectory: None,
                reference: None,
                source: "github:owner/plugin".to_string(),
            }),
            description: String::new(),
            category: None,
            category_id: None,
            tags: Vec::new(),
            source_updated_at: None,
            validated_at: validated_at.map(str::to_owned),
            popularity: types::Popularity {
                marketplace_rank: None,
                ranking_updated_at: None,
                github_stars: None,
                stars_fetched_at: None,
            },
        }
    }

    #[test]
    fn install_lookup_rejects_unvalidated_catalog_entries() {
        let unvalidated = plugin(None);
        assert!(find_verified_plugin(&[unvalidated], "owner/plugin").is_none());

        let validated = plugin(Some("2026-08-18T09:00:00Z"));
        assert!(find_verified_plugin(&[validated], "owner/plugin").is_some());
    }

    #[test]
    fn refreshes_only_a_stale_catalog_in_the_background() {
        let now = Utc::now();
        assert!(!needs_background_refresh(
            now - chrono::Duration::hours(23),
            now
        ));
        assert!(needs_background_refresh(
            now - chrono::Duration::hours(25),
            now
        ));
    }

    #[test]
    fn uses_a_cached_catalog_when_the_source_is_unavailable() {
        let cache = CatalogCache {
            provider_id: PROVIDER_ID.to_string(),
            etag: Some("etag".to_string()),
            fetched_at: Utc::now(),
            response_version: Some("2026-08-19".to_string()),
            catalog_updated_at: Some("2026-08-19".to_string()),
            catalog_count: Some(1),
            plugins: vec![plugin(Some("2026-08-19T00:00:00Z"))],
        };
        let recovered = fallback_to_cached_catalog(
            Some(cache),
            LauncherError::Marketplace("catalog unavailable: timed out".to_string()),
        )
        .unwrap();

        assert_eq!(recovered.plugins.len(), 1);
        assert_eq!(recovered.plugins[0].id, "owner/plugin");
        assert!(fallback_to_cached_catalog(
            None,
            LauncherError::Marketplace("catalog unavailable: timed out".to_string()),
        )
        .is_err());
    }
}

#[tauri::command]
pub async fn marketplace_install_custom(
    app: AppHandle,
    operations: State<'_, MarketplaceOperations>,
    request: MarketplaceCustomInstallRequest,
) -> Result<types::MarketplaceOperation> {
    let _guard = operations.acquire()?;
    install_custom_plugin(&app, &request.command).await
}

#[tauri::command]
pub async fn marketplace_remove(
    app: AppHandle,
    operations: State<'_, MarketplaceOperations>,
    request: MarketplaceRemoveRequest,
) -> Result<types::MarketplaceOperation> {
    let _guard = operations.acquire()?;
    remove_plugin(&app, request.installation_id, request.profile).await
}

async fn refresh_cache(existing: Option<CatalogCache>) -> Result<CatalogCache> {
    let provider = RegistryProvider::new()?;
    let matching_cache = existing
        .as_ref()
        .filter(|cache| cache.provider_id == PROVIDER_ID);
    match provider
        .fetch(matching_cache.and_then(|cache| cache.etag.as_deref()))
        .await?
    {
        ProviderResponse::NotModified => {
            let mut cache = matching_cache.cloned().ok_or_else(|| {
                LauncherError::Marketplace(
                    "provider returned not-modified without a catalog cache".to_string(),
                )
            })?;
            cache.fetched_at = Utc::now();
            cache::save(&cache)?;
            Ok(cache)
        }
        ProviderResponse::Snapshot(snapshot) => {
            let cache = CatalogCache {
                provider_id: PROVIDER_ID.to_string(),
                etag: snapshot.etag,
                fetched_at: Utc::now(),
                response_version: snapshot.response_version,
                catalog_updated_at: snapshot.catalog_updated_at,
                catalog_count: snapshot.catalog_count,
                plugins: snapshot.plugins,
            };
            cache::save(&cache)?;
            Ok(cache)
        }
    }
}

fn snapshot(cache: CatalogCache, query: MarketplaceQuery) -> Result<MarketplaceSnapshot> {
    let profiles = inventory::list_profiles()?;
    inventory::validate_available_profile(&query.profile, &profiles)?;
    let inventory = inventory::read_profile(&query.profile)?;
    let plugins = catalog::filter_and_sort(catalog::merge(&cache.plugins, &inventory), &query);
    Ok(MarketplaceSnapshot {
        source: MarketplaceSource {
            label: PROVIDER_LABEL.to_string(),
            url: PROVIDER_URL.to_string(),
            fetched_at: Some(cache.fetched_at.to_rfc3339()),
            catalog_updated_at: cache.catalog_updated_at.clone(),
            catalog_count: cache.catalog_count,
            stale: cache::is_stale(cache.fetched_at, Utc::now()),
        },
        plugins,
        profiles,
    })
}
