use unoskills_core::{
    DeployMode, UnoSkillsManager, domain::AgentCategory, registry::CustomAgentRecord,
};

fn create_skill_dir(path: &std::path::Path, name: &str, desc: &str, tags: &[&str]) {
    std::fs::create_dir_all(path).unwrap();
    let tag_lines: String = tags.iter().map(|t| format!("  - {t}\n")).collect();
    let md = format!(
        "---\nname: {name}\ndescription: {desc}\ntags:\n{tag_lines}---\n# {name}\nBody instructions for {name}.\n"
    );
    std::fs::write(path.join("SKILL.md"), md).unwrap();
    std::fs::write(path.join("extra.txt"), "supporting asset").unwrap();
}

#[test]
fn adopt_symlinked_skill_preserves_files_and_does_not_create_empty_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();

    // Real skill lives externally, and agent's skills_dir holds a symlink to it
    // (common with `npx skills add` or other skill managers).
    let external_skill = tmp.path().join("external-repo").join("my-linked-skill");
    create_skill_dir(
        &external_skill,
        "my-linked-skill",
        "Linked skill",
        &["link"],
    );

    let agent_dir = tmp.path().join("agent-skills");
    std::fs::create_dir_all(&agent_dir).unwrap();
    let agent_link = agent_dir.join("my-linked-skill");
    unoskills_core::sync::deploy_skill_dir(
        &external_skill,
        &agent_link,
        DeployMode::Symlink,
        unoskills_core::sync::ReplacePolicy::UserConfirmed,
    )
    .unwrap();

    mgr.add_custom_agent(&CustomAgentRecord {
        key: "custom_link_agent".into(),
        display_name: "Link Agent".into(),
        skills_dir: agent_dir.to_string_lossy().into(),
        category: AgentCategory::Coding,
    })
    .unwrap();

    let candidates = mgr.scan_adoptable().unwrap();
    let cand = candidates
        .iter()
        .find(|c| c.agent_key == "custom_link_agent" && c.name == "my-linked-skill")
        .expect("symlinked skill should be discovered as adoptable");

    let adopted = mgr.adopt_candidate(cand).unwrap();
    assert!(
        adopted.central_path.join("SKILL.md").is_file(),
        "adopted central copy must contain SKILL.md (not an empty dir!)"
    );
    assert!(
        adopted.central_path.join("extra.txt").is_file(),
        "adopted central copy must preserve supporting files"
    );

    // After adoption, it must no longer appear as unmanaged.
    let after = mgr.scan_adoptable().unwrap();
    assert!(
        !after
            .iter()
            .any(|c| c.agent_key == "custom_link_agent" && c.name == "my-linked-skill"),
        "adopted skill should disappear from scan_adoptable"
    );
}

#[test]
fn adopt_from_additional_scan_dir_cleans_up_and_disappears_from_adopt_list() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();

    let primary_dir = tmp.path().join("agent-primary");
    let extra_dir = tmp.path().join("agent-extra");
    std::fs::create_dir_all(&primary_dir).unwrap();
    let extra_skill = extra_dir.join("extra-dir-skill");
    create_skill_dir(&extra_skill, "extra-dir-skill", "From extra scan dir", &[]);

    mgr.add_custom_agent(&CustomAgentRecord {
        key: "custom_extra_agent".into(),
        display_name: "Extra Agent".into(),
        skills_dir: primary_dir.to_string_lossy().into(),
        category: AgentCategory::Coding,
    })
    .unwrap();

    // Simulate a candidate discovered in an additional scan dir (e.g., Codex scanning ~/.agents/skills)
    let cand = unoskills_core::AdoptCandidate {
        agent_key: "custom_extra_agent".into(),
        agent_name: "Extra Agent".into(),
        name: "extra-dir-skill".into(),
        description: Some("From extra scan dir".into()),
        path: extra_skill.clone(),
        estimated_tokens: 20,
    };

    let adopted = mgr.adopt_candidate(&cand).unwrap();
    assert!(adopted.central_path.join("SKILL.md").is_file());
    // The original extra_skill path should now point to central_path so re-scanning won't treat it as unmanaged!
    let canon_extra = extra_skill.canonicalize().unwrap();
    let canon_central = adopted.central_path.canonicalize().unwrap();
    assert_eq!(
        canon_extra, canon_central,
        "adopted candidate in additional_scan_dir must be linked to central_path so it stops reappearing in scan_adoptable"
    );
}

