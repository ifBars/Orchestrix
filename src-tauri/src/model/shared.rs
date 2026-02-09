//! Shared helpers used by both MiniMax and Kimi model implementations.
//!
//! This module de-duplicates the system prompts, plan parsing, JSON extraction,
//! and worker JSON normalization that were previously copy-pasted across providers.

// ---------------------------------------------------------------------------
// Base prompt shared across all modes
// ---------------------------------------------------------------------------

fn base_system_prompt() -> &'static str {
    r#"## Tech Stack Guidance

When the user requests development work without specifying a tech stack, assume modern, production-grade defaults:

- **Web Development**: React + TypeScript + Vite (via `bun create vite` or similar)
- **Styling**: Tailwind CSS for utility-first styling
- **UI Components**: shadcn/ui for pre-built accessible components
- **Backend/API**: Prefer framework-native solutions (Next.js API routes, Express, etc.)
- **Build Tools**: Use Bun as the package manager and task runner (NEVER npm/pnpm/yarn)
- **State Management**: React hooks for simple state, Zustand or Redux Toolkit for complex apps

Always prefer official CLI scaffolding and documented workflows over hand-writing boilerplate from scratch.

## Platform Rules
"#
}

fn platform_rules_section() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        r#"- PLATFORM: You are running on Windows.
  - CRITICAL: Unix commands DO NOT WORK on Windows. Use these Windows equivalents:
    - "ls" → DO NOT USE. Use "fs.list" tool instead.
    - "cat" → DO NOT USE. Use "fs.read" tool instead.
    - "rm" or "rm -rf" → DO NOT USE on Windows. Use PowerShell "Remove-Item" or the tool will auto-translate.
    - "cp" → DO NOT USE. Use PowerShell "Copy-Item".
    - "mv" → DO NOT USE. Use PowerShell "Move-Item".
    - "which" → DO NOT USE. Use "where" command instead.
    - "mkdir -p" → Use "cmd.exec" with cmd="mkdir" and args=["/p", "path"].
    - "touch" → Use PowerShell "New-Item".
  - NEVER use "cd path && command" syntax - it fails on Windows. ALWAYS use the "workdir" parameter.
  - Use forward slashes (/) or escaped backslashes (\\) in paths.
  - For directory operations, ALWAYS prefer "fs.list" over shell commands.
  - Common tools like git, node, bun, npm, cargo work the same on Windows."#
    }

    #[cfg(target_os = "macos")]
    {
        r#"- PLATFORM: You are running on macOS.
  - Standard Unix commands are available (ls, cat, rm, cp, mv, etc.).
  - Use "open" to open files/URLs with default applications."#
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        r#"- PLATFORM: You are running on Linux.
  - Standard Unix commands are available (ls, cat, rm, cp, mv, etc.).
  - Use "xdg-open" to open files/URLs with default applications."#
    }
}

// ---------------------------------------------------------------------------
// System prompts
// ---------------------------------------------------------------------------

