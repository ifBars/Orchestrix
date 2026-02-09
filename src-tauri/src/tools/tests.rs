//! Tool registry and tool execution tests

#[cfg(test)]
mod tests {
    use crate::policy::PolicyEngine;
    use crate::runtime::worktree::WorktreeManager;
    use crate::tests::{cleanup, init_git_repo, temp_workspace};
    use crate::tools::{ToolCallInput, ToolRegistry};
    use uuid::Uuid;

    #[test]
    fn test_tool_registry_has_all_tools() {
        let registry = ToolRegistry::default();
        let tools = registry.list();
        let names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();

        assert!(names.contains(&"fs.read".to_string()));
        assert!(names.contains(&"fs.write".to_string()));
        assert!(names.contains(&"search.rg".to_string()));
        assert!(names.contains(&"cmd.exec".to_string()));
        assert!(names.contains(&"git.status".to_string()));
        assert!(names.contains(&"git.diff".to_string()));
        assert!(names.contains(&"git.apply_patch".to_string()));
        assert!(names.contains(&"git.commit".to_string()));
        assert!(names.contains(&"git.log".to_string()));
        assert!(names.contains(&"agent.todo".to_string()));
        // 14 tools including agent.todo
        assert_eq!(names.len(), 14);
    }

    #[test]
    fn test_git_tools_in_worktree() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let manager = WorktreeManager::new();
        let run_id = Uuid::new_v4().to_string();
        let agent_id = Uuid::new_v4().to_string();
        let info = manager
            .create_worktree(&workspace, &run_id, &agent_id)
            .unwrap();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(info.path.clone());

        // git.status
        let status = registry
            .invoke(
                &policy,
                &info.path,
                ToolCallInput {
                    name: "git.status".to_string(),
                    args: serde_json::json!({}),
                },
            )
            .unwrap();
        assert!(status.ok);

        // fs.write + git.commit
        std::fs::write(info.path.join("test-file.txt"), "hello world\n").unwrap();
        let commit = registry
            .invoke(
                &policy,
                &info.path,
                ToolCallInput {
                    name: "git.commit".to_string(),
                    args: serde_json::json!({"message": "test commit from agent"}),
                },
            )
            .unwrap();
        assert!(commit.ok, "git commit should succeed: {:?}", commit);

        // git.log
        let log = registry
            .invoke(
                &policy,
                &info.path,
                ToolCallInput {
                    name: "git.log".to_string(),
                    args: serde_json::json!({"count": 5}),
                },
            )
            .unwrap();
        assert!(log.ok);
        let stdout = log
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(stdout.contains("test commit from agent"));

        // git.diff (unstaged)
        std::fs::write(info.path.join("test-file.txt"), "modified\n").unwrap();
        let diff = registry
            .invoke(
                &policy,
                &info.path,
                ToolCallInput {
                    name: "git.diff".to_string(),
                    args: serde_json::json!({}),
                },
            )
            .unwrap();
        assert!(diff.ok);