#[test]
fn self_import_from_central_library_refuses_instead_of_deleting_source() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();

    let src = tmp.path().join("alpha");
    create_skill_dir(&src, "alpha", "Alpha skill", &["test"]);
    let installed = mgr.install_from_ref(&src.to_string_lossy()).unwrap();

    // Re-importing the central_path onto itself must NOT delete SKILL.md!
    let res = mgr.install_from_ref(&installed.central_path.to_string_lossy());
    assert!(
        res.is_err(),
        "re-importing central_path onto itself must be rejected safely"
    );
    assert!(
        installed.central_path.join("SKILL.md").is_file(),
        "central SKILL.md must still exist after rejected self-import"
    );
}

#[test]
fn save_skill_markdown_updates_tags_and_description_accurately() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();

    let src = tmp.path().join("edit-me");
    create_skill_dir(&src, "edit-me", "Old desc", &["oldtag"]);
    let skill = mgr.install_from_ref(&src.to_string_lossy()).unwrap();
    assert_eq!(skill.tags, vec!["oldtag"]);

    let updated = mgr
        .save_skill_markdown(
            &skill.id,
            "---\nname: edit-me-v2\ntags:\n  - rust\n  - gpui\n---\n# Body\nUpdated content.\n",
        )
        .unwrap();
    assert_eq!(updated.name, "edit-me-v2");
    assert_eq!(
        updated.description, None,
        "removed description must not stick around"
    );
    assert_eq!(
        updated.tags,
        vec!["rust", "gpui"],
        "edited tags must persist"
    );
}

#[test]
fn shared_dir_copy_mode_allows_second_agent_and_preserves_until_last_undeploy() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();

    let src = tmp.path().join("copy-skill");
    create_skill_dir(&src, "copy-skill", "Copy mode shared dir", &[]);
    let skill = mgr.install_from_ref(&src.to_string_lossy()).unwrap();

    let shared_dir = tmp.path().join("shared-copy-dir");
    mgr.add_custom_agent(&CustomAgentRecord {
        key: "copy_a".into(),
        display_name: "Copy A".into(),
        skills_dir: shared_dir.to_string_lossy().into(),
        category: AgentCategory::Coding,
    })
    .unwrap();
    mgr.add_custom_agent(&CustomAgentRecord {
        key: "copy_b".into(),
        display_name: "Copy B".into(),
        skills_dir: shared_dir.to_string_lossy().into(),
        category: AgentCategory::Coding,
    })
    .unwrap();

    mgr.deploy_skill(&skill.id, "copy_a", Some(DeployMode::Copy), false)
        .unwrap();
    // Second agent sharing the same dir in Copy mode must NOT fail with TARGET_CONFLICT!
    mgr.deploy_skill(&skill.id, "copy_b", Some(DeployMode::Copy), false)
        .unwrap();

    mgr.undeploy_skill(&skill.id, "copy_a").unwrap();
    assert!(
        shared_dir.join(&skill.slug).join("SKILL.md").is_file(),
        "copy_b still uses shared dir"
    );

    mgr.undeploy_skill(&skill.id, "copy_b").unwrap();
    assert!(!shared_dir.join(&skill.slug).exists());
}

