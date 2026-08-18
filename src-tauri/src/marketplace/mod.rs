mod cache;
mod catalog;
mod install;
mod inventory;
mod provider;
pub mod types;

use chrono::Utc;
use tauri::{AppHandle, State};

use crate::error::{LauncherError, Result};

pub use install::MarketplaceOperations;
use install::{custom_preview, install_catalog_plugin, install_custom_plugin, remove_plugin};
use provider::{Dsh1024Provider, ProviderResponse, PROVIDER_ID, PROVIDER_LABEL};
use types::{
    CatalogCache, MarketplaceCustomInstallRequest, MarketplaceInstallRequest, MarketplaceQuery,
    MarketplaceRemoveRequest, MarketplaceSnapshot, MarketplaceSource,
};

#[tauri::command]
pub async fn marketplace_query(query: MarketplaceQuery) -> Result<MarketplaceSnapshot> {
    let cache = match cache::load()? {
        Some(cache) => {
            if !cache::is_stale(cache.fetched_at, Utc::now()) {
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

#[tauri::command]
pub async fn marketplace_refresh() -> Result<MarketplaceSnapshot> {
    let existing = cache::load()?;
    let refreshed = refresh_cache(existing).await?;
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
            sort: types::MarketplaceSort::Popularity,
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
    let cache = cache::load()?.ok_or_else(|| {
        LauncherError::Marketplace("catalog is unavailable; refresh before installing".to_string())
    })?;
    let plugin = cache
        .plugins
        .iter()
        .find(|plugin| plugin.id == request.plugin_id)
        .ok_or_else(|| {
            LauncherError::Marketplace("plugin is not present in the verified catalog".to_string())
        })?;
    install_catalog_plugin(
        &app,
        plugin.id.clone(),
        request.profile,
        plugin.install_spec.source(),
    )
    .await
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
    let provider = Dsh1024Provider::new()?;
    match provider
        .fetch(existing.as_ref().and_then(|cache| cache.etag.as_deref()))
        .await?
    {
        ProviderResponse::NotModified => {
            let mut cache = existing.ok_or_else(|| {
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
            fetched_at: Some(cache.fetched_at.to_rfc3339()),
            stale: cache::is_stale(cache.fetched_at, Utc::now()),
        },
        plugins,
        profiles,
    })
}
