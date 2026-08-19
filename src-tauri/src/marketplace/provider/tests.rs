use super::normalize::normalize_registry_snapshot;
use super::source::parse_install_source;
use crate::error::Result;
use crate::marketplace::types::CatalogPlugin;
use serde_json::{json, Value};

fn normalize_snapshot(value: Value) -> Result<(Vec<CatalogPlugin>, Option<String>)> {
    let (plugins, response_version, _, _) = normalize_registry_snapshot(&value)?;
    Ok((plugins, response_version))
}

#[test]
fn normalizes_the_curated_registry_with_localized_fields() {
    let (plugins, version) = normalize_snapshot(json!({
        "updated": "2026-08-17",
        "count": 1,
        "categories": { "theme": { "en": "Themes", "zh": "主题与外观" } },
        "plugins": [{
            "name": "dsh-theme",
            "owner": "owner",
            "url": "https://github.com/owner/dsh-theme",
            "category": "theme",
            "description": { "en": "A theme.", "zh": "一个主题。" },
            "npm": "@owner/dsh-theme",
            "stars": 312,
            "added": "2026-08-15"
        }]
    }))
    .unwrap();

    assert_eq!(version.as_deref(), Some("2026-08-17"));
    assert_eq!(plugins[0].id, "owner/dsh-theme");
    assert_eq!(plugins[0].description, "一个主题。");
    assert_eq!(plugins[0].category.as_deref(), Some("主题与外观"));
    assert_eq!(plugins[0].category_id.as_deref(), Some("theme"));
    assert_eq!(
        plugins[0].install_spec.as_ref().unwrap().source(),
        "npm:@owner/dsh-theme"
    );
    assert_eq!(plugins[0].popularity.marketplace_rank, None);
    assert_eq!(plugins[0].popularity.github_stars, Some(312));
}

#[test]
fn never_treats_an_upstream_popularity_field_as_a_market_rank() {
    let (plugins, _) = normalize_snapshot(json!({
        "plugins": [{
            "id": "owner/plugin",
            "name": "plugin",
            "description": "A plugin.",
            "installSpec": {
                "owner": "owner",
                "repository": "plugin",
                "source": "github:owner/plugin"
            },
            "popularity": {
                "marketplaceRank": 1,
                "githubStars": 42
            }
        }]
    }))
    .unwrap();

    assert_eq!(plugins[0].popularity.marketplace_rank, None);
    assert_eq!(plugins[0].popularity.github_stars, Some(42));
}

#[test]
fn extracts_a_github_source_from_the_curated_install_command() {
    let (plugins, _) = normalize_snapshot(json!({
        "plugins": [{
            "name": "dsh-cost-meter",
            "url": "https://github.com/owner/dsh-cost-meter",
            "description": { "en": "Usage metrics." },
            "install": "dsh plugin --profile web add github:owner/dsh-cost-meter#path:packages/plugin"
        }]
    }))
    .unwrap();

    let plugin = &plugins[0];
    let spec = plugin.install_spec.as_ref().unwrap();
    assert_eq!(
        spec.source(),
        "github:owner/dsh-cost-meter#path:packages/plugin"
    );
    assert_eq!(spec.subdirectory.as_deref(), Some("packages/plugin"));
}

#[test]
fn normalizes_the_registrys_absolute_subdirectory_syntax() {
    let (plugins, _) = normalize_snapshot(json!({
        "plugins": [{
            "name": "dsh-trail#bundle",
            "url": "https://github.com/owner/dsh-trail/tree/main/packages/bundle",
            "install": "dsh plugin --profile web add github:owner/dsh-trail#path:/packages/bundle"
        }]
    }))
    .unwrap();

    let spec = plugins[0].install_spec.as_ref().unwrap();
    assert_eq!(spec.subdirectory.as_deref(), Some("packages/bundle"));
    assert_eq!(
        spec.source(),
        "github:owner/dsh-trail#path:/packages/bundle"
    );
}

#[test]
fn accepts_a_quoted_tarball_source_from_the_registry() {
    let (plugins, _) = normalize_snapshot(json!({
        "plugins": [{
            "name": "tarball-plugin",
            "url": "https://github.com/owner/tarball-plugin/releases/latest",
            "install": "dsh plugin --profile web add \"https://github.com/owner/tarball-plugin/releases/latest/download/plugin.tgz\""
        }]
    }))
    .unwrap();

    assert_eq!(
        plugins[0].install_spec.as_ref().unwrap().source(),
        "https://github.com/owner/tarball-plugin/releases/latest/download/plugin.tgz"
    );
}

#[test]
fn ignores_an_invalid_entry_without_hiding_valid_plugins() {
    let (plugins, _) = normalize_snapshot(json!({
        "plugins": [
            {
                "name": "valid-plugin",
                "url": "https://github.com/owner/valid-plugin",
                "npm": "valid-plugin"
            },
            {
                "name": "invalid-plugin",
                "url": "https://github.com/owner/invalid-plugin",
                "install": "npm install invalid-plugin"
            }
        ]
    }))
    .unwrap();

    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "valid-plugin");
}

#[test]
fn rejects_a_malicious_or_inconsistent_repository() {
    let mut entry = json!({
        "name": "plugin",
        "owner": "owner;rm",
        "url": "https://github.com/owner/repo",
        "install": "dsh plugin add github:owner/repo"
    });
    assert!(normalize_snapshot(json!({ "plugins": [entry.clone()] })).is_err());

    entry["owner"] = json!("other");
    assert!(normalize_snapshot(json!({ "plugins": [entry] })).is_err());
}

#[test]
fn rejects_remote_commands_and_shell_metacharacters() {
    assert!(
        parse_install_source("dsh plugin --profile web add github:owner/repo;rm -rf /").is_err()
    );
    assert!(parse_install_source("dsh plugin --profile web add 'github:owner/repo'").is_err());
    assert!(parse_install_source("npm install dsh-plugin").is_err());
}

#[test]
fn keeps_a_registry_entry_without_a_source_visible_but_not_installable() {
    let (plugins, _) = normalize_snapshot(json!({
        "plugins": [{
            "name": "documentation-only",
            "description": "No install source yet."
        }]
    }))
    .unwrap();
    assert!(plugins[0].install_spec.is_none());
    assert_eq!(plugins[0].id, "documentation-only");
}