#[test]
fn deploy_no_clobber_protects_unmanaged_dir_and_force_overrides() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();

    let src = tmp.path().join("conflict-skill");
    create_skill_dir(&src, "conflict-skill", "Managed", &[]);
    let skill = mgr.install_from_ref(&src.to_string_lossy()).unwrap();

    let agent_dir = tmp.path().join("agent-conflict");
    let unmanaged = agent_dir.join(&skill.slug);
    std::fs::create_dir_all(&unmanaged).unwrap();
    std::fs::write(unmanaged.join("USER_NOTES.txt"), "precious").unwrap();

    mgr.add_custom_agent(&CustomAgentRecord {
        key: "conflict_agent".into(),
        display_name: "Conflict Agent".into(),
        skills_dir: agent_dir.to_string_lossy().into(),
        category: AgentCategory::Coding,
    })
    .unwrap();

    // Default force=false must fail with TARGET_CONFLICT and keep USER_NOTES.txt intact
    let err = mgr
        .deploy_skill(
            &skill.id,
            "conflict_agent",
            Some(DeployMode::Symlink),
            false,
        )
        .unwrap_err();
    assert!(err.to_string().contains("TARGET_CONFLICT"));
    assert!(unmanaged.join("USER_NOTES.txt").is_file());

    // Explicit force=true replaces it cleanly
    mgr.deploy_skill(&skill.id, "conflict_agent", Some(DeployMode::Symlink), true)
        .unwrap();
    assert!(unmanaged.join("SKILL.md").is_file());
}

#[test]
fn update_skill_detects_noop_and_refreshes_copy_deployments_on_change() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();

    let src = tmp.path().join("updatable-skill");
    create_skill_dir(&src, "updatable-skill", "v1 description", &["v1"]);
    let skill = mgr.install_from_ref(&src.to_string_lossy()).unwrap();

    let copy_agent_dir = tmp.path().join("copy-agent-skills");
    mgr.add_custom_agent(&CustomAgentRecord {
        key: "copy_upd_agent".into(),
        display_name: "Copy Update Agent".into(),
        skills_dir: copy_agent_dir.to_string_lossy().into(),
        category: AgentCategory::Coding,
    })
    .unwrap();
    mgr.deploy_skill(&skill.id, "copy_upd_agent", Some(DeployMode::Copy), false)
        .unwrap();

    // Unchanged source -> updated == false
    let noop = mgr.update_skill(&skill.id).unwrap();
    assert!(!noop.updated);
    assert_eq!(noop.refreshed_copies, 0);

    // Mutate source -> updated == true and Copy deployment gets refreshed automatically!
    create_skill_dir(&src, "updatable-skill", "v2 description", &["v2"]);
    let upgraded = mgr.update_skill(&skill.id).unwrap();
    assert!(upgraded.updated);
    assert_eq!(upgraded.refreshed_copies, 1);
    assert_eq!(
        upgraded.skill.description.as_deref(),
        Some("v2 description")
    );

    let deployed_md =
        std::fs::read_to_string(copy_agent_dir.join(&skill.slug).join("SKILL.md")).unwrap();
    assert!(deployed_md.contains("v2 description"));
}

#[test]
fn custom_agent_and_network_settings_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();

    mgr.set_http_proxy(Some("http://127.0.0.1:7890")).unwrap();
    mgr.set_github_token(Some("ghp_example_token")).unwrap();
    assert_eq!(mgr.http_proxy().as_deref(), Some("http://127.0.0.1:7890"));
    assert_eq!(mgr.github_token().as_deref(), Some("ghp_example_token"));

    mgr.set_http_proxy(None).unwrap();
    assert_eq!(mgr.http_proxy(), None);

    let dir = tmp.path().join("my-custom-agent-skills");
    mgr.add_custom_agent(&CustomAgentRecord {
        key: "my_custom".into(),
        display_name: "My Custom Agent".into(),
        skills_dir: dir.to_string_lossy().into(),
        category: AgentCategory::Coding,
    })
    .unwrap();
    assert!(
        mgr.list_agents()
            .unwrap()
            .iter()
            .any(|a| a.key == "my_custom" && a.is_custom)
    );

    mgr.remove_custom_agent("my_custom").unwrap();
    assert!(
        !mgr.list_agents()
            .unwrap()
            .iter()
            .any(|a| a.key == "my_custom")
    );
}

