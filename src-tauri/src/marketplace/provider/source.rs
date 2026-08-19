use serde_json::{Map, Value};
use url::Url;

use crate::error::Result;

use super::fields::{
    invalid, split_repository, string_field_map, valid_profile, validate_install_source,
    validate_reference, validate_repository_part, validate_subdirectory,
};

pub(super) fn registry_source(object: &Map<String, Value>) -> Result<Option<String>> {
    if let Some(npm) = string_field_map(object, "npm") {
        if npm.trim().is_empty() {
            return Ok(None);
        }
        let source = if npm.starts_with("npm:") {
            npm.to_owned()
        } else {
            format!("npm:{npm}")
        };
        validate_install_source(&source)?;
        return Ok(Some(source));
    }
    if let Some(install) = string_field_map(object, "install") {
        return parse_install_source(install).map(Some);
    }
    if let Some(url) = string_field_map(object, "url") {
        let (owner, repository) = parse_github_repository_url(url)?;
        let source = format!("github:{owner}/{repository}");
        validate_install_source(&source)?;
        return Ok(Some(source));
    }
    Ok(None)
}

pub(super) fn parse_install_source(command: &str) -> Result<String> {
    if command.len() > 1_000 || command.chars().any(char::is_control) {
        return Err(invalid("plugin install source is invalid"));
    }
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    let source = match tokens.as_slice() {
        ["dsh", "plugin", "--profile", profile, "add", source] if valid_profile(profile) => *source,
        ["dsh", "plugin", "add", source] => *source,
        _ => return Err(invalid("plugin install command is not supported")),
    };
    let source = source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(source);
    validate_install_source(source)?;
    Ok(source.to_owned())
}

pub(super) fn parse_github_source(
    source: &str,
) -> Result<Option<(String, String, Option<String>, Option<String>)>> {
    let Some(rest) = source.strip_prefix("github:") else {
        return Ok(None);
    };
    let (repository, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    let (owner, repository_name) = split_repository(Some(repository))?;
    validate_repository_part(&owner, "owner")?;
    validate_repository_part(&repository_name, "repository")?;
    let mut subdirectory = None;
    let mut reference = None;
    if !fragment.is_empty() {
        if let Some(path) = fragment.strip_prefix("path:") {
            let path = path.strip_prefix('/').unwrap_or(path);
            validate_subdirectory(path)?;
            subdirectory = Some(path.to_owned());
        } else if let Some(reference_value) = fragment.strip_prefix("ref:") {
            validate_reference(reference_value)?;
            reference = Some(reference_value.to_owned());
        } else {
            return Err(invalid("plugin GitHub source fragment is invalid"));
        }
    }
    Ok(Some((owner, repository_name, subdirectory, reference)))
}

pub(super) fn parse_github_repository_url(value: &str) -> Result<(String, String)> {
    let url = Url::parse(value).map_err(|_| invalid("plugin repository URL is invalid"))?;
    if url.scheme() != "https" || url.host_str() != Some("github.com") {
        return Err(invalid(
            "plugin repository URL is not an allowed GitHub URL",
        ));
    }
    let mut segments = url
        .path_segments()
        .ok_or_else(|| invalid("plugin repository URL is invalid"))?
        .filter(|segment| !segment.is_empty());
    let owner = segments
        .next()
        .ok_or_else(|| invalid("plugin repository URL has no owner"))?;
    let repository = segments
        .next()
        .ok_or_else(|| invalid("plugin repository URL has no repository"))?;
    validate_repository_part(owner, "owner")?;
    validate_repository_part(repository, "repository")?;
    Ok((owner.to_owned(), repository.to_owned()))
}
