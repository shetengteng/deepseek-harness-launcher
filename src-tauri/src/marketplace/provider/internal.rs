use serde_json::Value;

use crate::error::Result;

use crate::marketplace::types::{CatalogPlugin, InstallSpec, Popularity};

use super::fields::{
    invalid, number_field, optional_text, optional_timestamp, required_text, split_repository,
    string_field, string_list, text_or_empty, validate_install_source, validate_reference,
    validate_repository_part, validate_subdirectory,
};

pub(super) fn normalize_internal_plugin(
    value: &Value,
    validated_at: &str,
) -> Result<CatalogPlugin> {
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
    let source = string_field(install, "source")
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let mut source = format!("github:{owner}/{repository}");
            if let Some(subdirectory) = &subdirectory {
                source.push_str("#path:");
                source.push_str(subdirectory);
            }
            source
        });
    validate_install_source(&source)?;
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
    let category_id = optional_text(
        object
            .get("categoryId")
            .or_else(|| object.get("category_id")),
        100,
    )?;
    let tags = string_list(object.get("tags"), 32, 80)?;
    let popularity_value = object.get("popularity").unwrap_or(value);
    let popularity = Popularity {
        marketplace_rank: None,
        ranking_updated_at: None,
        github_stars: number_field(popularity_value, "githubStars")
            .or_else(|| number_field(popularity_value, "github_stars")),
        stars_fetched_at: optional_timestamp(popularity_value, "starsFetchedAt")
            .or_else(|| optional_timestamp(popularity_value, "stars_fetched_at")),
    };
    Ok(CatalogPlugin {
        id: expected_id,
        name,
        repository_url: Some(format!("https://github.com/{owner}/{repository}")),
        install_spec: Some(InstallSpec {
            owner,
            repository,
            subdirectory,
            reference,
            source,
        }),
        description,
        category,
        category_id,
        tags,
        source_updated_at: optional_timestamp(value, "sourceUpdatedAt")
            .or_else(|| optional_timestamp(value, "source_updated_at"))
            .or_else(|| optional_timestamp(value, "updatedAt")),
        validated_at: optional_timestamp(value, "validatedAt")
            .or_else(|| optional_timestamp(value, "validated_at"))
            .or_else(|| Some(validated_at.to_string())),
        popularity,
    })
}
