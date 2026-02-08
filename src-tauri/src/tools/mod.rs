use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::core::skills::{
    add_custom_skill, import_context7_skill, import_vercel_skill, list_all_skills,
    remove_custom_skill, NewCustomSkill,
};
use crate::core::tool::ToolDescriptor;
use crate::policy::{PolicyDecision, PolicyEngine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInput {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallOutput {
    pub ok: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("policy denied: {0}")]
    PolicyDenied(String),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("approval required for scope '{scope}': {reason}")]
    ApprovalRequired { scope: String, reason: String },
}

pub trait Tool: Send + Sync {
    fn descriptor(&self) -> ToolDescriptor;
    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError>;
}

pub struct ToolRegistry {
    tools: std::collections::HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn default() -> Self {
        let mut tools: std::collections::HashMap<String, Box<dyn Tool>> =
            std::collections::HashMap::new();

        tools.insert("fs.read".to_string(), Box::new(FsReadTool));
        tools.insert("fs.write".to_string(), Box::new(FsWriteTool));
        tools.insert("fs.list".to_string(), Box::new(FsListTool));
        tools.insert("search.rg".to_string(), Box::new(SearchRgTool));
        tools.insert("cmd.exec".to_string(), Box::new(CommandExecTool));
        tools.insert("git.status".to_string(), Box::new(GitStatusTool));
        tools.insert("git.diff".to_string(), Box::new(GitDiffTool));
        tools.insert("git.apply_patch".to_string(), Box::new(GitApplyPatchTool));
        tools.insert("git.commit".to_string(), Box::new(GitCommitTool));
        tools.insert("git.log".to_string(), Box::new(GitLogTool));
        tools.insert("skills.list".to_string(), Box::new(SkillsListTool));
        tools.insert("agent.todo".to_string(), Box::new(AgentTodoTool));
        tools.insert("subagent.spawn".to_string(), Box::new(SubAgentSpawnTool));
        tools.insert("skills.load".to_string(), Box::new(SkillsLoadTool));
        tools.insert("skills.remove".to_string(), Box::new(SkillsRemoveTool));

        Self { tools }
    }

    pub fn list(&self) -> Vec<ToolDescriptor> {
        self.tools.values().map(|t| t.descriptor()).collect()
    }

    /// Generate a detailed tool reference string for inclusion in LLM prompts.
    /// Includes tool name, description, and input schema so the LLM knows
    /// exactly what arguments each tool expects.
    pub fn tool_reference_for_prompt(&self) -> String {
        let mut tools: Vec<_> = self.tools.values().map(|t| t.descriptor()).collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));

        let mut out = String::new();
        for tool in &tools {
            out.push_str(&format!("### {}\n", tool.name));
            out.push_str(&format!("{}\n", tool.description));
            out.push_str(&format!(
                "Input schema: {}\n\n",
                serde_json::to_string(&tool.input_schema).unwrap_or_else(|_| "{}".to_string())
            ));
        }
        out
    }

    pub fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        call: ToolCallInput,
    ) -> Result<ToolCallOutput, ToolError> {
        let Some(tool) = self.tools.get(&call.name) else {
            return Err(ToolError::InvalidInput(format!(
                "unknown tool: {}",
                call.name
            )));
        };
        tool.invoke(policy, cwd, call.args)
    }
}

struct FsReadTool;
struct FsWriteTool;
struct FsListTool;
struct SearchRgTool;
struct CommandExecTool;
struct GitStatusTool;
struct GitDiffTool;
struct GitApplyPatchTool;
struct GitCommitTool;
struct GitLogTool;
struct SkillsListTool;
struct AgentTodoTool;
struct SubAgentSpawnTool;
struct SkillsLoadTool;
struct SkillsRemoveTool;

impl Tool for FsReadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "fs.read".into(),
            description: "Read file contents".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("path required".into()))?;
        let full = cwd.join(path);
        match policy.evaluate_path(&full) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }
        let content =
            std::fs::read_to_string(&full).map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({"path": full, "content": content}),
            error: None,
        })
    }
}

