//! Command execution tool with policy enforcement.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

/// Tool for executing shell commands.
pub struct CommandExecTool;

impl Tool for CommandExecTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "cmd.exec".into(),
            description: concat!(
                "Execute a command. The 'cmd' field is the binary name (e.g. 'mkdir', 'bun', 'git'). ",
                "The 'args' field is an array of string arguments. ",
                "Optionally pass 'workdir' (relative to workspace root) to run in a subdirectory. ",
                "Alternatively you can pass a single 'command' string and it will be run via the system shell."
            ).into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "cmd": {"type": "string", "description": "Binary name (e.g. 'mkdir', 'bun', 'node')"},
                    "args": {"type": "array", "items": {"type": "string"}, "description": "Arguments array"},
                    "command": {"type": "string", "description": "Alternative: full shell command string"},
                    "workdir": {"type": "string", "description": "Optional relative working directory (e.g. 'frontend'). Avoid using shell 'cd'."}
                }
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let command_cwd = normalize_workdir(cwd, input.get("workdir").and_then(|v| v.as_str()));

        match policy.evaluate_path(&command_cwd) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }

        if !command_cwd.exists() || !command_cwd.is_dir() {
            return Err(ToolError::InvalidInput(format!(
                "workdir does not exist or is not a directory: {}",
                command_cwd.to_string_lossy()
            )));
        }

        let command_field = input
            .get("command")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        // Get explicit args if provided
        let explicit_args: Option<Vec<String>> =
            input.get("args").and_then(|v| v.as_array()).map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            });

        // Try to get cmd from the input, with fallback to "command" key, then to args[0]
        let raw_cmd = input
            .get("cmd")
            .and_then(|v| v.as_str())
            .or_else(|| input.get("command").and_then(|v| v.as_str()))
            .or_else(|| {
                explicit_args
                    .as_ref()
                    .and_then(|items| items.first().map(|s| s.as_str()))
            })
            .ok_or_else(|| ToolError::InvalidInput("cmd required".into()))?;

        // If cmd contains spaces and no explicit args, split it into binary + args
        let (binary, mut args) = if explicit_args.is_some() {
            (raw_cmd.to_string(), explicit_args.unwrap())
        } else if raw_cmd.contains(' ') {
            let parts: Vec<&str> = raw_cmd.split_whitespace().collect();
            let bin = parts[0].to_string();
            let rest: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
            (bin, rest)
        } else {
            (raw_cmd.to_string(), Vec::new())
        };

        // Common LLM recovery: args accidentally include the binary as first item
        if args.first().map(|v| v == &binary).unwrap_or(false) {
            let _ = args.remove(0);
        }

        // Handle cd command for policy checking
        if binary.eq_ignore_ascii_case("cd") {
            if let Some(target) = resolve_cd_target(&command_cwd, command_field.as_deref(), &args) {
                match policy.evaluate_path(&target) {
                    PolicyDecision::Allow => {}
                    PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
                    PolicyDecision::NeedsApproval { scope, reason } => {
                        return Err(ToolError::ApprovalRequired { scope, reason })
                    }
                }
            }
        }

        // Policy check on the binary name
        match policy.evaluate_command(&binary) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { reason, .. } => {
                return Err(ToolError::PolicyDenied(reason))
            }
        }

        let output = if let Some(command) = command_field.as_deref() {
            run_shell_command(&command_cwd, command)?
        } else {
            match Command::new(&binary)
                .args(&args)
                .current_dir(&command_cwd)
                .output()
            {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    #[cfg(target_os = "windows")]
                    {
                        let shell_command = if args.is_empty() {
                            binary.clone()
                        } else {
                            format!("{} {}", binary, args.join(" "))
                        };
                        run_shell_command(&command_cwd, &shell_command)?
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        return Err(ToolError::Execution(format!(
                            "program not found: {binary}. Try cmd.exec with the 'command' field for shell built-ins"
                        )));
                    }
                }
                Err(error) => return Err(ToolError::Execution(error.to_string())),
            }
        };

        Ok(ToolCallOutput {
            ok: output.status.success(),
            data: serde_json::json!({
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
                "code": output.status.code(),
                "workdir": command_cwd,
                "invoked": if let Some(command) = command_field {
                    serde_json::json!({"mode": "shell", "command": command})
                } else {
                    serde_json::json!({"mode": "binary", "cmd": binary, "args": args})
                },
            }),
            error: None,
        })
    }
}

