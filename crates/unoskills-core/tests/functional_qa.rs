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
    #[cfg(unix)]
    std::os::unix::fs::symlink(&external_skill, &agent_link).unwrap();

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