impl Tool for FsWriteTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "fs.write".into(),
            description: "Write file contents".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("path required".into()))?;
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("content required".into()))?;
        let full = cwd.join(path);
        // Check policy BEFORE creating directories to avoid OS errors on denied paths
        match policy.evaluate_path(&full) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::Execution(e.to_string()))?;
        }
        std::fs::write(&full, content).map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({"path": full}),
            error: None,
        })
    }
}

impl Tool for FsListTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "fs.list".into(),
            description: "List directory contents without shell commands. Supports recursion, depth limit, and entry limit.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory path relative to workspace root (default: .)"},
                    "recursive": {"type": "boolean", "description": "If true, walk subdirectories recursively"},
                    "max_depth": {"type": "integer", "minimum": 0, "description": "Max depth when recursive=true (0 means only the target directory)"},
                    "limit": {"type": "integer", "minimum": 1, "description": "Max number of entries to return (default: 200)"},
                    "files_only": {"type": "boolean", "description": "If true, only include files"},
                    "dirs_only": {"type": "boolean", "description": "If true, only include directories"}
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
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let recursive = input
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let max_depth = input.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(200)
            .clamp(1, 2000) as usize;
        let files_only = input
            .get("files_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let dirs_only = input
            .get("dirs_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if files_only && dirs_only {
            return Err(ToolError::InvalidInput(
                "files_only and dirs_only cannot both be true".to_string(),
            ));
        }

        let full = cwd.join(path);
        match policy.evaluate_path(&full) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }

        if !full.exists() {
            return Err(ToolError::Execution(format!(
                "directory does not exist: {}",
                full.to_string_lossy()
            )));
        }
        if !full.is_dir() {
            return Err(ToolError::Execution(format!(
                "path is not a directory: {}",
                full.to_string_lossy()
            )));
        }

        let mut entries: Vec<serde_json::Value> = Vec::new();
        let mut stack: Vec<(PathBuf, usize)> = vec![(full.clone(), 0)];
        let mut truncated = false;

        while let Some((dir, depth)) = stack.pop() {
            let read_dir =
                std::fs::read_dir(&dir).map_err(|e| ToolError::Execution(e.to_string()))?;
            for item in read_dir {
                let item = item.map_err(|e| ToolError::Execution(e.to_string()))?;
                let item_path = item.path();

                let metadata = item
                    .metadata()
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                let is_dir = metadata.is_dir();
                let is_file = metadata.is_file();

                if (files_only && !is_file) || (dirs_only && !is_dir) {
                    if recursive && is_dir && depth < max_depth {
                        stack.push((item_path, depth + 1));
                    }
                    continue;
                }

                let rel_path = item_path
                    .strip_prefix(cwd)
                    .unwrap_or(&item_path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let modified_unix = metadata
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());

                entries.push(serde_json::json!({
                    "name": item.file_name().to_string_lossy(),
                    "path": rel_path,
                    "is_dir": is_dir,
                    "is_file": is_file,
                    "size": metadata.len(),
                    "modified_unix": modified_unix,
                    "depth": depth,
                }));

                if entries.len() >= limit {
                    truncated = true;
                    break;
                }

                if recursive && is_dir && depth < max_depth {
                    stack.push((item_path, depth + 1));
                }
            }

            if truncated {
                break;
            }
        }

        entries.sort_by(|a, b| {
            let a_path = a.get("path").and_then(|v| v.as_str()).unwrap_or_default();
            let b_path = b.get("path").and_then(|v| v.as_str()).unwrap_or_default();
            a_path.cmp(b_path)
        });

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({
                "path": full,
                "recursive": recursive,
                "max_depth": max_depth,
                "limit": limit,
                "count": entries.len(),
                "truncated": truncated,
                "entries": entries,
            }),
            error: None,
        })
    }
}

impl Tool for SearchRgTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "search.rg".into(),
            description: "Search with ripgrep".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("pattern required".into()))?;
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let full = cwd.join(path);
        match policy.evaluate_path(&full) {
            PolicyDecision::Allow => {}
            PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
            PolicyDecision::NeedsApproval { scope, reason } => {
                return Err(ToolError::ApprovalRequired { scope, reason })
            }
        }
        let output = Command::new("rg")
            .arg(pattern)
            .arg(&full)
            .output()
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(ToolCallOutput {
            ok: output.status.success(),
            data: serde_json::json!({
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
            }),
            error: None,
        })
    }
}