#[test]
#[ignore = "hits live skills.sh and GitHub network endpoints"]
fn live_network_marketplace_and_featured_stack_smoke() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();

    for board in ["trending", "hot", "all-time"] {
        let p0 = mgr
            .marketplace_leaderboard_page(board, 0)
            .unwrap_or_else(|e| panic!("board {board} page 0 failed: {e}"));
        println!(
            "board={board} p0: {} items, total={:?}, has_more={} (first: {:?})",
            p0.items.len(),
            p0.total,
            p0.has_more,
            p0.items.first().map(|i| i.install_ref())
        );
        assert!(
            !p0.items.is_empty(),
            "board {board} page 0 returned 0 items!"
        );
    }
    let p0 = mgr.marketplace_leaderboard_page("trending", 0).unwrap();
    let p1 = mgr.marketplace_leaderboard_page("trending", 1).unwrap();
    assert!(p0.has_more);
    assert!(!p1.items.is_empty());
    assert_ne!(
        p0.items[0].id, p1.items[0].id,
        "page 1 should differ from page 0"
    );

    let search = mgr.marketplace_search("react", 10).unwrap();
    println!(
        "search 'react': {} items (first: {:?})",
        search.len(),
        search.first().map(|i| i.install_ref())
    );
    assert!(!search.is_empty(), "search 'react' returned 0 items!");

    // Test installing one real skill from featured_stacks.toml!
    let stacks = mgr.featured_stacks().unwrap();
    let first_skill_ref = &stacks[0].skills[0].source;
    println!("installing real featured skill: {first_skill_ref}");
    let installed = mgr.install_from_ref(first_skill_ref).unwrap();
    println!(
        "installed {} (~{} tokens) at {}",
        installed.name,
        installed.estimated_tokens,
        installed.central_path.display()
    );
    assert!(installed.central_path.join("SKILL.md").is_file());
}

#[test]
fn project_scope_deploy_shared_dir_update_and_detach_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    let src = tmp.path().join("src/proj-skill");
    create_skill_dir(&src, "proj-skill", "Project scope skill v1", &["project"]);

    let skill = mgr.install_from_ref(&src.to_string_lossy()).unwrap();

    // Attach a mock repo directory as a Project Workspace
    let repo_dir = tmp.path().join("my-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    let proj = mgr.add_project(&repo_dir, None).unwrap();
    assert_eq!(proj.name, "my-repo");
    assert_eq!(mgr.list_projects().unwrap().len(), 1);

    // Pre-create an unmanaged conflict in <repo>/.claude/skills/proj-skill
    let claude_target = repo_dir.join(".claude/skills/proj-skill");
    std::fs::create_dir_all(&claude_target).unwrap();
    std::fs::write(claude_target.join("CUSTOM.md"), "user file").unwrap();

    let err = mgr
        .deploy_skill_to_project(
            &skill.id,
            &proj.id,
            "claude_code",
            Some(DeployMode::Copy),
            false,
        )
        .expect_err("must refuse to overwrite unmanaged project target");
    assert!(err.to_string().contains("TARGET_CONFLICT"));

    // Force-deploy in Copy mode resolves the conflict
    let dep_claude = mgr
        .deploy_skill_to_project(
            &skill.id,
            &proj.id,
            "claude_code",
            Some(DeployMode::Copy),
            true,
        )
        .unwrap();
    assert_eq!(dep_claude.mode, DeployMode::Copy);
    assert!(claude_target.join("SKILL.md").exists());

    // Deploy two agents sharing `.config/agents/skills` inside the same project (amp & replit)
    let dep_amp = mgr
        .deploy_skill_to_project(&skill.id, &proj.id, "amp", Some(DeployMode::Copy), false)
        .unwrap();
    let dep_replit = mgr
        .deploy_skill_to_project(&skill.id, &proj.id, "replit", Some(DeployMode::Copy), false)
        .unwrap();
    assert_eq!(dep_amp.target_path, dep_replit.target_path);
    assert!(dep_amp.target_path.join("SKILL.md").exists());

    // Upstream update refreshes project Copy deployments too
    create_skill_dir(&src, "proj-skill", "Project scope skill v2", &["project"]);
    let update = mgr.update_skill(&skill.id).unwrap();
    assert!(update.updated);
    assert!(update.refreshed_copies >= 2);
    let refreshed = std::fs::read_to_string(claude_target.join("SKILL.md")).unwrap();
    assert!(refreshed.contains("Project scope skill v2"));

    // Undeploying one shared-dir agent keeps the directory for the sibling agent
    mgr.undeploy_skill_from_project(&skill.id, &proj.id, "amp")
        .unwrap();
    assert!(dep_amp.target_path.exists());

    // Detaching the project cleanly undeploys remaining project skills while keeping central skill
    mgr.remove_project(&proj.id).unwrap();
    assert!(mgr.list_projects().unwrap().is_empty());
    assert!(!claude_target.exists());
    assert!(!dep_amp.target_path.exists());
    assert!(skill.central_path.join("SKILL.md").exists());
}