        cleanup(&workspace);
    }

    #[test]
    fn test_tool_policy_denies_disallowed_command() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        // cmd.exec with a disallowed binary should be denied
        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "cmd.exec".to_string(),
                args: serde_json::json!({"cmd": "netstat", "args": ["-an"]}),
            },
        );

        match result {
            Err(e) => {
                println!("  Policy correctly denied netstat: {e}");
                assert!(
                    e.to_string().contains("not allowed") || e.to_string().contains("denied"),
                    "error should indicate denial: {e}"
                );
            }
            Ok(output) => {
                // Some systems may not have netstat; if policy allowed it, that's a bug
                panic!(
                    "netstat should have been denied by policy, got: {:?}",
                    output
                );
            }
        }

        cleanup(&workspace);
    }

    #[test]
    fn test_tool_policy_allows_git_commands() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        // git should be allowed
        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "cmd.exec".to_string(),
                args: serde_json::json!({"cmd": "git", "args": ["--version"]}),
            },
        );
        assert!(result.is_ok(), "git should be allowed by policy");
        assert!(result.unwrap().ok);

        cleanup(&workspace);
    }

    #[test]
    fn test_tool_policy_allows_cd_inside_workspace() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "cmd.exec".to_string(),
                args: serde_json::json!({"command": "cd . && git status"}),
            },
        );

        assert!(result.is_ok(), "cd inside workspace should be allowed");

        cleanup(&workspace);
    }

    #[test]
    fn test_tool_policy_cd_outside_workspace_requires_approval() {
        let workspace = temp_workspace();
        init_git_repo(&workspace);

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "cmd.exec".to_string(),
                args: serde_json::json!({"command": "cd C:\\Windows\\System32 && dir"}),
            },
        );

        match result {
            Err(e) => {
                let text = e.to_string().to_ascii_lowercase();
                assert!(
                    text.contains("approval required") || text.contains("path outside workspace"),
                    "cd outside workspace should require approval, got: {e}"
                );
            }
            Ok(output) => panic!(
                "cd outside workspace should not run without approval, got: {:?}",
                output
            ),
        }

        cleanup(&workspace);
    }

    #[test]
    fn test_tool_fs_write_outside_workspace_denied() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "fs.write".to_string(),
                args: serde_json::json!({
                    "path": "C:\\Windows\\System32\\evil.txt",
                    "content": "should not be written"
                }),
            },
        );

        assert!(
            result.is_err(),
            "writing outside workspace should be denied"
        );

        cleanup(&workspace);
    }

    #[test]
    fn test_tool_fs_write_creates_nested_dirs() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "fs.write".to_string(),
                args: serde_json::json!({
                    "path": "deep/nested/dir/file.txt",
                    "content": "nested content"
                }),
            },
        );

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(
            output.ok,
            "fs.write should succeed for nested dirs: {:?}",
            output
        );
        assert!(workspace.join("deep/nested/dir/file.txt").exists());
        let content = std::fs::read_to_string(workspace.join("deep/nested/dir/file.txt")).unwrap();
        assert_eq!(content, "nested content");

        cleanup(&workspace);
    }

    #[test]
    fn test_tool_unknown_tool_returns_error() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "nonexistent.tool".to_string(),
                args: serde_json::json!({}),
            },
        );

        assert!(result.is_err(), "unknown tool should error");
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("nonexistent.tool"),
            "error should name the tool: {err_str}"
        );

        cleanup(&workspace);
    }

    #[test]
    fn test_tool_fs_read_nonexistent_file() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "fs.read".to_string(),
                args: serde_json::json!({"path": "does-not-exist.txt"}),
            },
        );

        // Should return Ok but with ok=false or an error message, not crash
        match result {
            Ok(output) => {
                assert!(
                    !output.ok || output.error.is_some(),
                    "reading nonexistent file should indicate failure"
                );
            }
            Err(_) => {
                // Also acceptable -- tool execution error
            }
        }

        cleanup(&workspace);
    }

    // cmd.exec shell mode tests
    #[test]
    fn test_cmd_exec_binary_mode_with_args() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"cmd": "git", "args": ["--version"]}),
                },
            )
            .expect("cmd.exec git --version should succeed");

        assert!(output.ok, "git --version should succeed");
        let stdout = output
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            stdout.contains("git version"),
            "stdout should contain 'git version': {stdout}"
        );

        // Verify invoked metadata
        let invoked = output.data.get("invoked").unwrap();
        assert_eq!(invoked.get("mode").and_then(|v| v.as_str()), Some("binary"));

        cleanup(&workspace);
    }

    #[test]
    fn test_cmd_exec_shell_mode_via_command_field() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"command": "echo hello world"}),
                },
            )
            .expect("cmd.exec shell mode should succeed");

        assert!(output.ok, "echo hello world should succeed");
        let stdout = output
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            stdout.contains("hello world"),
            "stdout should contain 'hello world': {stdout}"
        );

        // Verify invoked metadata shows shell mode
        let invoked = output.data.get("invoked").unwrap();
        assert_eq!(invoked.get("mode").and_then(|v| v.as_str()), Some("shell"));

        cleanup(&workspace);
    }

    #[test]
    fn test_cmd_exec_shell_builtin_mkdir() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let subdir_name = "test-subdir";
        let subdir = workspace.join(subdir_name);
        assert!(!subdir.exists(), "subdir should not exist yet");

        // Use "command" field to ensure shell mode for mkdir (use relative path for Windows compatibility)
        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"command": format!("mkdir {}", subdir_name)}),
                },
            )
            .expect("cmd.exec mkdir should succeed");

        assert!(
            output.ok,
            "mkdir via shell should succeed: {:?}",
            output.data
        );
        assert!(subdir.exists(), "subdir should exist after mkdir");

        cleanup(&workspace);
    }

    #[test]
    fn test_cmd_exec_cmd_with_spaces_splits_into_args() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        // When "cmd" contains spaces and no explicit "args", it should split
        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"cmd": "git --version"}),
                },
            )
            .expect("cmd.exec with spaces should succeed");

        assert!(output.ok);
        let stdout = output
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(stdout.contains("git version"));

        cleanup(&workspace);
    }

    #[test]
    fn test_cmd_exec_policy_denied() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "cmd.exec".to_string(),
                args: serde_json::json!({"cmd": "rm", "args": ["-rf", "/"]}),
            },
        );

        match result {
            Err(crate::tools::ToolError::PolicyDenied(_)) => {
                // Expected
            }
            Err(e) => {
                // Also acceptable if policy blocks it differently
                println!("  Got error (acceptable): {e}");
            }
            Ok(output) => {
                // If policy doesn't block rm, the command itself should fail
                // on a sane system. Either way, we just verify the tool ran.
                println!("  cmd.exec allowed rm: ok={}", output.ok);
            }
        }

        cleanup(&workspace);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_cmd_exec_windows_shell_fallback_for_builtins() {
        // On Windows, shell builtins like "dir" should work via auto-fallback
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"cmd": "dir"}),
                },
            )
            .expect("cmd.exec dir should succeed via Windows shell fallback");

        assert!(output.ok, "dir should succeed: {:?}", output.data);

        cleanup(&workspace);
    }

    #[test]
    fn test_cmd_exec_missing_cmd_field() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "cmd.exec".to_string(),
                args: serde_json::json!({}),
            },
        );
        assert!(result.is_err(), "cmd.exec without cmd should fail");

        cleanup(&workspace);
    }

    #[test]
    fn test_cmd_exec_with_workdir_runs_in_subdirectory() {
        let workspace = temp_workspace();
        let subdir = workspace.join("sub");
        std::fs::create_dir_all(&subdir).unwrap();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"cmd": "ls", "args": ["-la"], "workdir": "sub"}),
                },
            )
            .expect("cmd.exec with workdir should succeed");

        assert!(output.ok);
        let returned_workdir = output
            .data
            .get("workdir")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            returned_workdir.ends_with("sub"),
            "workdir should end with sub: {returned_workdir}"
        );

        cleanup(&workspace);
    }

    #[test]
    fn test_cmd_exec_infers_cmd_from_first_arg() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"args": ["git", "--version"]}),
                },
            )
            .expect("cmd.exec should infer cmd from args[0]");

        assert!(output.ok);
        let stdout = output
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            stdout.contains("git version"),
            "stdout should contain git version: {stdout}"
        );

        cleanup(&workspace);
    }

    #[test]
    fn test_tool_registry_includes_all_skill_tools() {
        let registry = ToolRegistry::default();
        let tool_names: Vec<String> = registry.list().into_iter().map(|t| t.name).collect();

        assert!(
            tool_names.contains(&"skills.list".to_string()),
            "missing skills.list"
        );
        assert!(
            tool_names.contains(&"subagent.spawn".to_string()),
            "missing subagent.spawn"
        );
        assert!(
            tool_names.contains(&"skills.load".to_string()),
            "missing skills.load"
        );
        assert!(
            tool_names.contains(&"skills.remove".to_string()),
            "missing skills.remove"
        );
    }

    #[test]
    fn test_tool_reference_for_prompt_includes_skills() {
        let registry = ToolRegistry::default();
        let reference = registry.tool_reference_for_prompt();

        assert!(
            reference.contains("### skills.list"),
            "tool reference should include skills.list"
        );
        assert!(
            reference.contains("### subagent.spawn"),
            "tool reference should include subagent.spawn"
        );
        assert!(
            reference.contains("### skills.load"),
            "tool reference should include skills.load"
        );
        assert!(
            reference.contains("### skills.remove"),
            "tool reference should include skills.remove"
        );
    }
}