impl Tool for CommandExecTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "cmd.exec".into(),
            description: "Execute a command. The 'cmd' field is the binary name (e.g. 'mkdir', 'bun', 'git'). The 'args' field is an array of string arguments. Optionally pass 'workdir' (relative to workspace root) to run in a subdirectory. Alternatively you can pass a single 'command' string and it will be run via the system shell.".into(),
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

        // Common LLM recovery: args accidentally include the binary as first item.
        if args.first().map(|v| v == &binary).unwrap_or(false) {
            let _ = args.remove(0);
        }

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
            run_shell_command(&command_cwd, &command)?
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
                    serde_json::json!({"mode":"shell","command":command})
                } else {
                    serde_json::json!({"mode":"binary","cmd":binary,"args":args})
                },
            }),
            error: None,
        })
    }
}

fn run_shell_command(cwd: &Path, command: &str) -> Result<std::process::Output, ToolError> {
    #[cfg(target_os = "windows")]
    {
        return Command::new("cmd")
            .args(["/C", command])
            .current_dir(cwd)
            .output()
            .map_err(|e| ToolError::Execution(e.to_string()));
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

    let mut rest = first_segment[2..].trim_start();
    if rest.is_empty() {
        return None;
    }

    if rest.to_ascii_lowercase().starts_with("/d") {
        rest = rest[2..].trim_start();
        if rest.is_empty() {
            return None;
        }
    }

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

impl Tool for GitStatusTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "git.status".into(),
            description: "Run git status --short. Shows modified, added, and deleted files in the current worktree.".into(),
            input_schema: serde_json::json!({"type":"object"}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        _: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        run_git(policy, cwd, &["status", "--short"])
    }
}

impl Tool for GitDiffTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "git.diff".into(),
            description: "Run git diff. Shows unstaged changes in the current worktree. Pass {\"staged\": true} to see staged changes.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "staged": {"type": "boolean", "description": "If true, show staged (cached) changes instead of unstaged"}
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
        let staged = input
            .get("staged")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if staged {
            run_git(policy, cwd, &["diff", "--cached"])
        } else {
            run_git(policy, cwd, &["diff"])
        }
    }
}

impl Tool for GitApplyPatchTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "git.apply_patch".into(),
            description: "Apply patch via git apply".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"patch":{"type":"string"}},"required":["patch"]}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let patch = input
            .get("patch")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("patch required".into()))?;

        let patch_path = cwd.join(".orchestrix").join("patch.diff");
        if let Some(parent) = patch_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::Execution(e.to_string()))?;
        }
        std::fs::write(&patch_path, patch).map_err(|e| ToolError::Execution(e.to_string()))?;

        run_git(policy, cwd, &["apply", &patch_path.to_string_lossy()])
    }
}

impl Tool for GitCommitTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "git.commit".into(),
            description: "Stage all changes and commit in the current worktree. This is useful inside agent worktrees to checkpoint progress.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string", "description": "Commit message"}
                },
                "required": ["message"]
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
        let message = input
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("message required".into()))?;

        // Stage all changes first.
        run_git(policy, cwd, &["add", "-A"])?;

        // Commit with the provided message.
        let output = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .arg("commit")
            .arg("-m")
            .arg(message)
            .env("GIT_AUTHOR_NAME", "Orchestrix")
            .env("GIT_AUTHOR_EMAIL", "orchestrix@local")
            .env("GIT_COMMITTER_NAME", "Orchestrix")
            .env("GIT_COMMITTER_EMAIL", "orchestrix@local")
            .output()
            .map_err(|e| ToolError::Execution(format!("git commit failed: {e}")))?;

        Ok(ToolCallOutput {
            ok: output.status.success(),
            data: serde_json::json!({
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
                "code": output.status.code(),
            }),
            error: if output.status.success() {
                None
            } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            },
        })
    }
}

