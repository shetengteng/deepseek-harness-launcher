use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::error::{LauncherError, Result};
use crate::host::SpawnDshWebOptions;

const MAX_COMMAND_LENGTH: usize = 2_048;
const MAX_PROFILE_LENGTH: usize = 64;
const MAX_SOURCE_LENGTH: usize = 1_024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_PLUGIN_ATTEMPTS: usize = 3;
const SUMMARY_LIMIT: usize = 280;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum PluginAction {
    Add,
    Remove,
}

impl PluginAction {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "add" => Some(Self::Add),
            "remove" => Some(Self::Remove),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PluginCommand {
    action: PluginAction,
    profile: String,
    source: String,
}

#[derive(Debug, Serialize)]
pub struct PluginCommandResult {
    action: PluginAction,
    profile: String,
    source: String,
    summary: String,
}

#[tauri::command]
pub async fn run_plugin_command(command: String) -> Result<PluginCommandResult> {
    let parsed = parse_plugin_command(&command)?;
    let options = super::host::build_spawn_options().await?;
    let mut last_summary = String::new();
    let mut last_code = None;
    for attempt in 0..MAX_PLUGIN_ATTEMPTS {
        let output = run_managed_plugin(&options, &parsed).await?;
        let captured = compact_output(&output.stdout, &output.stderr);
        last_summary = summarize_output(&captured);
        last_code = output.status.code();
        if output.status.success() {
            return Ok(PluginCommandResult {
                action: parsed.action,
                profile: parsed.profile,
                source: parsed.source,
                summary: last_summary,
            });
        }
        tracing::warn!(
            action = ?parsed.action,
            profile = %parsed.profile,
            source = %parsed.source,
            attempt,
            exit_code = ?last_code,
            summary = %last_summary,
            "dsh plugin command failed"
        );
        if parsed.action != PluginAction::Add {
            break;
        }
        let keys = blocked_build_keys(&captured, &parsed.source);
        if keys.is_empty() {
            break;
        }
        let profile_dir = resolve_dsh_home()?.join("profiles").join(&parsed.profile);
        allow_builds_in(&profile_dir, &keys)?;
        tracing::info!(
            profile = %parsed.profile,
            source = %parsed.source,
            keys = ?keys,
            attempt,
            "retrying plugin add after allowBuilds"
        );
    }

    Err(LauncherError::DshPlugin(format!(
        "dsh plugin command exited with code {last_code:?}: {last_summary}"
    )))
}

async fn run_managed_plugin(
    options: &SpawnDshWebOptions,
    parsed: &PluginCommand,
) -> Result<Output> {
    let mut child = Command::new(&options.node_executable);
    child
        .current_dir(&options.cwd)
        .arg("--expose-internals")
        .arg(&options.cli_entry)
        .args([
            "plugin",
            "--profile",
            &parsed.profile,
            match parsed.action {
                PluginAction::Add => "add",
                PluginAction::Remove => "remove",
            },
            &parsed.source,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .env_clear();
    for (key, value) in &options.env {
        child.env(key, value);
    }

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        child.creation_flags(CREATE_NO_WINDOW);
    }

    tokio::time::timeout(COMMAND_TIMEOUT, child.output())
        .await
        .map_err(|_| LauncherError::DshPlugin("command timed out after 5 minutes".to_string()))?
        .map_err(|error| LauncherError::DshPlugin(format!("failed to start managed dsh: {error}")))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InstalledPlugin {
    name: String,
    spec: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ProfilePluginList {
    profile: String,
    plugins: Vec<InstalledPlugin>,
}

#[derive(Debug, Deserialize, Default)]
struct ProfileManifest {
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    #[serde(default)]
    dsh: DshManifestSection,
}

#[derive(Debug, Deserialize, Default)]
struct DshManifestSection {
    #[serde(default)]
    profile: DshProfileManifest,
}

#[derive(Debug, Deserialize, Default)]
struct DshProfileManifest {
    #[serde(default)]
    bundles: Vec<String>,
}

#[tauri::command]
pub async fn list_profile_plugins(profile: String) -> Result<ProfilePluginList> {
    if !is_profile_valid(&profile) {
        return Err(invalid_command());
    }
    list_profile_plugins_in(&resolve_dsh_home()?, &profile)
}

fn resolve_dsh_home() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("DSH_HOME") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let base = directories::BaseDirs::new().ok_or_else(|| LauncherError::PathResolve {
        what: "dsh_home",
        cause: "HOME or APPDATA may be unset".to_string(),
    })?;
    Ok(base.home_dir().join(".dsh"))
}

fn list_profile_plugins_in(home: &Path, profile: &str) -> Result<ProfilePluginList> {
    let manifest_path = home.join("profiles").join(profile).join("package.json");
    if !manifest_path.is_file() {
        return Ok(ProfilePluginList {
            profile: profile.to_string(),
            plugins: Vec::new(),
        });
    }
    let raw = std::fs::read_to_string(&manifest_path)?;
    let manifest: ProfileManifest = serde_json::from_str(&raw)?;
    Ok(ProfilePluginList {
        profile: profile.to_string(),
        plugins: installed_plugins(&manifest),
    })
}

fn installed_plugins(manifest: &ProfileManifest) -> Vec<InstalledPlugin> {
    let bundles = manifest
        .dsh
        .profile
        .bundles
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    manifest
        .dependencies
        .iter()
        .filter(|(name, _)| bundles.contains(name.as_str()))
        .map(|(name, spec)| InstalledPlugin {
            name: name.clone(),
            spec: spec.clone(),
        })
        .collect()
}

fn parse_plugin_command(input: &str) -> Result<PluginCommand> {
    let command = input.trim();
    if command.is_empty() || command.len() > MAX_COMMAND_LENGTH {
        return Err(invalid_command());
    }
    let parts = command.split_whitespace().collect::<Vec<_>>();
    let ["dsh", "plugin", "--profile", profile, action, source] = parts.as_slice() else {
        return Err(invalid_command());
    };
    let Some(action) = PluginAction::parse(action) else {
        return Err(invalid_command());
    };
    if !is_profile_valid(profile) || !is_source_valid(source) {
        return Err(invalid_command());
    }

    Ok(PluginCommand {
        action,
        profile: (*profile).to_string(),
        source: (*source).to_string(),
    })
}

fn invalid_command() -> LauncherError {
    LauncherError::DshPlugin(
        "expected: dsh plugin --profile <profile> add|remove <source>".to_string(),
    )
}

fn is_profile_valid(profile: &str) -> bool {
    !profile.is_empty()
        && profile.len() <= MAX_PROFILE_LENGTH
        && profile
            .bytes()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, b'-' | b'_'))
}

fn is_source_valid(source: &str) -> bool {
    !source.is_empty()
        && source.len() <= MAX_SOURCE_LENGTH
        && !source.contains(['\'', '"'])
        && source.chars().all(|character| !character.is_control())
}

fn compact_output(stdout: &[u8], stderr: &[u8]) -> String {
    let text = format!("{} {}", decode_output(stdout), decode_output(stderr)).replace("\\\"", "\"");
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    tail_chars(&compact, 8_192)
}

fn decode_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect()
}