/// Translates common Unix shell commands to Windows equivalents.
/// This helps when the LLM generates Unix commands on Windows.
#[cfg(target_os = "windows")]
pub fn translate_unix_to_windows(command: &str) -> String {
    let trimmed = command.trim();

    // Handle "which" command -> "where"
    if trimmed.starts_with("which ") {
        return trimmed.replacen("which ", "where ", 1);
    }

    // Handle "command -v" (bash built-in for checking command existence) -> "where"
    if trimmed.starts_with("command -v ") {
        return trimmed.replacen("command -v ", "where ", 1);
    }

    // Handle "rm -rf" -> rmdir /s /q
    if trimmed.starts_with("rm") {
        let after_rm = &trimmed[2..].trim_start();
        if after_rm.starts_with("-rf ") || after_rm.starts_with("-r ") {
            let after_flags = after_rm[after_rm.find(' ').unwrap_or(0)..].trim_start();
            return format!("rmdir /s /q {}", after_flags);
        }
        // Single file rm (not -rf, -r, etc.)
        if !after_rm.starts_with('-') {
            let path = after_rm.trim();
            return format!("del /q {}", path);
        }
    }

    // Handle "rm " (single file) -> del
    if trimmed.starts_with("rm ") && !trimmed.contains(" -") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            return format!("del /q {}", parts[1]);
        }
    }

    // Handle "mkdir -p" -> mkdir (Windows mkdir doesn't need -p)
    if trimmed.starts_with("mkdir -p ") {
        let after_flag = &trimmed["mkdir -p ".len()..].trim_start();
        if after_flag.is_empty() {
            return "mkdir".to_string();
        }
        return format!("mkdir {}", after_flag);
    }

    // Handle "cp -r" or "cp -a" -> xcopy /e /i /h
    if trimmed.starts_with("cp -r ")
        || trimmed.starts_with("cp -a ")
        || trimmed.starts_with("cp -R ")
    {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            let src = parts[parts.len() - 2];
            let dst = parts[parts.len() - 1];
            return format!("xcopy /e /i /h {} {}", src, dst);
        }
    }

    // Handle "cp " (single file) -> copy
    if trimmed.starts_with("cp ") && !trimmed.contains(" -") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            let src = parts[1];
            let dst = parts[2];
            return format!("copy {} {}", src, dst);
        }
    }

    // Handle "mv " -> move
    if trimmed.starts_with("mv ") && !trimmed.starts_with("mv -") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            let src = parts[1];
            let dst = parts[2];
            return format!("move {} {}", src, dst);
        }
    }

    // Handle "touch " -> type nul >
    if trimmed.starts_with("touch ") {
        let path = trimmed.split_whitespace().nth(1).unwrap_or("");
        return format!("type nul > {}", path);
    }

    // Handle "cat " -> type
    if trimmed.starts_with("cat ") {
        let path = trimmed.split_whitespace().nth(1).unwrap_or("");
        return format!("type {}", path);
    }

    // Handle "ls " -> dir
    if trimmed.starts_with("ls ") || trimmed == "ls" {
        return "dir".to_string();
    }

    // Handle "tree" -> use ASCII-only output via dir /s /b
    if trimmed == "tree" || trimmed.starts_with("tree ") {
        return "dir /s /b".to_string();
    }

    // Handle "cd path && command" by stripping the cd part
    if trimmed.starts_with("cd ") && trimmed.contains(" && ") {
        if let Some(pos) = trimmed.find(" && ") {
            return trimmed[pos + 4..].to_string();
        }
    }

    command.to_string()
}

fn run_shell_command(cwd: &Path, command: &str) -> Result<std::process::Output, ToolError> {
    #[cfg(target_os = "windows")]
    {
        let translated = translate_unix_to_windows(command);
        let utf8_command = format!("chcp 65001 >nul 2>&1 && {}", translated);
        Command::new("cmd")
            .args(["/C", &utf8_command])
            .current_dir(cwd)
            .output()
            .map_err(|e| ToolError::Execution(e.to_string()))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("sh")
            .args(["-lc", command])
            .current_dir(cwd)
            .output()
            .map_err(|e| ToolError::Execution(e.to_string()))
    }
}

fn resolve_cd_target(cwd: &Path, command_field: Option<&str>, args: &[String]) -> Option<PathBuf> {
    if let Some(command) = command_field {
        if let Some(raw_target) = parse_cd_target_from_shell_command(command) {
            return Some(resolve_path_from_cd_arg(cwd, &raw_target));
        }
    }

    let raw_target = args.first()?.as_str();
    if raw_target.eq_ignore_ascii_case("/d") {
        let second = args.get(1)?;
        return Some(resolve_path_from_cd_arg(cwd, second));
    }
    Some(resolve_path_from_cd_arg(cwd, raw_target))
}

fn parse_cd_target_from_shell_command(command: &str) -> Option<String> {
    let trimmed = command.trim_start();
    if !trimmed.to_ascii_lowercase().starts_with("cd") {
        return None;
    }

    let first_segment = trimmed
        .split("&&")
        .next()
        .unwrap_or(trimmed)
        .split(';')
        .next()
        .unwrap_or(trimmed)
        .trim();

    if first_segment.len() < 2 {
        return None;
    }

    let rest_trimmed = first_segment[2..].trim_start().to_string();
    if rest_trimmed.is_empty() {
        return None;
    }

    let rest = if rest_trimmed.to_ascii_lowercase().starts_with("/d") {
        let after_d = rest_trimmed[2..].trim_start();
        if after_d.is_empty() {
            return None;
        }
        after_d.to_string()
    } else {
        rest_trimmed
    };

    let rest = rest.as_str();

    if rest.starts_with('"') {
        let closing = rest[1..].find('"')? + 1;
        return Some(rest[1..closing].to_string());
    }

    if rest.starts_with('\'') {
        let closing = rest[1..].find('\'')? + 1;
        return Some(rest[1..closing].to_string());
    }

    let token = rest.split_whitespace().next()?;
    Some(token.to_string())
}

fn resolve_path_from_cd_arg(cwd: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        candidate
    } else {
        cwd.join(candidate)
    }
}

fn normalize_workdir(cwd: &Path, workdir: Option<&str>) -> PathBuf {
    match workdir {
        Some(wd) if !wd.is_empty() => cwd.join(wd),
        _ => cwd.to_path_buf(),
    }
}