impl Tool for GitLogTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "git.log".into(),
            description: "Show recent git log entries. Defaults to 10 entries in oneline format."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "count": {"type": "integer", "description": "Number of log entries to show (default: 10)"}
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
        let count = input.get("count").and_then(|v| v.as_u64()).unwrap_or(10);
        let count_str = format!("-{count}");
        run_git(policy, cwd, &["log", "--oneline", &count_str])
    }
}

impl Tool for SkillsListTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.list".into(),
            description: "List all available skills (builtin + custom + imported).".into(),
            input_schema: serde_json::json!({"type":"object"}),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({"skills": list_all_skills()}),
            error: None,
        })
    }
}

impl Tool for AgentTodoTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "agent.todo".into(),
            description:
                "Manage the agent's local todo list. Actions: list, set, add, update, clear.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["list", "set", "add", "update", "clear"]},
                    "todos": {"type": "array", "items": {"type": "object"}},
                    "item": {"type": "object"},
                    "index": {"type": "integer"}
                }
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");
        let state_dir = cwd.join(".orchestrix");
        std::fs::create_dir_all(&state_dir).map_err(|e| ToolError::Execution(e.to_string()))?;
        let todo_path = state_dir.join("agent-todo.json");

        let mut todos: Vec<serde_json::Value> = if todo_path.exists() {
            let raw = std::fs::read_to_string(&todo_path)
                .map_err(|e| ToolError::Execution(e.to_string()))?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            Vec::new()
        };

        match action {
            "set" => {
                let next = input
                    .get("todos")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| {
                        ToolError::InvalidInput("todos array is required for set".to_string())
                    })?;
                todos = next.clone();
            }
            "add" => {
                let item = input.get("item").ok_or_else(|| {
                    ToolError::InvalidInput("item is required for add".to_string())
                })?;
                todos.push(item.clone());
            }
            "update" => {
                let idx = input.get("index").and_then(|v| v.as_u64()).ok_or_else(|| {
                    ToolError::InvalidInput("index is required for update".to_string())
                })? as usize;
                let item = input.get("item").ok_or_else(|| {
                    ToolError::InvalidInput("item is required for update".to_string())
                })?;
                if idx >= todos.len() {
                    return Err(ToolError::InvalidInput("index out of range".to_string()));
                }
                todos[idx] = item.clone();
            }
            "clear" => {
                todos.clear();
            }
            "list" => {}
            _ => return Err(ToolError::InvalidInput(format!("unknown action: {action}"))),
        }

        if action != "list" {
            std::fs::write(
                &todo_path,
                serde_json::to_string_pretty(&todos)
                    .map_err(|e| ToolError::Execution(e.to_string()))?,
            )
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        }

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({ "todos": todos }),
            error: None,
        })
    }
}

impl Tool for SkillsLoadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.load".into(),
            description: concat!(
                "Load/import a skill into the local custom catalog. ",
                "First call skills.list to see available skills. ",
                "Modes: 'context7' requires library_id; 'vercel' requires skill_name; ",
                "'custom' requires title, install_command, and url."
            )
            .into(),
            input_schema: serde_json::json!({
                "type":"object",
                "required": ["mode"],
                "properties":{
                    "mode":{"type":"string","enum":["custom","context7","vercel"],
                            "description":"Import mode. Use 'context7' for Context7 libraries, 'vercel' for Vercel agent-skills, 'custom' for manually-defined skills."},
                    "id":{"type":"string","description":"Optional custom ID"},
                    "title":{"type":"string","description":"Required for custom mode"},
                    "description":{"type":"string","description":"Skill description (custom mode)"},
                    "install_command":{"type":"string","description":"Required for custom mode. How to install the skill."},
                    "url":{"type":"string","description":"Required for custom mode. URL for the skill."},
                    "source":{"type":"string","description":"Optional source label (custom mode)"},
                    "tags":{"type":"array","items":{"type":"string"},"description":"Optional tags"},
                    "library_id":{"type":"string","description":"Required for context7 mode. The Context7 library ID."},
                    "skill_name":{"type":"string","description":"Required for vercel mode. The skill name in vercel-labs/agent-skills."}
                }
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let mode = input
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("custom")
            .to_ascii_lowercase();

        let loaded = match mode.as_str() {
            "context7" => {
                let library_id = input
                    .get("library_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidInput(
                            "library_id is required for context7 mode".to_string(),
                        )
                    })?;
                import_context7_skill(library_id, input.get("title").and_then(|v| v.as_str()))
                    .map_err(ToolError::Execution)?
            }
            "vercel" => {
                let skill_name = input
                    .get("skill_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidInput(
                            "skill_name is required for vercel mode".to_string(),
                        )
                    })?;
                import_vercel_skill(skill_name).map_err(ToolError::Execution)?
            }
            _ => {
                let title = input.get("title").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidInput("title is required for custom mode".to_string())
                })?;
                let description = input
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let install_command = input
                    .get("install_command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidInput(
                            "install_command is required for custom mode".to_string(),
                        )
                    })?;
                let url = input.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidInput("url is required for custom mode".to_string())
                })?;

                let tags = input.get("tags").and_then(|v| v.as_array()).map(|items| {
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                });

                add_custom_skill(NewCustomSkill {
                    id: input
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|v| v.to_string()),
                    title: title.to_string(),
                    description: description.to_string(),
                    install_command: install_command.to_string(),
                    url: url.to_string(),
                    source: input
                        .get("source")
                        .and_then(|v| v.as_str())
                        .map(|v| v.to_string()),
                    tags,
                })
                .map_err(ToolError::Execution)?
            }
        };

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({"skill": loaded}),
            error: None,
        })
    }
}

