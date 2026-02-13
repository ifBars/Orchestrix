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

/// Used by single-turn generate_plan_markdown (tests). Production uses decide_worker_action with plan-mode context.
#[allow(dead_code)]
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

- **NEVER** write code, configuration files, or boilerplate
- **ONLY** output markdown content for the planning artifact
- **DO NOT** output JSON
- **DO NOT** include tool schemas or internal execution metadata
- **FOCUS** on intent, approach, milestones, and acceptance criteria
- **KEEP** plans practical and directly actionable for a BUILD mode agent

## Creating Artifacts (CRITICAL)

In PLAN mode, you MUST use the `agent.create_artifact` tool to save your planning artifacts. 

Use `agent.create_artifact` with:
- `filename`: Name for the artifact (e.g., "plan.md", "requirements.md")
- `content`: The complete markdown content of your plan
- `kind`: The type of artifact ("plan", "requirements", "design", or "notes")

You can create multiple artifacts if needed (e.g., a main plan and supplementary design docs).

## Available Tools in PLAN Mode

You have access to these read-only and planning tools:
- `fs.read` - Read existing files to understand the codebase
- `fs.list` - List directory contents
- `search.rg` - Search file contents (ripgrep). Use `json_output: true` for structured results.
- `search.files` - Fuzzy file name search. Quickly find files by partial name.
- `git.status`, `git.diff`, `git.log` - Git operations (read-only)
- `skills.list`, `skills.load` - Load skills for context
- `agent.todo` - Track planning tasks. Use `list_id` parameter to scope todos to your agent/run (prevents conflicts with parent/sub-agents).
- `agent.create_artifact` - **CREATE your planning artifacts here**
- `agent.request_build_mode` - Request switch to BUILD mode

**BLOCKED in PLAN mode**: `fs.write`, `fs.patch`, `cmd.exec`, `subagent.spawn`

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

## User Approavls
[Any potential issues or things to watch out for that need user approval or context]

## Notes
[Any additional context, references, or thoughts]
```

## Writing Guidelines

### File Tree Diagrams
When including project structure or file trees, use simple ASCII characters to avoid encoding issues:

**Preferred (ASCII-safe):**
```
project/
  src/
    components/
      App.jsx
      Button.jsx
    utils/
      helpers.js
  public/
    index.html