#[cfg(test)]
fn output_summary(stdout: &[u8], stderr: &[u8]) -> String {
    summarize_output(&compact_output(stdout, stderr))
}

fn summarize_output(compact: &str) -> String {
    if compact.is_empty() {
        return "操作已完成。".to_string();
    }
    let focused = focus_plugin_output(compact);
    prefix_chars(focused, SUMMARY_LIMIT)
}

fn focus_plugin_output(text: &str) -> &str {
    const MARKERS: [&str; 4] = [
        "ERR_PNPM_GIT_DEP_PREPARE_NOT_ALLOWED",
        "git-hosted package",
        "Ignored build scripts",
        "ERR_PNPM_",
    ];
    for marker in MARKERS {
        if let Some(index) = text.find(marker) {
            return &text[index.saturating_sub(24)..];
        }
    }
    let chars = text.chars().count();
    if chars <= SUMMARY_LIMIT {
        return text;
    }
    let skip = chars - SUMMARY_LIMIT;
    text.char_indices()
        .nth(skip)
        .map(|(index, _)| &text[index..])
        .unwrap_or(text)
}

fn prefix_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn tail_chars(text: &str, limit: usize) -> String {
    let chars = text.chars().count();
    if chars <= limit {
        return text.to_string();
    }
    text.chars().skip(chars - limit).collect()
}