impl Tool for SubAgentSpawnTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "subagent.spawn".into(),
            description: "Delegate a focused objective to a child sub-agent. Use this instead of implicit delegation actions.".into(),
            input_schema: serde_json::json!({
                "type":"object",
                "properties":{
                    "objective":{"type":"string","description":"Focused delegated objective"},
                    "max_retries":{"type":"integer","description":"Optional retries for delegated objective"}
                },
                "required":["objective"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        _input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        Err(ToolError::Execution(
            "subagent.spawn is orchestrator-managed and cannot be invoked directly".to_string(),
        ))
    }
}

impl Tool for SkillsRemoveTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "skills.remove".into(),
            description: "Remove a custom loaded skill from the local catalog.".into(),
            input_schema: serde_json::json!({
                "type":"object",
                "properties":{"skill_id":{"type":"string"}},
                "required":["skill_id"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        _cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let skill_id = input
            .get("skill_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("skill_id is required".to_string()))?;
        let removed = remove_custom_skill(skill_id).map_err(ToolError::Execution)?;

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::json!({"removed": removed, "skill_id": skill_id}),
            error: None,
        })
    }
}

fn run_git(policy: &PolicyEngine, cwd: &Path, args: &[&str]) -> Result<ToolCallOutput, ToolError> {
    match policy.evaluate_command("git") {
        PolicyDecision::Allow => {}
        PolicyDecision::Deny(reason) => return Err(ToolError::PolicyDenied(reason)),
        PolicyDecision::NeedsApproval { reason, .. } => {
            return Err(ToolError::PolicyDenied(reason))
        }
    }

    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| ToolError::Execution(e.to_string()))?;

    Ok(ToolCallOutput {
        ok: output.status.success(),
        data: serde_json::json!({
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
            "code": output.status.code(),
        }),
        error: None,
    })
}

pub fn infer_tool_call(step_title: &str, tool_intent: Option<&str>) -> Option<ToolCallInput> {
    let intent = tool_intent.unwrap_or(step_title).to_ascii_lowercase();

    if intent.contains("git.status") || intent.contains("status") {
        return Some(ToolCallInput {
            name: "git.status".to_string(),
            args: serde_json::json!({}),
        });
    }
    if intent.contains("git.diff") || intent.contains("diff") {
        return Some(ToolCallInput {
            name: "git.diff".to_string(),
            args: serde_json::json!({}),
        });
    }
    if intent.contains("search") || intent.contains("rg") {
        return Some(ToolCallInput {
            name: "search.rg".to_string(),
            args: serde_json::json!({ "pattern": step_title, "path": "." }),
        });
    }

    None
}

pub fn normalize_workdir(base: &Path, candidate: Option<&str>) -> PathBuf {
    match candidate {
        Some(value) if !value.trim().is_empty() => base.join(value),
        _ => base.to_path_buf(),
    }
}