#[test]
fn find_any_stack_resolves_local_and_featured_without_network() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    // Point remote index at an unreachable localhost port so any accidental
    // network call would fail/stall.
    mgr.set_setting("stacks_registry_url", "http://127.0.0.1:1/unreachable.json")
        .unwrap();

    let featured = mgr.find_any_stack("frontend-react-ship").unwrap();
    assert!(featured.is_some());

    let src = tmp.path().join("src/local-s");
    create_skill_dir(&src, "local-s", "Local stack skill", &["local"]);
    let skill = mgr.install_from_ref(&src.to_string_lossy()).unwrap();
    let local = mgr
        .create_stack_from_skills("My Local Combo", "me", "desc", vec![], &[skill.id])
        .unwrap();
    let found_local = mgr.find_any_stack(&local.id).unwrap();
    assert!(found_local.is_some());
}

#[test]
fn community_index_cache_invalidates_when_registry_url_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    let cached_body = r#"[
        {
            "id": "cached-from-url-a",
            "name": "Cached From URL A",
            "author": "tester",
            "tags": ["test"],
            "skills": [
                {"name": "skill-creator", "source": "anthropics/skills@skill-creator"}
            ]
        }
    ]"#;
    mgr.set_setting("stacks_registry_url", "http://127.0.0.1:1/url-a.json")
        .unwrap();
    mgr.set_setting("community_index_url", "http://127.0.0.1:1/url-a.json")
        .unwrap();
    mgr.set_setting("community_index_etag", "\"etag-a\"")
        .unwrap();
    mgr.set_setting("community_index_cache", cached_body)
        .unwrap();

    // Offline fallback works when URL matches the cached URL.
    let from_a = mgr.community_stacks().unwrap();
    assert_eq!(from_a.len(), 1);
    assert_eq!(from_a[0].id, "cached-from-url-a");

    // Switching to a different unreachable URL must NOT serve URL A's stale cache.
    mgr.set_setting("stacks_registry_url", "http://127.0.0.1:1/url-b.json")
        .unwrap();
    assert!(
        mgr.community_stacks().is_err(),
        "stale cache from URL A must not be served for URL B"
    );
}