fn blocked_build_keys(output: &str, source: &str) -> Vec<String> {
    if !is_blocked_build(output) {
        return Vec::new();
    }
    let mut keys = Vec::new();
    if let Some(name) = parse_git_prepare_package(output) {
        push_unique(&mut keys, name);
    }
    for name in parse_ignored_builds(output) {
        push_unique(&mut keys, name);
    }
    if keys.is_empty() {
        if let Some(name) = github_repo_name(source) {
            push_unique(&mut keys, name);
        }
    }
    keys
}

fn is_blocked_build(output: &str) -> bool {
    output.contains("ERR_PNPM_GIT_DEP_PREPARE_NOT_ALLOWED")
        || output.contains("ERR_PNPM_IGNORED_BUILDS")
        || output.contains("needs to execute build scripts")
        || output.contains("Ignored build scripts")
}

fn parse_git_prepare_package(output: &str) -> Option<String> {
    const MARKER: &str = "git-hosted package \"";
    let start = output.find(MARKER)? + MARKER.len();
    let raw = output[start..].split('"').next()?.trim();
    if raw.is_empty() {
        return None;
    }
    let name = match raw.rfind('@') {
        Some(index) if index > 0 => &raw[..index],
        _ => raw,
    };
    is_package_name(name).then(|| name.to_string())
}

fn parse_ignored_builds(output: &str) -> Vec<String> {
    let Some(index) = output.find("Ignored build scripts") else {
        return Vec::new();
    };
    let rest = output[index..]
        .split_once(':')
        .map(|(_, names)| names)
        .unwrap_or("");
    let end = rest
        .find(" ERR_")
        .or_else(|| rest.find(" dsh:"))
        .unwrap_or(rest.len());
    let mut names = Vec::new();
    for chunk in rest[..end].split(',') {
        let trimmed = chunk.trim().trim_end_matches('.');
        let name = match trimmed.rfind('@') {
            Some(index) if index > 0 => &trimmed[..index],
            _ => trimmed,
        };
        if is_package_name(name) {
            push_unique(&mut names, name.to_string());
        }
    }
    names
}

fn github_repo(source: &str) -> Option<String> {
    let rest = source
        .strip_prefix("github:")
        .or_else(|| source.strip_prefix("git+https://github.com/"))
        .or_else(|| source.strip_prefix("https://github.com/"))?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let repo = rest.split(['#', '&', '?']).next()?;
    let mut parts = repo.split('/');
    let owner = parts.next()?;
    let name = parts.next()?;
    if parts.next().is_some() || !is_github_segment(owner) || !is_github_segment(name) {
        return None;
    }
    Some(format!("{owner}/{name}"))
}

fn github_repo_name(source: &str) -> Option<String> {
    github_repo(source)?
        .split_once('/')
        .map(|(_, name)| name.to_string())
}

fn is_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'@' | b'/' | b'_' | b'.' | b'-')
        })
}

fn is_github_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