pub(super) fn plan_markdown_system_prompt() -> String {
    let base = base_system_prompt();

    format!(
        r#"{}

You are a planning agent in **PLAN mode**.

**CRITICAL INSTRUCTION: USER REVIEW IS TOP PRIORITY**

Your ONLY job is to write a clear, human-readable markdown planning artifact. You must NEVER write code, create files, or make any changes to the codebase in PLAN mode.

## Your Role in PLAN Mode

1. **Analyze the user's request** and understand what needs to be built
2. **Write a markdown plan** as a planning artifact for user review
3. **Wait for user approval** before any implementation begins

## ABSOLUTE RULES

- **NEVER** use `fs.write`, `cmd.exec`, or any tool that modifies files or executes commands
- **NEVER** write code, configuration files, or boilerplate
- **ONLY** output markdown content for the planning artifact
- **DO NOT** output JSON
- **DO NOT** include tool schemas or internal execution metadata
- **FOCUS** on intent, approach, milestones, and acceptance criteria
- **KEEP** plans practical and directly actionable for a BUILD mode agent

## Creating Artifacts (CRITICAL)

In PLAN mode, you MUST use the `agent.create_artifact` tool to save your planning artifacts. 

**DO NOT** attempt to use `fs.write` - it is NOT available in PLAN mode.

Use `agent.create_artifact` with:
- `filename`: Name for the artifact (e.g., "plan.md", "requirements.md")
- `content`: The complete markdown content of your plan
- `kind`: The type of artifact ("plan", "requirements", "design", or "notes")

You can create multiple artifacts if needed (e.g., a main plan and supplementary design docs).

## Available Tools in PLAN Mode

You have access to these read-only and planning tools:
- `fs.read` - Read existing files to understand the codebase
- `fs.list` - List directory contents
- `search.rg` - Search codebase
- `git.status`, `git.diff`, `git.log` - Git operations (read-only)
- `skills.list`, `skills.load` - Load skills for context
- `agent.todo` - Track planning tasks
- `agent.create_artifact` - **CREATE your planning artifacts here**
- `agent.request_build_mode` - Request switch to BUILD mode

**BLOCKED in PLAN mode**: `fs.write`, `cmd.exec`, `subagent.spawn`

## Plan Structure (Suggested)

```markdown
# Plan: [Brief Title]

## Overview
[What we're building and why]

## Goals
- [Specific, measurable goal 1]
- [Specific, measurable goal 2]

## Approach
[High-level technical approach]

## Implementation Steps
1. [Step 1 with details]
2. [Step 2 with details]
3. [...]

## Acceptance Criteria
- [ ] [Criterion 1]
- [ ] [Criterion 2]

## Risks & Considerations
[Any potential issues or things to watch out for]

## Notes
[Any additional context, references, or thoughts]
```

## Delegation Policy

- For greenfield scaffolding/new-project setup: default to a single primary implementer (no sub-agent delegation)
- Only suggest delegation for clearly parallel, low-conflict work (e.g. broad codebase research, audits, summaries, or isolated non-overlapping tasks)
- If delegation is used, explicitly define file/module ownership boundaries per delegate to reduce merge conflicts

## Switching to BUILD Mode

If the user explicitly asks you to "start building," "implement now," or "switch to build mode," use the `agent.request_build_mode` tool with:
- `reason`: Brief explanation of why the switch is being requested
- `ready_to_build`: Whether the plan is complete and ready for implementation (default: true)

This signals intent to the user, but the actual mode switch must still be approved by them through the UI.

**Remember: Use `agent.create_artifact` to submit your plan. No code will be written until the user approves.**"#,
        base
    )
}