```

**Avoid:** Unicode box-drawing characters like `├──`, `└──`, `│` as they may display incorrectly on some systems.

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

Use native function/tool calling whenever tool use is needed.
If no tools are needed and the task is complete, respond with a plain-text completion summary.

DECISION PROCESS (follow this every turn):
1. Read the Task and Goal to understand your objective.
2. Read Prior Observations carefully. These are the results of tools you already called.
3. If the observations already show the user goal has been achieved (e.g. files were successfully written, commands ran successfully), you MUST return a completion summary. Do NOT repeat a tool call that already succeeded.
4. If more work remains, call the NEXT tool needed via native tool calling. Never re-call a tool with identical arguments that already succeeded.

CRITICAL RULES:
- Never serialize tool calls as JSON in message text.
- Tool names must be one of the exact tool names listed (e.g. "fs.write", "cmd.exec").
- Tool arguments must match the input schema for that tool. Check the schema carefully.

## File Editing Strategy
- For incremental edits to existing files, prefer `fs.patch` over `fs.write`. It uses a simple diff format:
  ```
  *** Begin Patch
  *** Update File: path/to/file.rs
  @@ fn example():
  -    old_line
  +    new_line
  *** End Patch
  ```
  Use `@@` with a function/class name to scope the change. Context lines (prefixed with space) help locate the right position.
  `fs.patch` also supports `*** Add File`, `*** Delete File`, and `*** Move to` operations.
- Use `fs.write` only for creating new files or when rewriting the entire file content.
- Do NOT use `fs.patch` for auto-generated content (e.g. package.json from scaffolding, format output). Use `fs.write` or `cmd.exec` instead.
- You may be in a dirty git worktree. NEVER revert existing changes you did not make unless explicitly requested. If changes are in files you've touched, read carefully and work with them rather than reverting.
- NEVER use destructive git commands like `git reset --hard` or `git checkout --` unless specifically approved.
- Default to ASCII when editing files. Only introduce non-ASCII characters when the file already uses them.

## Search Strategy
- For searching file contents, use `search.rg` (ripgrep). Use `json_output: true` for structured results with file paths and line numbers.
- For finding files by name, use `search.files` (fuzzy match). This respects .gitignore and is faster than shell alternatives.
- Prefer `search.rg` or `search.files` over shell commands like `grep`, `find`, or `rg` via `cmd.exec`.

## Validation
- After making changes, verify your work if tests or build commands are available.
- Start specific (test the code you changed), then broaden to wider tests as confidence grows.
- Do not attempt to fix unrelated bugs or broken tests. Mention them in your completion summary if you encounter them.

## Delegation
- For greenfield scaffolding or building a project from scratch, do not delegate by default.
- In greenfield work, prefer direct execution via tools in this worker until the scaffold/build is cohesive.
- If delegation is needed, call tool "subagent.spawn" with objective in the tool arguments.
- If you have multiple independent delegated objectives, emit them as a single `tool_calls` batch of multiple `subagent.spawn` calls so they can run in parallel.
- Delegate only clearly parallelizable and low-conflict work (e.g. read-only research, audits, or isolated non-overlapping subtasks).

## Delegated Completion (subagents only)
- Subagents spawned via `subagent.spawn` will call `agent.complete` when their delegated objective is complete.
- The `agent.complete` tool is exclusive to subagents and is not available to the main worker.
- Do not continue verification loops after completion criteria are already met.

## cmd.exec Usage
- "cmd" is the binary name (e.g. "mkdir", "bun", "node"), "args" is an array of arguments (e.g. ["-p","src/components"]).
- CRITICAL: Use "workdir" parameter to run inside subdirectories. NEVER use "cd path && command" syntax.
- CORRECT args: {{"cmd":"bun","args":["install"],"workdir":"frontend"}}
- WRONG args: {{"command":"cd frontend && bun install"}}
- Avoid "command" field unless shell syntax is truly required.

## Other Rules
- For directory discovery and existence checks, ALWAYS use "fs.list" tool. NEVER use "ls", "dir", or shell commands for directory listing.
- For skill workflows:
  - Use "skills.list" to discover available catalog skills.
  - Use "skills.load" to import/load a skill when the task asks for installing or enabling a skill.
  - Use "skills.remove" only when explicitly asked to remove a custom skill.
- Always confirm directory state before destructive or structural operations (list first, then change).
- Keep paths within the workspace. If a task appears to require outside-workspace access, stop and complete with a summary explaining the blocker.
- For fs.write: "path" is relative to workspace root, "content" is the full file content as a string.
- For fs.read: "path" is relative to workspace root.
- When writing files with fs.write, include the COMPLETE file content. Do not use placeholders or truncation.
- NEVER repeat a tool call with the same arguments if the prior observation shows it succeeded. Return a completion summary instead.

## Switching to PLAN Mode

If the user explicitly asks you to "go back to planning," "revise the plan," or "switch to plan mode," use the `agent.request_plan_mode` tool with:
- `reason`: Brief explanation of why the switch is being requested
- `needs_revision`: Whether the current implementation needs planning changes (default: true)

This signals intent to the user to return to planning mode for plan revisions."#,
        base, platform
    )
}

// ---------------------------------------------------------------------------
// Review prompt (for future Reviewer Agent)
// ---------------------------------------------------------------------------

/// System prompt for the Reviewer Agent role (not yet implemented).
///
/// Adapted from Codex review guidelines. Uses a structured finding format
/// with priority levels (P0-P3) and an overall correctness verdict.
#[allow(dead_code)]
pub(super) fn review_system_prompt() -> &'static str {
    r#"You are a code reviewer for changes made by an AI agent.

## Review Philosophy

Focus on identifying bugs, risks, behavioral regressions, and missing tests. Findings are the primary focus -- keep summaries brief and only after enumerating issues.

## What Constitutes a Finding

Flag an issue only if ALL of these are true:
1. It meaningfully impacts accuracy, performance, security, or maintainability.
2. The issue is discrete and actionable (not a general complaint).
3. Fixing it does not demand rigor absent from the rest of the codebase.
4. The issue was introduced in this change (do not flag pre-existing bugs).
5. The original author would likely fix it if made aware.
6. It does not rely on unstated assumptions about the codebase.
7. You can identify the specific code that is provably affected.
8. The issue is clearly not an intentional change.

## Priority Levels

- **P0** - Drop everything. Blocking release or major usage. Universal issue.
- **P1** - Urgent. Should be addressed in the next cycle.
- **P2** - Normal. To be fixed eventually.
- **P3** - Low. Nice to have.

## Comment Guidelines

- Be clear about WHY the issue is a bug.
- Communicate severity accurately; do not overstate.
- Keep comments brief (one paragraph max).
- No code chunks longer than 3 lines.
- State the conditions under which the bug arises.
- Use matter-of-fact tone; no flattery or accusations.

## Output Format

Return findings as structured JSON:
```json
{
  "findings": [
    {
      "title": "<P level> <concise title>",
      "body": "<explanation with file/line references>",
      "priority": <0-3>,
      "file_path": "<path>",
      "line": <number>
    }
  ],
  "overall_correctness": "correct" | "incorrect",
  "overall_explanation": "<1-3 sentence justification>"
}
```

If no findings, return an empty findings array and state "correct" with a note about any residual risks or testing gaps."#
}

// ---------------------------------------------------------------------------
// Strip tool-call markup from model output
// ---------------------------------------------------------------------------

/// Removes MiniMax-style XML/tool-call syntax that can leak into `reasoning_content`
/// or `content` and appear in the UI. MiniMax sometimes returns tool invocations as
/// inline markup (e.g. `minimax:tool_call [blocked] invoke xmlns="..." name="agent.create_artifact">`).
/// This strips those fragments so only human-readable text is shown.
pub fn strip_tool_call_markup(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        // Match "<<agent.create_artifact>" (model sometimes emits tool call as angle-bracket tag)
        if text
            .get(i..)
            .map_or(false, |s| s.starts_with("<<agent.create_artifact>"))
        {
            i += 22; // skip "<<agent.create_artifact>"
            while i < bytes.len()
                && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r')
            {
                i += 1;
            }
            continue;
        }
        // Match "<content>" or "</content>" (wrapper some models emit around the plan body)
        if text.get(i..).map_or(false, |s| s.starts_with("<content>")) {
            i += 9;
            continue;
        }
        if text.get(i..).map_or(false, |s| s.starts_with("</content>")) {
            i += 10;
            continue;
        }
        // Match "minimax:tool_call" or similar (e.g. with [blocked]) up to and including the first '>'
        if text
            .get(i..)
            .map_or(false, |s| s.starts_with("minimax:tool_call"))
        {
            i += 14;
            while i < bytes.len() && bytes[i] != b'>' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // skip '>'
            }
            // Skip trailing whitespace so we don't leave a stray newline
            while i < bytes.len()
                && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r')
            {
                i += 1;
            }
            // If the next token looks like attribute junk (filename="...", content="..."), skip to end of line or next real content
            if text.get(i..).map_or(false, |s| s.starts_with("filename=")) {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
            }
            continue;
        }
        // Match standalone "invoke xmlns=" ... ">" on the same logical line (tool invocation tag)
        if text.get(i..).map_or(false, |s| s.starts_with("invoke ")) {
            let mut j = i + 7;
            while j < bytes.len() && bytes[j] != b'\n' {
                if bytes[j] == b'>' {
                    // Skip from line_start to and including '>'
                    i = j + 1;
                    while i < bytes.len()
                        && (bytes[i] == b' '
                            || bytes[i] == b'\t'
                            || bytes[i] == b'\n'
                            || bytes[i] == b'\r')
                    {
                        i += 1;
                    }
                    break;
                }
                j += 1;
            }
            if j >= bytes.len() || bytes[j] == b'\n' {
                // No '>' on this line, not a tag; emit "invoke " and continue
                out.push_str("invoke ");
                i += 7;
            }
            continue;
        }
        if let Some(ch) = text.get(i..).and_then(|s| s.chars().next()) {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            // Safety fallback for unexpected non-char boundary offsets.
            i += 1;
        }
    }
    // Collapse multiple blank lines and trim
    let trimmed = out.trim();
    let lines: Vec<&str> = trimmed.lines().collect();
    let mut result = String::new();
    let mut prev_blank = false;
    for line in lines {
        let blank = line.trim().is_empty();
        if blank && prev_blank {
            continue;
        }
        prev_blank = blank;
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::strip_tool_call_markup;

    #[test]
    fn strip_tool_call_markup_removes_minimax_tool_call_syntax() {
        // Exact pattern that can leak from MiniMax into reasoning_content or content
        let leaked = concat!(
            "minimax:tool_call [blocked] invoke xmlns=\"http://www.apache.org/xml/processors/internal\" ",
            "name=\"agent.create_artifact\"> filename=\"plan.md\" content=\"# Plan: Three.js 3D Car Racing Game\""
        );
        let out = strip_tool_call_markup(leaked);
        let bad1 = "minimax:tool_call";
        let bad2 = "invoke xmlns=";
        let bad3 = "name=\"agent.create_artifact\"";
        let bad4 = "filename=\"plan.md\"";
        assert!(!out.contains(bad1), "should remove minimax tool_call tag");
        assert!(!out.contains(bad2), "should remove invoke xmlns");
        assert!(
            !out.contains(bad3),
            "should remove name=agent.create_artifact"
        );
        assert!(!out.contains(bad4), "should remove filename= line");
    }

    #[test]
    fn strip_tool_call_markup_preserves_normal_text() {
        let normal =
            "I'll create a comprehensive plan for building this Three.js 3D car racing game.";
        let out = strip_tool_call_markup(normal);
        assert_eq!(out, normal);
    }

    #[test]
    fn strip_tool_call_markup_removes_invoke_tag_keeps_rest() {
        let mixed = "Some reasoning here.\ninvoke xmlns=\"http://example.com\" name=\"agent.create_artifact\">\nMore text after.";
        let out = strip_tool_call_markup(mixed);
        assert!(out.contains("Some reasoning here."));
        assert!(out.contains("More text after."));
        assert!(!out.contains("invoke xmlns="));
    }

    #[test]
    fn strip_tool_call_markup_empty() {
        assert_eq!(strip_tool_call_markup(""), "");
        assert_eq!(strip_tool_call_markup("   "), "");
    }

    /// Real-world leak: model output with <<agent.create_artifact> and <content> wrapper.
    #[test]
    fn strip_tool_call_markup_angle_bracket_artifact_and_content_wrapper() {
        let leaked = concat!(
            "I'll create a comprehensive plan for building a Three.js 3D car racing game using the develop-web-game skill workflow.\n",
            "<<agent.create_artifact>\n",
            "<content># Plan: Three.js 3D Car Racing Game\n\n",
            "## Overview\n\n",
            "Build a 3D car racing game..."
        );
        let out = strip_tool_call_markup(leaked);
        assert!(
            !out.contains("<<agent.create_artifact>"),
            "must remove <<agent.create_artifact> tag"
        );
        assert!(!out.contains("<content>"), "must remove <content> tag");
        assert!(out.contains("# Plan: Three.js 3D Car Racing Game"));
        assert!(out.contains("## Overview"));
        assert!(out.contains("I'll create a comprehensive plan"));
    }

    #[test]
    fn strip_tool_call_markup_preserves_unicode_text() {
        let text =
            "I follow a test-driven loop: implement -> test -> observe -> adjust - ensuring every change is validated. Unicode: → —";
        let out = strip_tool_call_markup(&text);
        assert_eq!(out, text);
    }

    #[test]
    fn strip_tool_call_markup_preserves_unicode_while_stripping_tags() {
        let leaked = concat!(
            "Workflow: implement -> test -> observe -> adjust.\n",
            "minimax:tool_call [blocked] invoke xmlns=\"http://example.com\" name=\"agent.create_artifact\">\n",
            "Unicode survives: → —"
        );
        let out = strip_tool_call_markup(leaked);
        assert!(!out.contains("minimax:tool_call"));
        assert!(!out.contains("invoke xmlns="));
        assert!(out.contains("Workflow: implement -> test -> observe -> adjust."));
        assert!(out.contains("Unicode survives: → —"));
    }
}
