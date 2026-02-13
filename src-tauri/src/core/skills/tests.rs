//! Skills module unit tests

#[cfg(test)]
mod tests {
    use crate::core::skills::{self, NewCustomSkill};
    use crate::policy::PolicyEngine;
    use crate::tests::{cleanup_skills_env, isolated_skills_path, temp_workspace};
    use crate::tools::{ToolCallInput, ToolRegistry};

    #[test]
    fn test_skills_list_all_returns_builtins() {
        let (skills_path, _guard) = isolated_skills_path();

        let all = skills::list_all_skills();
        assert!(
            all.len() >= 2,
            "should have at least 2 builtin skills, got {}",
            all.len()
        );

        let ids: Vec<&str> = all.iter().map(|s| s.id.as_str()).collect();
        assert!(
            ids.contains(&"vercel-react-best-practices"),
            "missing vercel-react-best-practices"
        );
        assert!(ids.contains(&"find-skills"), "missing find-skills");

        // All builtins should not be marked custom
        for skill in &all {
            if !skill.is_custom {
                assert!(
                    ["builtin", "vercel"].contains(&skill.source.as_str()),
                    "builtin skill {} has unexpected source: {}",
                    skill.id,
                    skill.source
                );
            }
        }

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_list_is_sorted_alphabetically() {
        let (skills_path, _guard) = isolated_skills_path();

        let all = skills::list_all_skills();
        let titles: Vec<String> = all.iter().map(|s| s.title.to_ascii_lowercase()).collect();
        let mut sorted = titles.clone();
        sorted.sort();
        assert_eq!(
            titles, sorted,
            "skills should be sorted alphabetically by title"
        );

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_list_deduplicates_by_id() {
        let (skills_path, _guard) = isolated_skills_path();

        // Add a custom skill with the same id as a builtin
        let _added = skills::add_custom_skill(NewCustomSkill {
            id: Some("find-skills".to_string()),
            title: "Custom Find Skills Override".to_string(),
            description: "Overridden".to_string(),
            install_command: "echo override".to_string(),
            url: "https://example.com".to_string(),
            source: None,
            tags: None,
        })
        .expect("add_custom_skill failed");

        let all = skills::list_all_skills();
        let find_skills_entries: Vec<_> = all.iter().filter(|s| s.id == "find-skills").collect();
        assert_eq!(
            find_skills_entries.len(),
            1,
            "should have exactly 1 find-skills entry after dedup"
        );

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_add_custom_skill_success() {
        let (skills_path, _guard) = isolated_skills_path();

        let skill = skills::add_custom_skill(NewCustomSkill {
            id: None,
            title: "My Test Skill".to_string(),
            description: "A test skill for unit tests".to_string(),
            install_command: "echo hello".to_string(),
            url: "https://example.com/test".to_string(),
            source: Some("test-source".to_string()),
            tags: Some(vec!["test".to_string(), "unit-test".to_string()]),
        })
        .expect("add_custom_skill failed");

        assert_eq!(skill.id, "my-test-skill", "id should be derived from title");
        assert_eq!(skill.title, "My Test Skill");
        assert_eq!(skill.source, "test-source");
        assert!(skill.is_custom);
        assert_eq!(skill.tags, vec!["test", "unit-test"]);

        // Verify it persisted
        let all = skills::list_all_skills();
        assert!(
            all.iter().any(|s| s.id == "my-test-skill"),
            "custom skill should be in list"
        );

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_add_custom_skill_with_explicit_id() {
        let (skills_path, _guard) = isolated_skills_path();

        let skill = skills::add_custom_skill(NewCustomSkill {
            id: Some("custom-explicit-id".to_string()),
            title: "Explicit ID Skill".to_string(),
            description: "Has an explicit id".to_string(),
            install_command: "echo test".to_string(),
            url: "https://example.com".to_string(),
            source: None,
            tags: None,
        })
        .expect("add failed");

        assert_eq!(skill.id, "custom-explicit-id");
        assert_eq!(skill.source, "custom"); // default source

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_add_custom_skill_validation() {
        let (skills_path, _guard) = isolated_skills_path();

        // Empty title should fail
        let result = skills::add_custom_skill(NewCustomSkill {
            id: None,
            title: "".to_string(),
            description: "desc".to_string(),
            install_command: "echo".to_string(),
            url: "https://example.com".to_string(),
            source: None,
            tags: None,
        });
        assert!(result.is_err(), "empty title should fail");
        assert!(
            result.unwrap_err().contains("title"),
            "error should mention title"
        );

        // Empty install_command should fail
        let result = skills::add_custom_skill(NewCustomSkill {
            id: None,
            title: "Valid Title".to_string(),
            description: "desc".to_string(),
            install_command: "  ".to_string(),
            url: "https://example.com".to_string(),
            source: None,
            tags: None,
        });
        assert!(result.is_err(), "empty install_command should fail");

        // Empty url should fail
        let result = skills::add_custom_skill(NewCustomSkill {
            id: None,
            title: "Valid Title".to_string(),
            description: "desc".to_string(),
            install_command: "echo test".to_string(),
            url: "".to_string(),
            source: None,
            tags: None,
        });
        assert!(result.is_err(), "empty url should fail");

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_remove_custom_skill() {
        let (skills_path, _guard) = isolated_skills_path();

        // Add a skill first
        skills::add_custom_skill(NewCustomSkill {
            id: Some("removable-skill".to_string()),
            title: "Removable".to_string(),
            description: "To be removed".to_string(),
            install_command: "echo".to_string(),
            url: "https://example.com".to_string(),
            source: None,
            tags: None,
        })
        .unwrap();

        // Verify it exists
        let all = skills::list_all_skills();
        assert!(all.iter().any(|s| s.id == "removable-skill"));

        // Remove it
        let removed = skills::remove_custom_skill("removable-skill").unwrap();
        assert!(removed, "should return true when skill was removed");

        // Verify it's gone
        let all = skills::list_all_skills();
        assert!(
            !all.iter().any(|s| s.id == "removable-skill"),
            "skill should be gone"
        );

        // Remove again should return false
        let removed_again = skills::remove_custom_skill("removable-skill").unwrap();
        assert!(
            !removed_again,
            "should return false for already-removed skill"
        );

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_remove_nonexistent_returns_false() {
        let (skills_path, _guard) = isolated_skills_path();

        let removed = skills::remove_custom_skill("does-not-exist").unwrap();
        assert!(!removed);

        // Empty id returns false
        let removed = skills::remove_custom_skill("").unwrap();
        assert!(!removed);

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_search_exact_match() {
        let (skills_path, _guard) = isolated_skills_path();

        let results = skills::search_skills("find-skills", None, 25);
        assert!(!results.is_empty(), "should find 'find-skills'");
        assert!(results.iter().any(|s| s.id == "find-skills"));

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_search_partial_match() {
        let (skills_path, _guard) = isolated_skills_path();

        let results = skills::search_skills("react", None, 25);
        assert!(!results.is_empty(), "should find skills matching 'react'");
        assert!(results
            .iter()
            .any(|s| s.id == "vercel-react-best-practices"));

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_search_tokenized_match() {
        let (skills_path, _guard) = isolated_skills_path();

        let results = skills::search_skills("find skills", None, 25);
        assert!(
            !results.is_empty(),
            "tokenized search for 'find skills' should return results"
        );

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_search_by_source_filter() {
        let (skills_path, _guard) = isolated_skills_path();

        let vercel_only = skills::search_skills("", Some("vercel"), 25);
        for skill in &vercel_only {
            assert_eq!(
                skill.source.to_ascii_lowercase(),
                "vercel",
                "source filter should only return vercel skills, got {}",
                skill.source
            );
        }

        let builtin_only = skills::search_skills("", Some("builtin"), 25);
        for skill in &builtin_only {
            assert_eq!(skill.source.to_ascii_lowercase(), "builtin");
        }

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_search_no_results_returns_fallback() {
        let (skills_path, _guard) = isolated_skills_path();

        let results = skills::search_skills("xyznonexistentthing123", None, 25);
        // Should return fallback entry (find-skills) not an empty vec
        assert!(
            !results.is_empty(),
            "search with no match should return fallback entries"
        );
        let ids: Vec<&str> = results.iter().map(|s| s.id.as_str()).collect();
        assert!(
            ids.contains(&"find-skills"),
            "fallback should include find-skills, got: {:?}",
            ids
        );

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_search_respects_limit() {
        let (skills_path, _guard) = isolated_skills_path();

        let results = skills::search_skills("", None, 1);
        assert_eq!(results.len(), 1, "limit=1 should return exactly 1 result");

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_import_context7_valid() {
        let (skills_path, _guard) = isolated_skills_path();

        let skill = skills::import_context7_skill("/tauri-apps/tauri", None).unwrap();
        assert!(
            skill.id.starts_with("context7-"),
            "id should start with 'context7-': {}",
            skill.id
        );
        assert!(skill.is_custom);
        assert_eq!(skill.source, "context7");
        assert!(skill.url.contains("context7.com"));
        assert!(skill.title.contains("tauri-apps"));

        // Should be persisted
        let all = skills::list_all_skills();
        assert!(all.iter().any(|s| s.id == skill.id));

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_import_context7_with_custom_title() {
        let (skills_path, _guard) = isolated_skills_path();

        let skill = skills::import_context7_skill("/facebook/react", Some("React Docs")).unwrap();
        assert_eq!(skill.title, "React Docs");

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_import_context7_invalid_format() {
        let (skills_path, _guard) = isolated_skills_path();

        // No leading slash
        let result = skills::import_context7_skill("tauri-apps/tauri", None);
        assert!(result.is_err());

        // Too few segments
        let result = skills::import_context7_skill("/onlyone", None);
        assert!(result.is_err());

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_import_vercel_valid() {
        let (skills_path, _guard) = isolated_skills_path();

        let skill = skills::import_vercel_skill("next-auth-setup").unwrap();
        assert_eq!(skill.id, "next-auth-setup");
        assert!(skill.is_custom);
        assert_eq!(skill.source, "vercel");
        assert!(skill.install_command.contains("next-auth-setup"));
        assert!(skill.url.contains("skills.sh"));
        assert_eq!(skill.title, "Vercel: Next Auth Setup");

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_import_vercel_with_at_prefix() {
        let (skills_path, _guard) = isolated_skills_path();

        // "vercel-labs/agent-skills@my-skill" should extract "my-skill"
        let skill = skills::import_vercel_skill("vercel-labs/agent-skills@my-skill").unwrap();
        assert_eq!(skill.id, "my-skill");

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_import_vercel_empty_fails() {
        let (skills_path, _guard) = isolated_skills_path();

        let result = skills::import_vercel_skill("");
        assert!(result.is_err());

        let result = skills::import_vercel_skill("  ");
        assert!(result.is_err());

        cleanup_skills_env(&skills_path);
    }

    #[test]
    fn test_skills_upsert_replaces_existing() {
        let (skills_path, _guard) = isolated_skills_path();

        // Add a skill
        skills::add_custom_skill(NewCustomSkill {
            id: Some("upsert-test".to_string()),
            title: "Original Title".to_string(),
            description: "Original".to_string(),
            install_command: "echo original".to_string(),
            url: "https://example.com/original".to_string(),
            source: None,
            tags: None,
        })
        .unwrap();

        // Upsert with same id but different title
        skills::add_custom_skill(NewCustomSkill {
            id: Some("upsert-test".to_string()),
            title: "Updated Title".to_string(),
            description: "Updated".to_string(),
            install_command: "echo updated".to_string(),
            url: "https://example.com/updated".to_string(),
            source: None,
            tags: None,
        })
        .unwrap();

        let all = skills::list_all_skills();
        let found: Vec<_> = all.iter().filter(|s| s.id == "upsert-test").collect();
        assert_eq!(found.len(), 1, "should have exactly 1 entry after upsert");
        assert_eq!(found[0].title, "Updated Title");

        cleanup_skills_env(&skills_path);
    }

    // Tool-based skills tests
    #[test]
    fn test_tool_skills_list_returns_skills() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        let output = registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.list".to_string(),
                    args: serde_json::json!({}),
                },
            )
            .expect("skills.list should succeed");

        assert!(output.ok);
        let skills = output
            .data
            .get("skills")
            .and_then(|v| v.as_array())
            .expect("should have skills array");
        assert!(skills.len() >= 2, "should have at least 2 skills");

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn test_tool_skills_list_includes_find_skills_entry() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        let output = registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.list".to_string(),
                    args: serde_json::json!({}),
                },
            )
            .expect("skills.list should succeed");

        assert!(output.ok);
        let skills = output
            .data
            .get("skills")
            .and_then(|v| v.as_array())
            .expect("should have skills array");
        assert!(
            skills
                .iter()
                .any(|skill| { skill.get("id").and_then(|v| v.as_str()) == Some("find-skills") }),
            "skills.list should include find-skills entry"
        );

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn test_tool_skills_list_contains_builtin_entries() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        let output = registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.list".to_string(),
                    args: serde_json::json!({}),
                },
            )
            .expect("skills.list should succeed");

        assert!(output.ok);
        let skills = output
            .data
            .get("skills")
            .and_then(|v| v.as_array())
            .expect("should have skills array");
        assert!(skills
            .iter()
            .any(|skill| { skill.get("source").and_then(|v| v.as_str()) == Some("builtin") }));

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn test_tool_skills_load_custom_mode() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        let output = registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.load".to_string(),
                    args: serde_json::json!({
                        "mode": "custom",
                        "title": "Tool-Loaded Skill",
                        "description": "Added via skills.load tool",
                        "install_command": "echo loaded",
                        "url": "https://example.com/tool-loaded"
                    }),
                },
            )
            .expect("skills.load custom should succeed");

        assert!(output.ok);
        let loaded = output
            .data
            .get("skill")
            .expect("should have skill in response");
        assert_eq!(
            loaded.get("title").and_then(|v| v.as_str()).unwrap(),
            "Tool-Loaded Skill"
        );
        assert!(loaded.get("is_custom").and_then(|v| v.as_bool()).unwrap());

        // Verify it's now in the list
        let list_out = registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.list".to_string(),
                    args: serde_json::json!({}),
                },
            )
            .unwrap();
        let skills = list_out
            .data
            .get("skills")
            .and_then(|v| v.as_array())
            .unwrap();
        assert!(skills
            .iter()
            .any(|s| s.get("title").and_then(|v| v.as_str()) == Some("Tool-Loaded Skill")));

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn test_tool_skills_load_context7_mode() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        let output = registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.load".to_string(),
                    args: serde_json::json!({
                        "mode": "context7",
                        "library_id": "/vercel/next.js",
                        "title": "Next.js Docs"
                    }),
                },
            )
            .expect("skills.load context7 should succeed");

        assert!(output.ok);
        let loaded = output.data.get("skill").unwrap();
        assert_eq!(
            loaded.get("source").and_then(|v| v.as_str()).unwrap(),
            "context7"
        );
        assert_eq!(
            loaded.get("title").and_then(|v| v.as_str()).unwrap(),
            "Next.js Docs"
        );

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn test_tool_skills_load_vercel_mode() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        let output = registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.load".to_string(),
                    args: serde_json::json!({
                        "mode": "vercel",
                        "skill_name": "eslint-config"
                    }),
                },
            )
            .expect("skills.load vercel should succeed");

        assert!(output.ok);
        let loaded = output.data.get("skill").unwrap();
        assert_eq!(
            loaded.get("source").and_then(|v| v.as_str()).unwrap(),
            "vercel"
        );
        assert_eq!(
            loaded.get("id").and_then(|v| v.as_str()).unwrap(),
            "eslint-config"
        );

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn test_tool_skills_load_custom_missing_required_field() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        // Missing title
        let result = registry.invoke(
            &policy,
            &cwd,
            ToolCallInput {
                name: "skills.load".to_string(),
                args: serde_json::json!({
                    "mode": "custom",
                    "install_command": "echo test",
                    "url": "https://example.com"
                }),
            },
        );
        assert!(
            result.is_err(),
            "skills.load custom without title should fail"
        );

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn test_tool_skills_load_context7_missing_library_id() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        let result = registry.invoke(
            &policy,
            &cwd,
            ToolCallInput {
                name: "skills.load".to_string(),
                args: serde_json::json!({"mode": "context7"}),
            },
        );
        assert!(
            result.is_err(),
            "skills.load context7 without library_id should fail"
        );

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn test_tool_skills_remove_existing() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        // First load a skill
        registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.load".to_string(),
                    args: serde_json::json!({
                        "mode": "custom",
                        "id": "to-remove-via-tool",
                        "title": "Remove Me",
                        "install_command": "echo rm",
                        "url": "https://example.com/rm"
                    }),
                },
            )
            .unwrap();

        // Now remove it
        let output = registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.remove".to_string(),
                    args: serde_json::json!({"skill_id": "to-remove-via-tool"}),
                },
            )
            .expect("skills.remove should succeed");

        assert!(output.ok);
        assert_eq!(
            output.data.get("removed").and_then(|v| v.as_bool()),
            Some(true)
        );

        // Verify it's gone
        let list_out = registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.list".to_string(),
                    args: serde_json::json!({}),
                },
            )
            .unwrap();
        let skills = list_out
            .data
            .get("skills")
            .and_then(|v| v.as_array())
            .unwrap();
        assert!(!skills
            .iter()
            .any(|s| s.get("id").and_then(|v| v.as_str()) == Some("to-remove-via-tool")));

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn test_tool_skills_remove_nonexistent() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        let output = registry
            .invoke(
                &policy,
                &cwd,
                ToolCallInput {
                    name: "skills.remove".to_string(),
                    args: serde_json::json!({"skill_id": "no-such-skill"}),
                },
            )
            .expect("skills.remove should succeed even for nonexistent");

        assert!(output.ok);
        assert_eq!(
            output.data.get("removed").and_then(|v| v.as_bool()),
            Some(false)
        );

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn test_tool_skills_remove_missing_skill_id() {
        let (skills_path, _guard) = isolated_skills_path();

        let registry = ToolRegistry::default();
        let policy = PolicyEngine::new(temp_workspace());
        let cwd = temp_workspace();

        let result = registry.invoke(
            &policy,
            &cwd,
            ToolCallInput {
                name: "skills.remove".to_string(),
                args: serde_json::json!({}),
            },
        );
        assert!(
            result.is_err(),
            "skills.remove without skill_id should fail"
        );

        cleanup_skills_env(&skills_path);
        let _ = std::fs::remove_dir_all(&cwd);
    }
}