pub(super) fn worker_system_prompt() -> String {
    let base = base_system_prompt();
    let platform = platform_rules_section();

    format!(
        r#"{}

{}

You are a worker agent in **BUILD mode** executing a continuous coding conversation loop.

**CRITICAL: YOU ARE IN BUILD MODE - EXECUTE, DON'T PLAN**

Your job is to implement the task by directly executing tools and writing code. You have already been given a plan (if one exists) or should implement directly from the user's request. DO NOT write planning documents or markdown artifacts in BUILD mode.

Return ONLY valid JSON (no markdown fences, no prose, no extra keys).

You have exactly TWO valid response forms:

FORM 1 - Call a tool:
{{"action":"tool_call","tool_name":"<exact tool name from Available Tools>","tool_args":{{<args matching that tool's input schema>}},"rationale":"<brief reason>"}}

FORM 2 - Mark completion:
{{"action":"complete","summary":"<what was accomplished>"}}

DECISION PROCESS (follow this every turn):
1. Read the Task and Goal to understand your objective.
2. Read Prior Observations carefully. These are the results of tools you already called.
3. If the observations already show the user goal has been achieved (e.g. files were successfully written, commands ran successfully), you MUST return "complete". Do NOT repeat a tool call that already succeeded.
4. If more work remains, call the NEXT tool needed. Never re-call a tool with identical arguments that already succeeded.

CRITICAL RULES:
- "action" MUST be exactly "tool_call" or "complete". Never use the tool name as the action value.
- "tool_name" must be one of the exact tool names listed (e.g. "fs.write", "cmd.exec").
- "tool_args" must match the input schema for that tool. Check the schema carefully.
- Delegation policy:
  - For greenfield scaffolding or building a project from scratch, do not delegate by default.
  - In greenfield work, prefer direct execution via tools in this worker until the scaffold/build is cohesive.
  - If delegation is needed, use tool_call with tool_name "subagent.spawn" and objective in tool_args.
  - Delegate only clearly parallelizable and low-conflict work (e.g. read-only research, audits, or isolated non-overlapping subtasks).
- For cmd.exec:
   - "cmd" is the binary name (e.g. "mkdir", "bun", "node"), "args" is an array of arguments (e.g. ["-p","src/components"]).
   - CRITICAL: Use "workdir" parameter to run inside subdirectories. NEVER use "cd path && command" syntax.
   - CORRECT: {{"tool_name":"cmd.exec","tool_args":{{"cmd":"bun","args":["install"],"workdir":"frontend"}}}}
   - WRONG: {{"tool_name":"cmd.exec","tool_args":{{"command":"cd frontend && bun install"}}}}
   - Avoid "command" field unless shell syntax is truly required.
 - For directory discovery and existence checks, ALWAYS use "fs.list" tool. NEVER use "ls", "dir", or shell commands for directory listing.
- For skill workflows:
  - Use "skills.list" to discover available catalog skills.
  - Use "skills.load" to import/load a skill when the task asks for installing or enabling a skill.
  - Use "skills.remove" only when explicitly asked to remove a custom skill.
- Always confirm directory state before destructive or structural operations (list first, then change).
- Keep paths within the workspace. If a task appears to require outside-workspace access, stop and complete with a summary explaining the blocker.
- For fs.write: "path" is relative to workspace root, "content" is the full file content as a string.
- For fs.read: "path" is relative to workspace root.
- When writing files, include the COMPLETE file content. Do not use placeholders or truncation.
- Produce only valid JSON. Escape special characters in strings properly.
- NEVER repeat a tool call with the same arguments if the prior observation shows it succeeded. Return "complete" instead.

## Switching to PLAN Mode

If the user explicitly asks you to "go back to planning," "revise the plan," or "switch to plan mode," use the `agent.request_plan_mode` tool with:
- `reason`: Brief explanation of why the switch is being requested
- `needs_revision`: Whether the current implementation needs planning changes (default: true)

This signals intent to the user to return to planning mode for plan revisions."#,
        base, platform
    )
}

// ---------------------------------------------------------------------------
// JSON extraction (brace-counting)
// ---------------------------------------------------------------------------

/// Extract the first complete JSON object from `input` using brace counting.
///
/// This is more robust than the naive first-`{`-to-last-`}` approach because
/// it correctly handles:
/// - Multiple JSON objects in one response (e.g. `{...}\n{...}`)
/// - Escaped characters in strings
/// - Nested braces
///
/// Falls back to the naive approach if brace counting doesn't find a match.
pub fn extract_json_object(input: &str) -> Option<String> {
    let start = input.find('{')?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in input[start..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(input[start..=start + i].to_string());
                }
            }
            _ => {}
        }
    }

    // Fallback: first { to last }
    let end = input.rfind('}')?;
    if end < start {
        return None;
    }
    Some(input[start..=end].to_string())
}

// ---------------------------------------------------------------------------
// Worker JSON normalization
// ---------------------------------------------------------------------------

/// Normalize worker JSON when the LLM puts the tool name as the `"action"` value
/// instead of using `"tool_call"`.
///
/// Examples:
/// ```text
/// {"action": "fs.write", "tool_args": {...}} -> {"action": "tool_call", "tool_name": "fs.write", ...}
/// {"action": "cmd.exec", "args": {...}}      -> {"action": "tool_call", "tool_name": "cmd.exec", "tool_args": {...}}
/// ```
pub fn normalize_worker_json(raw: &str) -> String {
    let Ok(mut obj) = serde_json::from_str::<serde_json::Value>(raw) else {
        return raw.to_string();
    };
    let Some(map) = obj.as_object_mut() else {
        return raw.to_string();
    };

    let action = map
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // If action is already "tool_call" or "complete", we still need to check
    // if tool_name is present for tool_call actions - the LLM may have provided
    // a malformed response with action="tool_call" but missing tool_name.
    if action == "complete" {
        return serde_json::to_string(&obj).unwrap_or_else(|_| raw.to_string());
    }

    // Determine whether this looks like a tool call. We check multiple signals:
    // 1. The action value looks like a tool name (contains a dot, or matches keywords)
    // 2. The object has "tool_name", "tool_args", "args", "input", or "parameters" keys
    //    even if "action" is something unexpected
    // 3. No "action" key at all but "tool_name" is present
    let action_looks_like_tool = action.contains('.')
        || [
            "read", "write", "exec", "search", "status", "diff", "apply", "patch", "list", "load",
            "remove", "spawn", "commit", "log",
        ]
        .iter()
        .any(|kw| action.contains(kw));

    let has_tool_call_keys = map.contains_key("tool_name")
        || map.contains_key("tool_args")
        || (map.contains_key("args") && !action.is_empty() && action != "complete");

    let is_tool_call = action_looks_like_tool || has_tool_call_keys;

    if is_tool_call {
        map.insert(
            "action".to_string(),
            serde_json::Value::String("tool_call".to_string()),
        );

        // If tool_name is not present, infer it from the action value or argument keys.
        if !map.contains_key("tool_name") {
            if action_looks_like_tool && !action.is_empty() && action != "tool_call" {
                map.insert(
                    "tool_name".to_string(),
                    serde_json::Value::String(action.clone()),
                );
            } else {
                // Infer tool_name from argument keys
                let inferred_tool = infer_tool_from_args(map);
                if let Some(tool_name) = inferred_tool {
                    map.insert(
                        "tool_name".to_string(),
                        serde_json::Value::String(tool_name),
                    );
                }
                // else: tool_name truly missing — serde will catch it downstream
            }
        }

        // Handle various LLM naming conventions for tool arguments.
        normalize_tool_args(map);
    }

    serde_json::to_string(&obj).unwrap_or_else(|_| raw.to_string())
}

/// Infer the tool name based on argument keys present in the request.
/// This helps when the LLM provides action="tool_call" but forgets tool_name.
fn infer_tool_from_args(map: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    // Check for cmd.exec indicators
    if map.contains_key("cmd") || map.contains_key("command") || map.contains_key("workdir") {
        return Some("cmd.exec".to_string());
    }

    // Check for fs.write indicators (has both path and content)
    if map.contains_key("path") && map.contains_key("content") {
        return Some("fs.write".to_string());
    }

    // Check for fs.list indicators
    if map.contains_key("path")
        && (map.contains_key("recursive")
            || map.contains_key("max_depth")
            || map.contains_key("limit")
            || map.contains_key("files_only")
            || map.contains_key("dirs_only"))
    {
        return Some("fs.list".to_string());
    }

    // Check for fs.read indicators (has path but not content)
    if map.contains_key("path") && !map.contains_key("content") {
        return Some("fs.read".to_string());
    }

    // Check for fs.search indicators
    if map.contains_key("pattern") {
        return Some("fs.search".to_string());
    }

    // Check for git.status indicators
    if map.contains_key("status") && !map.contains_key("cmd") {
        return Some("git.status".to_string());
    }

    // Check for subagent.spawn indicators
    if map.contains_key("objective") && map.contains_key("goal") {
        return Some("subagent.spawn".to_string());
    }

    None
}

/// Extract tool_args from non-standard keys the LLM may have used.
fn normalize_tool_args(map: &mut serde_json::Map<String, serde_json::Value>) {
    if map.contains_key("tool_args") {
        return;
    }

    if let Some(args) = map.remove("args") {
        map.insert("tool_args".to_string(), args);
    } else if let Some(input) = map.remove("input") {
        map.insert("tool_args".to_string(), input);
    } else if let Some(params) = map.remove("parameters") {
        map.insert("tool_args".to_string(), params);
    } else {
        // Collect remaining non-standard keys as tool_args.
        let reserved = ["action", "tool_name", "tool_args", "rationale"];
        let extra_keys: Vec<String> = map
            .keys()
            .filter(|k| !reserved.contains(&k.as_str()))
            .cloned()
            .collect();
        if !extra_keys.is_empty() {
            let mut tool_args = serde_json::Map::new();
            for key in extra_keys {
                if let Some(val) = map.remove(&key) {
                    tool_args.insert(key, val);
                }
            }
            map.insert(
                "tool_args".to_string(),
                serde_json::Value::Object(tool_args),
            );
        } else {
            map.insert("tool_args".to_string(), serde_json::json!({}));
        }
    }
}
