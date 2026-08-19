use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::process::Command;

use crate::error::{LauncherError, Result};

const MAX_COMMAND_LENGTH: usize = 2_048;
const MAX_PROFILE_LENGTH: usize = 64;
const MAX_SOURCE_LENGTH: usize = 1_024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(90);

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
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        child.creation_flags(CREATE_NO_WINDOW);
    }

    let output = tokio::time::timeout(COMMAND_TIMEOUT, child.output())
        .await
        .map_err(|_| LauncherError::DshPlugin("command timed out after 90 seconds".to_string()))?
        .map_err(|error| {
            LauncherError::DshPlugin(format!("failed to start managed dsh: {error}"))
        })?;
    let summary = output_summary(&output.stdout, &output.stderr);
    if !output.status.success() {
        tracing::warn!(
            action = ?parsed.action,
            profile = %parsed.profile,
            source = %parsed.source,
            exit_code = ?output.status.code(),
            summary = %summary,
            "dsh plugin command failed"
        );
        return Err(LauncherError::DshPlugin(format!(
            "dsh plugin command exited with code {:?}: {summary}",
            output.status.code()
        )));
    }

    Ok(PluginCommandResult {
        action: parsed.action,
        profile: parsed.profile,
        source: parsed.source,
        summary,
    })
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

fn output_summary(stdout: &[u8], stderr: &[u8]) -> String {
    let output = if stderr.is_empty() { stdout } else { stderr };
    let text = String::from_utf8_lossy(output)
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect::<String>();
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        "操作已完成。".to_string()
    } else {
        compact.chars().take(280).collect()
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
    fn output_summary_prefers_stderr_and_removes_control_characters() {
        assert_eq!(
            output_summary(b"done", b"\x1b[31mfailed\nplease retry"),
            "[31mfailed please retry"
        );
    }
}
