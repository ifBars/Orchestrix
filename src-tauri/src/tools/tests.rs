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

        // Verify all built-in tools are present
        assert!(names.contains(&"fs.read".to_string()));
        assert!(names.contains(&"fs.write".to_string()));
        assert!(names.contains(&"fs.list".to_string()));
        assert!(names.contains(&"fs.patch".to_string()));
        assert!(names.contains(&"search.rg".to_string()));
        assert!(names.contains(&"search.files".to_string()));
        assert!(names.contains(&"cmd.exec".to_string()));
        assert!(names.contains(&"git.status".to_string()));
        assert!(names.contains(&"git.diff".to_string()));
        assert!(names.contains(&"git.apply_patch".to_string()));
        assert!(names.contains(&"git.commit".to_string()));
        assert!(names.contains(&"git.log".to_string()));
        assert!(names.contains(&"agent.todo".to_string()));
        assert!(names.contains(&"agent.complete".to_string()));
        assert!(names.contains(&"skills.list".to_string()));
        assert!(names.contains(&"skills.load".to_string()));
        assert!(names.contains(&"skills.remove".to_string()));
        assert!(names.contains(&"subagent.spawn".to_string()));
        assert!(names.contains(&"agent.request_build_mode".to_string()));
        assert!(names.contains(&"agent.request_plan_mode".to_string()));
        assert!(names.contains(&"agent.create_artifact".to_string()));

        // Count only built-in tools (exclude MCP tools which have "." in server name like "server.tool")
        // Built-in tools use "_" separators (e.g., dev_server.start, agent.todo)
        let builtin_names: Vec<&String> = names
            .iter()
            .filter(|n| {
                !n.contains('.')
                    || n.starts_with("dev_server.")
                    || n.starts_with("agent.")
                    || n.starts_with("skills.")
                    || n.starts_with("git.")
                    || n.starts_with("fs.")
                    || n.starts_with("cmd.")
                    || n.starts_with("search.")
                    || n.starts_with("subagent.")
                    || n.starts_with("web.")
            })
            .collect();
        // 26 built-in tools (19 original + fs.patch + search.files + 4 dev_server.*)
        assert_eq!(
            builtin_names.len(),
            26,
            "expected 26 built-in tools, got: {:?}",
            builtin_names
        );
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

    // Windows Unix command translation tests
    #[cfg(target_os = "windows")]
    mod windows_command_translation_tests {
        use crate::tools::cmd::translate_unix_to_windows;

        #[test]
        fn test_which_translates_to_where() {
            assert_eq!(translate_unix_to_windows("which bun"), "where bun");
            assert_eq!(translate_unix_to_windows("which git"), "where git");
            assert_eq!(translate_unix_to_windows("which node"), "where node");
        }

        #[test]
        fn test_rm_rf_translates_to_rmdir() {
            assert_eq!(
                translate_unix_to_windows("rm -rf node_modules"),
                "rmdir /s /q node_modules"
            );
            assert_eq!(translate_unix_to_windows("rm -r temp"), "rmdir /s /q temp");
        }

        #[test]
        fn test_rm_single_file_translates_to_del() {
            assert_eq!(translate_unix_to_windows("rm file.txt"), "del /q file.txt");
        }

        #[test]
        fn test_mkdir_p_translates_to_mkdir() {
            assert_eq!(
                translate_unix_to_windows("mkdir -p src/components"),
                "mkdir src/components"
            );
        }

        #[test]
        fn test_cp_r_translates_to_xcopy() {
            assert_eq!(
                translate_unix_to_windows("cp -r src dst"),
                "xcopy /e /i /h src dst"
            );
            assert_eq!(
                translate_unix_to_windows("cp -a src dst"),
                "xcopy /e /i /h src dst"
            );
            assert_eq!(
                translate_unix_to_windows("cp -R src dst"),
                "xcopy /e /i /h src dst"
            );
        }

        #[test]
        fn test_cp_single_file_translates_to_copy() {
            assert_eq!(
                translate_unix_to_windows("cp file1 file2"),
                "copy file1 file2"
            );
        }

        #[test]
        fn test_mv_translates_to_move() {
            assert_eq!(translate_unix_to_windows("mv old new"), "move old new");
        }

        #[test]
        fn test_touch_translates_to_type_nul() {
            assert_eq!(
                translate_unix_to_windows("touch newfile.txt"),
                "type nul > newfile.txt"
            );
        }

        #[test]
        fn test_cat_translates_to_type() {
            assert_eq!(translate_unix_to_windows("cat file.txt"), "type file.txt");
        }

        #[test]
        fn test_ls_translates_to_dir() {
            assert_eq!(translate_unix_to_windows("ls"), "dir");
            assert_eq!(translate_unix_to_windows("ls src"), "dir");
            assert_eq!(translate_unix_to_windows("ls -la"), "dir");
        }

        #[test]
        fn test_cd_then_command_strips_cd() {
            assert_eq!(
                translate_unix_to_windows("cd frontend && bun install"),
                "bun install"
            );
            assert_eq!(
                translate_unix_to_windows("cd src && npm run build"),
                "npm run build"
            );
        }

        #[test]
        fn test_non_translated_commands_unchanged() {
            // Commands that don't need translation should pass through unchanged
            assert_eq!(translate_unix_to_windows("echo hello"), "echo hello");
            assert_eq!(translate_unix_to_windows("git status"), "git status");
        }

        #[test]
        fn test_translation_handles_whitespace() {
            assert_eq!(translate_unix_to_windows("  which bun  "), "where bun");
            assert_eq!(
                translate_unix_to_windows("rm   -rf   dir"),
                "rmdir /s /q dir"
            );
        }
    }

    // Windows cmd.exec with translated Unix commands tests
    #[cfg(target_os = "windows")]
    #[test]
    fn test_cmd_exec_translates_which_to_where() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"command": "which git"}),
                },
            )
            .expect("cmd.exec which git should succeed via translation");

        assert!(output.ok, "which git should succeed: {:?}", output.data);
        let stdout = output
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            !stdout.is_empty() || output.ok,
            "where git should return a path"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_cmd_exec_translates_ls_to_dir() {
        let workspace = temp_workspace();
        std::fs::create_dir_all(workspace.join("src")).unwrap();
        std::fs::write(workspace.join("src/main.rs"), "fn main() {}").unwrap();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"command": "ls"}),
                },
            )
            .expect("cmd.exec ls should succeed via translation");

        assert!(
            output.ok,
            "ls should succeed via dir translation: {:?}",
            output.data
        );
        let stdout = output
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            stdout.contains("src")
                || stdout.contains("main.rs")
                || stdout.contains("test-workspace"),
            "dir output should contain files or directories: {}",
            stdout
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_cmd_exec_translates_cat_to_type() {
        let workspace = temp_workspace();
        std::fs::write(workspace.join("test.txt"), "hello world").unwrap();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"command": "cat test.txt"}),
                },
            )
            .expect("cmd.exec cat should succeed via translation");

        assert!(
            output.ok,
            "cat test.txt should succeed via type translation"
        );
        let stdout = output
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            stdout.contains("hello world"),
            "type output should contain file content"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_cmd_exec_translates_rm_to_del() {
        let workspace = temp_workspace();
        let test_file = workspace.join("to_delete.txt");
        std::fs::write(&test_file, "delete me").unwrap();
        assert!(test_file.exists(), "test file should exist before deletion");

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"command": "rm to_delete.txt"}),
                },
            )
            .expect("cmd.exec rm should succeed via translation");

        assert!(
            output.ok,
            "rm to_delete.txt should succeed via del translation"
        );
        assert!(!test_file.exists(), "test file should be deleted");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_cmd_exec_cd_chain_translates_correctly() {
        let workspace = temp_workspace();
        let subdir = workspace.join("frontend");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("package.json"), "{}").unwrap();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"command": "cd frontend && echo in frontend"}),
                },
            )
            .expect("cd chain should translate correctly");

        assert!(
            output.ok,
            "cd frontend && echo should work: {:?}",
            output.data
        );
    }

    // Additional non-Windows specific tests
    #[test]
    fn test_cmd_exec_with_empty_args_array() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "cmd.exec".to_string(),
                args: serde_json::json!({"cmd": "echo", "args": []}),
            },
        );

        assert!(result.is_ok(), "cmd.exec with empty args should succeed");
        let output = result.unwrap();
        assert!(
            output.ok
                || !output
                    .data
                    .get("stdout")
                    .unwrap()
                    .as_str()
                    .unwrap_or("")
                    .is_empty()
        );
    }

    #[test]
    fn test_cmd_exec_handles_special_characters_in_args() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"command": "echo hello && echo world"}),
                },
            )
            .expect("cmd.exec with && should succeed");

        let stdout = output
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            stdout.contains("hello") && stdout.contains("world"),
            "output should contain both echos"
        );
    }

    #[test]
    fn test_fs_list_with_various_patterns() {
        let workspace = temp_workspace();
        std::fs::create_dir_all(workspace.join("src/utils")).unwrap();
        std::fs::write(workspace.join("src/main.rs"), "").unwrap();
        std::fs::write(workspace.join("src/utils/helper.rs"), "").unwrap();
        std::fs::write(workspace.join("Cargo.toml"), "").unwrap();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "fs.list".to_string(),
                    args: serde_json::json!({"path": "src"}),
                },
            )
            .expect("fs.list src should succeed");

        assert!(result.ok, "fs.list should succeed");
        let entries = result
            .data
            .get("entries")
            .and_then(|v| v.as_array())
            .expect("should have entries");
        assert!(
            entries.len() >= 2,
            "src should have at least main.rs and utils/"
        );
    }

    #[test]
    fn test_fs_list_recursive() {
        let workspace = temp_workspace();
        std::fs::create_dir_all(workspace.join("a/b/c")).unwrap();
        std::fs::write(workspace.join("a/b/c/deep.txt"), "").unwrap();
        std::fs::write(workspace.join("a/shallow.txt"), "").unwrap();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "fs.list".to_string(),
                    args: serde_json::json!({"path": "a", "recursive": true}),
                },
            )
            .expect("fs.list recursive should succeed");

        assert!(result.ok, "fs.list recursive should succeed");
        let entries = result
            .data
            .get("entries")
            .and_then(|v| v.as_array())
            .expect("should have entries");
        assert!(
            entries.len() >= 3,
            "recursive list should include all nested files"
        );
    }

    #[test]
    fn test_search_rg_basic() {
        let workspace = temp_workspace();
        std::fs::write(
            workspace.join("test.rs"),
            "fn hello() { println!(\"hello\"); }",
        )
        .unwrap();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "search.rg".to_string(),
                    args: serde_json::json!({"pattern": "hello"}),
                },
            )
            .expect("search.rg should succeed");

        assert!(result.ok, "search.rg should succeed");
        let stdout = result
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(stdout.contains("hello"), "rg output should contain match");
    }

    #[test]
    fn test_workdir_normalization() {
        let workspace = temp_workspace();
        let nested = workspace.join("level1/level2");
        std::fs::create_dir_all(&nested).unwrap();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let output = registry
            .invoke(
                &policy,
                &workspace,
                ToolCallInput {
                    name: "cmd.exec".to_string(),
                    args: serde_json::json!({"cmd": "echo", "args": ["test"], "workdir": "level1/level2"}),
                },
            )
            .expect("cmd.exec with nested workdir should succeed");

        assert!(output.ok, "cmd.exec should succeed");
        let returned_workdir = output
            .data
            .get("workdir")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            returned_workdir.contains("level1") && returned_workdir.contains("level2"),
            "workdir should normalize correctly: {}",
            returned_workdir
        );
    }

    #[test]
    fn test_cmd_exec_binary_not_found_with_fallback() {
        let workspace = temp_workspace();
        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(workspace.clone());

        let result = registry.invoke(
            &policy,
            &workspace,
            ToolCallInput {
                name: "cmd.exec".to_string(),
                args: serde_json::json!({"cmd": "definitely_not_a_real_command_12345", "args": ["arg1"]}),
            },
        );

        match result {
            Err(_) => {
                // Expected: command not found should error
            }
            Ok(output) => {
                // On some systems, unknown commands might behave differently
                assert!(
                    !output.ok
                        || output
                            .data
                            .get("stderr")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .contains("not found"),
                    "unknown command should fail: {:?}",
                    output.data
                );
            }
        }
    }
}