#[test]
fn git_backup_push_and_pull_rebuilds_sqlite_and_refreshes_copies_without_syncing_db() {
    let tmp = tempfile::tempdir().unwrap();
    let bare_repo = tmp.path().join("remote-backup.git");
    std::fs::create_dir_all(&bare_repo).unwrap();
    let init_status = std::process::Command::new("git")
        .current_dir(&bare_repo)
        .args(["init", "--bare"])
        .status()
        .expect("git must be installed");
    assert!(init_status.success());

    let repo_url = bare_repo.to_string_lossy().to_string();

    // Machine A: install 2 skills + 1 local stack, push to bare repo
    let mgr_a = UnoSkillsManager::open_at(tmp.path().join("machine-a")).unwrap();
    let src_alpha = tmp.path().join("src/alpha-skill");
    let src_beta = tmp.path().join("src/beta-skill");
    create_skill_dir(&src_alpha, "alpha-skill", "Alpha v1", &["sync"]);
    create_skill_dir(&src_beta, "beta-skill", "Beta v1", &["sync"]);
    let alpha = mgr_a
        .install_from_ref(&src_alpha.to_string_lossy())
        .unwrap();
    let beta = mgr_a.install_from_ref(&src_beta.to_string_lossy()).unwrap();
    mgr_a
        .create_stack_from_skills(
            "Sync Combo",
            "alice",
            "Portable stack",
            vec!["sync".into()],
            &[alpha.id.clone(), beta.id.clone()],
        )
        .unwrap();

    mgr_a.configure_git_backup(&repo_url, Some("main")).unwrap();
    let push1 = mgr_a.git_backup_push().unwrap();
    assert!(push1.committed);
    assert!(push1.pushed);
    assert_eq!(push1.skills_exported, 2);
    assert_eq!(push1.stacks_exported, 1);

    // Verify bare repo contains ONLY plain-text skills & stacks and ZERO .db files
    let tree_out = std::process::Command::new("git")
        .current_dir(&bare_repo)
        .args(["ls-tree", "-r", "--name-only", "refs/heads/main"])
        .output()
        .unwrap();
    assert!(tree_out.status.success());
    let tree_list = String::from_utf8_lossy(&tree_out.stdout);
    assert!(tree_list.contains("skills/alpha-skill/SKILL.md"));
    assert!(tree_list.contains("skills/beta-skill/SKILL.md"));
    assert!(tree_list.contains("stacks/sync-combo.toml"));
    assert!(
        !tree_list.contains(".db"),
        "binary SQLite DB must never be committed to backup repo"
    );

    // Second push with no changes is a clean no-op
    let push_noop = mgr_a.git_backup_push().unwrap();
    assert!(!push_noop.committed);
    assert!(!push_noop.pushed);

    // Machine B: fresh empty UnoSkills library pulls from the bare repo and rebuilds SQLite
    let mgr_b = UnoSkillsManager::open_at(tmp.path().join("machine-b")).unwrap();
    mgr_b.configure_git_backup(&repo_url, Some("main")).unwrap();
    let pull1 = mgr_b.git_backup_pull().unwrap();
    assert_eq!(pull1.skills_imported, 2);
    assert_eq!(pull1.skills_updated, 0);
    assert_eq!(pull1.stacks_imported, 1);
    assert_eq!(mgr_b.list_skills().unwrap().len(), 2);
    assert_eq!(mgr_b.list_local_stacks().unwrap().len(), 1);

    // Deploy alpha-skill on Machine B in Copy mode via a custom agent dir
    let custom_dir_b = tmp.path().join("machine-b-agent/skills");
    mgr_b
        .add_custom_agent(&CustomAgentRecord {
            key: "sync_agent".into(),
            display_name: "SyncAgent".into(),
            skills_dir: custom_dir_b.to_string_lossy().to_string(),
            category: AgentCategory::Coding,
        })
        .unwrap();
    let alpha_b = mgr_b.find_skill("alpha-skill").unwrap().unwrap();
    mgr_b
        .deploy_skill(&alpha_b.id, "sync_agent", Some(DeployMode::Copy), false)
        .unwrap();

    // Machine A updates alpha-skill and pushes again
    mgr_a
        .save_skill_markdown(
            &alpha.id,
            "---\nname: alpha-skill\ndescription: Alpha v2 updated\ntags:\n  - sync\n---\n# Alpha v2\n",
        )
        .unwrap();
    let push2 = mgr_a.git_backup_push().unwrap();
    assert!(push2.committed);
    assert!(push2.pushed);

    // Machine B pulls again: alpha updates & refreshes its Copy deployment, beta stays unchanged
    let pull2 = mgr_b.git_backup_pull().unwrap();
    assert_eq!(pull2.skills_imported, 0);
    assert_eq!(pull2.skills_updated, 1);
    assert_eq!(pull2.skills_unchanged, 1);
    assert_eq!(pull2.refreshed_copies, 1);
    let copied_md = std::fs::read_to_string(custom_dir_b.join("alpha-skill/SKILL.md")).unwrap();
    assert!(copied_md.contains("Alpha v2 updated"));
}