fn is_allow_builds_key(key: &str) -> bool {
    is_package_name(key)
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn allow_builds_in(profile_dir: &Path, keys: &[String]) -> Result<()> {
    let keys = keys
        .iter()
        .filter(|key| is_allow_builds_key(key))
        .cloned()
        .collect::<Vec<_>>();
    if keys.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(profile_dir)?;
    let path = profile_dir.join("pnpm-workspace.yaml");
    let yaml = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    std::fs::write(path, merge_allow_builds(&yaml, &keys))?;
    Ok(())
}

fn merge_allow_builds(yaml: &str, keys: &[String]) -> String {
    let eol = if yaml.contains("\r\n") { "\r\n" } else { "\n" };
    let mut map = BTreeMap::new();
    let block_re = |text: &str| -> Vec<(usize, usize, String)> {
        let mut blocks = Vec::new();
        let mut search = 0;
        while let Some(relative) = text[search..].find("allowBuilds:") {
            let start = search + relative;
            let after_key = start + "allowBuilds:".len();
            let after_line = text[after_key..]
                .find('\n')
                .map(|index| after_key + index + 1)
                .unwrap_or(text.len());
            let mut end = after_line;
            while end < text.len() {
                let line_end = text[end..]
                    .find('\n')
                    .map(|index| end + index + 1)
                    .unwrap_or(text.len());
                let line = text[end..line_end].trim_end_matches(['\r', '\n']);
                if line.is_empty() || line.starts_with(' ') || line.starts_with('\t') {
                    end = line_end;
                } else {
                    break;
                }
            }
            blocks.push((start, end, text[after_line..end].to_string()));
            search = end.max(start + 1);
        }
        blocks
    };
    let blocks = block_re(yaml);
    for (_, _, body) in &blocks {
        for line in body.lines() {
            let Some((key, value)) = parse_allow_builds_entry(line) else {
                continue;
            };
            map.insert(key, value);
        }
    }
    for key in keys {
        if is_allow_builds_key(key) {
            map.insert(key.clone(), "true".to_string());
        }
    }
    let block = map
        .iter()
        .map(|(key, value)| format!("  {}: {value}", quote_yaml_key(key)))
        .collect::<Vec<_>>()
        .join(eol);
    let block_text = format!("allowBuilds:{eol}{block}{eol}");
    if let Some((start, _, _)) = blocks.first() {
        let mut next = String::new();
        next.push_str(&yaml[..*start]);
        next.push_str(&block_text);
        let mut cursor = *start;
        for (block_start, block_end, _) in &blocks {
            if *block_start > cursor {
                next.push_str(&yaml[cursor..*block_start]);
            }
            cursor = *block_end;
        }
        next.push_str(&yaml[cursor..]);
        if next.ends_with(eol) {
            next
        } else {
            format!("{next}{eol}")
        }
    } else if yaml.is_empty() {
        block_text
    } else if yaml.ends_with('\n') {
        format!("{yaml}{block_text}")
    } else {
        format!("{yaml}{eol}{block_text}")
    }
}

fn parse_allow_builds_entry(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (raw_key, raw_value) = trimmed.rsplit_once(':')?;
    let mut key = raw_key.trim();
    if key.len() >= 2
        && ((key.starts_with('\'') && key.ends_with('\''))
            || (key.starts_with('"') && key.ends_with('"')))
    {
        key = &key[1..key.len() - 1];
    }
    let value = raw_value.trim();
    if value.is_empty() || value == "true" || value == "false" {
        is_allow_builds_key(key).then(|| {
            (
                key.to_string(),
                if value == "false" {
                    "false".to_string()
                } else {
                    "true".to_string()
                },
            )
        })
    } else {
        None
    }
}

fn quote_yaml_key(key: &str) -> String {
    let needs_quotes = key.starts_with([
        '@', '-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%',
        '`',
    ]) || key.contains(": ")
        || key.ends_with(':');
    if needs_quotes {
        format!("'{}'", key.replace('\'', "''"))
    } else {
        key.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_managed_add_and_remove_commands() {
        let add = parse_plugin_command("dsh plugin --profile web add github:owner/plugin").unwrap();
        assert_eq!(add.action, PluginAction::Add);
        assert_eq!(add.profile, "web");
        assert_eq!(add.source, "github:owner/plugin");

        let remove =
            parse_plugin_command("dsh plugin --profile personal remove npm:@dsh/theme-paper")
                .unwrap();
        assert_eq!(remove.action, PluginAction::Remove);
    }

    #[test]
    fn rejects_shell_syntax_and_extra_arguments() {
        for command in [
            "dsh plugin --profile web add github:owner/plugin && whoami",
            "dsh plugin --profile web add 'github:owner/plugin'",
            "dsh plugin --profile web --force add github:owner/plugin",
            "dsh plugin --profile ../web add github:owner/plugin",
        ] {
            assert!(matches!(
                parse_plugin_command(command),
                Err(LauncherError::DshPlugin(_))
            ));
        }
    }

    #[test]
    fn output_summary_keeps_the_pnpm_build_error() {
        let stdout = b"progress The git-hosted package \"dsh-lumina-tarot@0.1.0\" needs to execute build scripts but is not in the allowBuilds allowlist.";
        let stderr = b"dsh: pnpm failed in profile directory /tmp/web";
        let summary = output_summary(stdout, stderr);
        assert!(summary.contains("dsh-lumina-tarot@0.1.0"));
        assert!(summary.contains("needs to execute build scripts"));
    }

    #[test]
    fn blocked_build_keys_include_git_and_ignored_packages() {
        let output = r#"ERR_PNPM_GIT_DEP_PREPARE_NOT_ALLOWED The git-hosted package "dsh-lumina-tarot@0.1.0" needs to execute build scripts https://codeload.github.com/shetengteng/dsh-lumina-tarot/tar.gz/0123456789abcdef0123456789abcdef01234567 Ignored build scripts: esbuild."#;
        let keys = blocked_build_keys(output, "github:shetengteng/dsh-lumina-tarot");
        assert!(keys.contains(&"dsh-lumina-tarot".to_string()));
        assert!(keys.contains(&"esbuild".to_string()));
        assert!(!keys.iter().any(|key| key.contains('@')));
    }

    #[test]
    fn unrelated_git_failures_do_not_write_allow_builds() {
        let output = "dsh: pnpm failed in profile directory /tmp/web dsh: git-hosted plugins build on install via their prepare script, which pnpm blocks until allowed — add the exact key pnpm printed above under allowBuilds";
        assert!(blocked_build_keys(output, "github:owner/plugin").is_empty());
    }

    #[test]
    fn merge_allow_builds_appends_a_block_and_quotes_scoped_names() {
        let yaml = merge_allow_builds(
            "packages:\n  - .\n",
            &[
                "dsh-lumina-tarot".to_string(),
                "@scope/pkg".to_string(),
                "dsh-lumina-tarot@git+https://github.com/shetengteng/dsh-lumina-tarot.git"
                    .to_string(),
            ],
        );
        assert!(yaml.contains("allowBuilds:\n"));
        assert!(yaml.contains("  dsh-lumina-tarot: true\n"));
        assert!(yaml.contains("  '@scope/pkg': true\n"));
        assert!(!yaml.contains("git+https://"));
    }

    #[test]
    fn merge_allow_builds_drops_git_union_keys() {
        let yaml = "allowBuilds:\n  dsh-lumina-tarot: true\n  dsh-lumina-tarot@git+https://github.com/shetengteng/dsh-lumina-tarot.git: true\n";
        let merged = merge_allow_builds(yaml, &[]);
        assert!(merged.contains("  dsh-lumina-tarot: true\n"));
        assert!(!merged.contains("git+https://"));
    }

    #[test]
    fn merge_allow_builds_repairs_duplicate_blocks() {
        let yaml =
            "packages:\n  - .\nallowBuilds:\n  esbuild: true\nallowBuilds:\n  leftover: true\n";
        let merged = merge_allow_builds(yaml, &["dsh-lumina-tarot".to_string()]);
        assert_eq!(merged.matches("allowBuilds:").count(), 1);
        assert!(merged.contains("  esbuild: true\n"));
        assert!(merged.contains("  leftover: true\n"));
        assert!(merged.contains("  dsh-lumina-tarot: true\n"));
    }

    #[test]
    fn lists_only_dependency_managed_bundles() {
        let manifest: ProfileManifest = serde_json::from_str(
            r#"{
              "dsh": {
                "profile": {
                  "bundles": [
                    "@deepseek-ai/dsh-base",
                    "@deepseek-ai/dsh-web-app",
                    "dshmarket",
                    "dsh-lumina-tarot"
                  ]
                }
              },
              "dependencies": {
                "dsh-lumina-tarot": "link:/tmp/dsh-lumina-tarot",
                "dshmarket": "1.31.1",
                "plain-lib": "2.0.0"
              }
            }"#,
        )
        .unwrap();

        assert_eq!(
            installed_plugins(&manifest),
            vec![
                InstalledPlugin {
                    name: "dsh-lumina-tarot".to_string(),
                    spec: "link:/tmp/dsh-lumina-tarot".to_string(),
                },
                InstalledPlugin {
                    name: "dshmarket".to_string(),
                    spec: "1.31.1".to_string(),
                },
            ]
        );
    }

    #[test]
    fn missing_profile_manifest_is_an_empty_list() {
        let temp = tempfile::tempdir().unwrap();
        let listed = list_profile_plugins_in(temp.path(), "web").unwrap();
        assert_eq!(listed.profile, "web");
        assert!(listed.plugins.is_empty());
    }

    #[test]
    fn reads_installed_plugins_from_the_profile_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let profile_dir = temp.path().join("profiles").join("web");
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::write(
            profile_dir.join("package.json"),
            r#"{
              "dsh": { "profile": { "bundles": ["dshmarket"] } },
              "dependencies": { "dshmarket": "1.31.1" }
            }"#,
        )
        .unwrap();

        let listed = list_profile_plugins_in(temp.path(), "web").unwrap();
        assert_eq!(
            listed.plugins,
            vec![InstalledPlugin {
                name: "dshmarket".to_string(),
                spec: "1.31.1".to_string(),
            }]
        );
    }
}
