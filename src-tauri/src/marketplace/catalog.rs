use std::cmp::Reverse;

use super::types::{
    CatalogPlugin, InventoryPlugin, MarketplacePlugin, MarketplaceQuery, MarketplaceSort,
    PluginStatus, Popularity,
};

pub fn merge(catalog: &[CatalogPlugin], inventory: &[InventoryPlugin]) -> Vec<MarketplacePlugin> {
    let mut matched = vec![false; inventory.len()];
    let mut plugins = catalog
        .iter()
        .map(|plugin| {
            let source = plugin.install_spec.source();
            let installed = inventory
                .iter()
                .enumerate()
                .find(|(_, item)| item.source == source);
            if let Some((index, _)) = installed {
                matched[index] = true;
            }
            MarketplacePlugin {
                id: plugin.id.clone(),
                name: plugin.name.clone(),
                repository_url: Some(plugin.repository_url.clone()),
                install_spec: Some(plugin.install_spec.clone()),
                description: plugin.description.clone(),
                category: plugin.category.clone(),
                tags: plugin.tags.clone(),
                source_updated_at: plugin.source_updated_at.clone(),
                validated_at: plugin.validated_at.clone(),
                popularity: plugin.popularity.clone(),
                status: if installed.is_some() {
                    PluginStatus::Installed
                } else {
                    PluginStatus::Available
                },
                installation_id: installed.map(|(_, item)| item.installation_id.clone()),
                installed_source: installed.map(|(_, item)| item.source.clone()),
                local_only: false,
            }
        })
        .collect::<Vec<_>>();
    for (index, item) in inventory.iter().enumerate() {
        if matched[index] {
            continue;
        }
        plugins.push(MarketplacePlugin {
            id: format!("local:{}", item.installation_id),
            name: item.name.clone(),
            repository_url: None,
            install_spec: None,
            description: "此插件未被当前目录收录。".to_string(),
            category: None,
            tags: vec![],
            source_updated_at: None,
            validated_at: None,
            popularity: Popularity {
                marketplace_rank: None,
                ranking_updated_at: None,
                github_stars: None,
                stars_fetched_at: None,
            },
            status: PluginStatus::Installed,
            installation_id: Some(item.installation_id.clone()),
            installed_source: Some(item.source.clone()),
            local_only: true,
        });
    }
    plugins
}

pub fn filter_and_sort(
    mut plugins: Vec<MarketplacePlugin>,
    query: &MarketplaceQuery,
) -> Vec<MarketplacePlugin> {
    let needle = query
        .query
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    plugins.retain(|plugin| {
        let matches_query = needle.is_empty()
            || [
                plugin.name.as_str(),
                plugin.id.as_str(),
                plugin.description.as_str(),
            ]
            .into_iter()
            .chain(plugin.tags.iter().map(String::as_str))
            .any(|field| field.to_lowercase().contains(&needle));
        let matches_category = query.category.as_deref().is_none_or(|category| {
            category == "all" || plugin.category.as_deref() == Some(category)
        });
        let matches_installed = !query.installed_only.unwrap_or(false)
            || plugin.status == PluginStatus::Installed
            || plugin.status == PluginStatus::UpdateAvailable;
        matches_query && matches_category && matches_installed
    });
    match query.sort {
        MarketplaceSort::Popularity => {
            plugins.sort_by_key(|plugin| plugin.popularity.marketplace_rank.unwrap_or(u32::MAX))
        }
        MarketplaceSort::Updated => {
            plugins.sort_by_key(|plugin| Reverse(plugin.source_updated_at.clone()))
        }
        MarketplaceSort::Relevance => plugins.sort_by_key(|plugin| plugin.name.to_lowercase()),
    }
    plugins
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::types::{InstallSpec, Popularity};

    fn catalog() -> CatalogPlugin {
        CatalogPlugin {
            id: "owner/repo".to_string(),
            name: "plugin".to_string(),
            repository_url: "https://github.com/owner/repo".to_string(),
            install_spec: InstallSpec {
                owner: "owner".to_string(),
                repository: "repo".to_string(),
                subdirectory: None,
                reference: None,
            },
            description: "Test plugin".to_string(),
            category: None,
            tags: vec![],
            source_updated_at: None,
            validated_at: None,
            popularity: Popularity {
                marketplace_rank: Some(8),
                ranking_updated_at: None,
                github_stars: Some(5),
                stars_fetched_at: None,
            },
        }
    }

    #[test]
    fn keeps_catalog_external_dependencies_removable() {
        let inventory = vec![InventoryPlugin {
            installation_id: "external".to_string(),
            name: "external".to_string(),
            source: "npm:external".to_string(),
        }];
        let results = merge(&[catalog()], &inventory);
        assert_eq!(results.len(), 2);
        assert!(results[1].local_only);
        assert_eq!(results[1].installation_id.as_deref(), Some("external"));
    }

    #[test]
    fn maps_only_an_exact_spec_to_installed() {
        let inventory = vec![InventoryPlugin {
            installation_id: "plugin".to_string(),
            name: "plugin".to_string(),
            source: "github:owner/repo".to_string(),
        }];
        assert_eq!(
            merge(&[catalog()], &inventory)[0].status,
            PluginStatus::Installed
        );
        let inventory = vec![InventoryPlugin {
            installation_id: "plugin".to_string(),
            name: "plugin".to_string(),
            source: "github:owner/other".to_string(),
        }];
        assert_eq!(
            merge(&[catalog()], &inventory)[0].status,
            PluginStatus::Available
        );
    }
}
