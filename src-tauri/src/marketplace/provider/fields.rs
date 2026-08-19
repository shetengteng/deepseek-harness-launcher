use serde_json::{Map, Value};

use crate::error::{LauncherError, Result};

pub(super) fn category_label(categories: Option<&Value>, id: &str) -> Option<String> {
    let category = categories?.get(id)?.as_object()?;
    let label = category
        .get("zh")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| category.get("en").and_then(Value::as_str))?;
    (label.len() <= 100 && !label.chars().any(char::is_control)).then(|| label.to_owned())
}

pub(super) fn localized_description(value: Option<&Value>) -> Result<String> {
    let Some(value) = value else {
        return Ok(String::new());
    };
    if let Some(text) = value.as_str() {
        return validate_text(text, 5_000).map(str::to_owned);
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid("plugin description is invalid"))?;
    let selected = object
        .get("zh")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| object.get("en").and_then(Value::as_str))
        .unwrap_or_default();
    validate_text(selected, 5_000).map(str::to_owned)
}

pub(super) fn string_field<'a>(value: Option<&'a Value>, key: &str) -> Option<&'a str> {
    value?.get(key)?.as_str()
}

pub(super) fn string_field_map<'a>(value: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

pub(super) fn number_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64()
}

pub(super) fn split_repository(value: Option<&str>) -> Result<(String, String)> {
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

pub(super) fn required_text(value: Option<&Value>, field: &str, max: usize) -> Result<String> {
    optional_text(value, max)?.ok_or_else(|| invalid(&format!("plugin {field} is missing")))
}

pub(super) fn text_or_empty(value: Option<&Value>, max: usize) -> Result<String> {
    Ok(optional_text(value, max)?.unwrap_or_default())
}

pub(super) fn optional_text(value: Option<&Value>, max: usize) -> Result<Option<String>> {
    let Some(value) = value else { return Ok(None) };
    let value = value
        .as_str()
        .ok_or_else(|| invalid("plugin text field is invalid"))?;
    Ok(Some(validate_text(value, max)?.to_owned()))
}

pub(super) fn text_value(value: Option<&Value>) -> Option<String> {
    let value = value?.as_str()?;
    (value.len() <= 64 && !value.chars().any(char::is_control)).then(|| value.to_owned())
}

pub(super) fn validate_text(value: &str, max: usize) -> Result<&str> {
    if value.len() > max || value.chars().any(char::is_control) {
        return Err(invalid("plugin text field is invalid"));
    }
    Ok(value)
}

pub(super) fn string_list(
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
            Ok(validate_text(tag, max_item_len)?.to_owned())
        })
        .collect()
}

pub(super) fn optional_timestamp(value: &Value, key: &str) -> Option<String> {
    let value = value.get(key)?.as_str()?;
    (value.len() <= 64 && !value.chars().any(char::is_control)).then(|| value.to_owned())
}

pub(super) fn validate_repository_part(value: &str, field: &str) -> Result<()> {
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

pub(super) fn validate_subdirectory(value: &str) -> Result<()> {
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

pub(super) fn validate_reference(value: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'));
    valid
        .then_some(())
        .ok_or_else(|| invalid("plugin ref is invalid"))
}

pub(super) fn validate_install_source(value: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= 500
        && value.is_ascii()
        && !value.chars().any(|character| {
            character.is_whitespace()
                || character.is_control()
                || matches!(
                    character,
                    '\'' | '"' | '`' | ';' | '|' | '&' | '<' | '>' | '$'
                )
        });
    valid
        .then_some(())
        .ok_or_else(|| invalid("plugin install source is invalid"))
}

pub(super) fn validate_catalog_id(value: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= 320
        && !value.chars().any(char::is_control)
        && !value.chars().any(char::is_whitespace);
    valid
        .then_some(())
        .ok_or_else(|| invalid("plugin ID is invalid"))
}

pub(super) fn valid_profile(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub(super) fn invalid(message: &str) -> LauncherError {
    LauncherError::Marketplace(format!("catalog validation failed: {message}"))
}
