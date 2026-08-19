use std::collections::BTreeSet;

use serde_json::Value;

use crate::error::{LauncherError, Result};
use crate::marketplace::types::{CatalogPlugin, InstallSpec, Popularity};

use super::fields::{
    category_label, invalid, localized_description, optional_text, required_text, string_list,
    text_value, validate_catalog_id, validate_repository_part,
};
use super::internal::normalize_internal_plugin;
use super::source::{parse_github_repository_url, parse_github_source, registry_source};

pub(super) fn normalize_registry_snapshot(
    value: &Value,
) -> Result<(
    Vec<CatalogPlugin>,
    Option<String>,
    Option<String>,
    Option<u32>,
)> {
    let (entries, categories, response_version, catalog_updated_at, declared_count) = match value {
        Value::Array(entries) => (entries, None, None, None, None),
        Value::Object(object) => {
            let entries = object
                .get("plugins")
                .or_else(|| object.get("data"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    LauncherError::Marketplace("catalog response has no plugin list".to_string())
                })?;
            let response_version =
                text_value(object.get("version")).or_else(|| text_value(object.get("updated")));
            let catalog_updated_at = text_value(object.get("updated"));
            let declared_count = object
                .get("count")
                .and_then(Value::as_u64)
                .map(|count| {
                    u32::try_from(count).map_err(|_| invalid("catalog count is too large"))
                })
                .transpose()?;
            (
                entries,
                object.get("categories"),
                response_version,
                catalog_updated_at,
                declared_count,
            )
        }
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
    let validated_at = chrono::Utc::now().to_rfc3339();
    let mut plugins = Vec::with_capacity(entries.len());
    let mut ids = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let plugin = match normalize_plugin(entry, categories, &validated_at) {
            Ok(plugin) => plugin,
            Err(error) => {
                tracing::warn!(catalog_entry_index = index, %error, "ignoring invalid marketplace entry");
                continue;
            }
        };
        if !ids.insert(plugin.id.clone()) {
            tracing::warn!(catalog_entry_index = index, plugin_id = %plugin.id, "ignoring duplicate marketplace entry");
            continue;
        }
        plugins.push(plugin);
    }
    if plugins.is_empty() {
        return Err(LauncherError::Marketplace(
            "catalog contains no valid plugins".to_string(),
        ));
    }
    let catalog_count = declared_count.or_else(|| u32::try_from(entries.len()).ok());
    Ok((plugins, response_version, catalog_updated_at, catalog_count))
}

fn normalize_plugin(
    value: &Value,
    categories: Option<&Value>,
    validated_at: &str,
) -> Result<CatalogPlugin> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("plugin entry is not an object"))?;
    if object.contains_key("installSpec") || object.contains_key("install_spec") {
        return normalize_internal_plugin(value, validated_at);
    }

    let name = required_text(object.get("name"), "name", 160)?;
    let description = localized_description(object.get("description"))?;
    let category_id = optional_text(object.get("category"), 100)?;
    let category = category_id
        .as_deref()
        .and_then(|id| category_label(categories, id))
        .or_else(|| category_id.clone());
    let tags = string_list(object.get("tags"), 32, 80)?;

    let url_value = object
        .get("url")
        .or_else(|| object.get("page"))
        .or_else(|| object.get("repositoryUrl"));
    let url_text = optional_text(url_value, 500)?;
    let url_repository = url_text
        .as_deref()
        .map(parse_github_repository_url)
        .transpose()?;
    let owner_field = optional_text(object.get("owner"), 100)?;
    let repository_field = optional_text(object.get("repository"), 100)?
        .or_else(|| optional_text(object.get("repo"), 100).ok().flatten());
    if let Some(owner) = &owner_field {
        validate_repository_part(owner, "owner")?;
    }
    if let Some(repository) = &repository_field {
        validate_repository_part(repository, "repository")?;
    }
    if let Some((url_owner, url_repository_name)) = &url_repository {
        if owner_field
            .as_deref()
            .is_some_and(|owner| owner != url_owner)
            || repository_field
                .as_deref()
                .is_some_and(|repository| repository != url_repository_name)
        {
            return Err(invalid("plugin repository fields do not match its URL"));
        }
    }

    let source = registry_source(object)?;
    let source_parts = source
        .as_deref()
        .map(parse_github_source)
        .transpose()?
        .flatten();
    let owner = owner_field
        .or_else(|| url_repository.as_ref().map(|(owner, _)| owner.clone()))
        .or_else(|| source_parts.as_ref().map(|parts| parts.0.clone()))
        .unwrap_or_default();
    let repository = repository_field
        .or_else(|| {
            url_repository
                .as_ref()
                .map(|(_, repository)| repository.clone())
        })
        .or_else(|| source_parts.as_ref().map(|parts| parts.1.clone()))
        .unwrap_or_default();
    if !owner.is_empty() {
        validate_repository_part(&owner, "owner")?;
    }
    if !repository.is_empty() {
        validate_repository_part(&repository, "repository")?;
    }

    let subdirectory = source_parts.as_ref().and_then(|parts| parts.2.clone());
    let reference = source_parts.as_ref().and_then(|parts| parts.3.clone());
    let canonical_repository_url = if !owner.is_empty() && !repository.is_empty() {
        Some(format!("https://github.com/{owner}/{repository}"))
    } else {
        None
    };
    let id = optional_text(object.get("id"), 320)?.unwrap_or_else(|| {
        if !owner.is_empty() && !repository.is_empty() {
            match &subdirectory {
                Some(subdirectory) => format!("{owner}/{repository}/{subdirectory}"),
                None => format!("{owner}/{repository}"),
            }
        } else {
            source.clone().unwrap_or_else(|| name.clone())
        }
    });
    validate_catalog_id(&id)?;

    let install_spec = source.map(|source| InstallSpec {
        owner,
        repository,
        subdirectory,
        reference,
        source,
    });
    let github_stars = object.get("stars").and_then(Value::as_u64);
    let stars_fetched_at = if github_stars.is_some() {
        Some(validated_at.to_string())
    } else {
        None
    };
    let popularity = Popularity {
        marketplace_rank: None,
        ranking_updated_at: None,
        github_stars,
        stars_fetched_at,
    };

    Ok(CatalogPlugin {
        id,
        name,
        repository_url: canonical_repository_url,
        install_spec,
        description,
        category,
        category_id,
        tags,
        source_updated_at: optional_text(object.get("added"), 64)?
            .or(optional_text(object.get("sourceUpdatedAt"), 64)?)
            .or(optional_text(object.get("updatedAt"), 64)?),
        validated_at: Some(validated_at.to_string()),
        popularity,
    })
}
