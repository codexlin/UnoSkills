use unoskills_core::{
    DeployMode, DeploymentImpactKind, StackSkillRef, UnoSkillsManager, domain::AgentCategory,
    registry::CustomAgentRecord,
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
fn install_preserves_the_existing_library_directory_on_slug_collision() {
    let tmp = tempfile::tempdir().unwrap();
    let manager = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    let first = tmp.path().join("first/shared");
    let second = tmp.path().join("second/shared");
    create_skill_dir(&first, "First Skill", "Original skill", &[]);
    create_skill_dir(&second, "Second Skill", "Different source", &[]);
    let installed = manager.install_from_ref(first.to_str().unwrap()).unwrap();
    let notes = installed.central_path.join("local-notes.txt");
    std::fs::write(&notes, "local changes").unwrap();

    let error = manager
        .install_from_ref(second.to_str().unwrap())
        .unwrap_err();
    assert!(error.to_string().contains("SKILL_SLUG_CONFLICT"));
    let again = manager.install_from_ref(first.to_str().unwrap()).unwrap();
    assert_eq!(again.id, installed.id);
    assert_eq!(again.name, "First Skill");
    assert_eq!(std::fs::read_to_string(notes).unwrap(), "local changes");

    let orphan = manager.skills_dir().join("orphan");
    create_skill_dir(&orphan, "Unindexed Skill", "Unindexed directory", &[]);
    let candidate = tmp.path().join("second/orphan");
    create_skill_dir(&candidate, "Candidate", "Unrelated skill", &[]);
    let error = manager
        .install_from_ref(candidate.to_str().unwrap())
        .unwrap_err();
    assert!(error.to_string().contains("SKILL_SLUG_CONFLICT"));
    assert!(
        std::fs::read_to_string(orphan.join("SKILL.md"))
            .unwrap()
            .contains("Unindexed Skill")
    );
}

#[test]
fn edited_deployment_copies_block_undeploy_and_bulk_removal() {
    let tmp = tempfile::tempdir().unwrap();
    let manager = UnoSkillsManager::open_at(tmp.path().join("library")).unwrap();
    let source = tmp.path().join("source");
    create_skill_dir(&source, "Source", "Original", &[]);
    let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();
    manager
        .add_custom_agent(&CustomAgentRecord {
            key: "copy_agent".into(),
            display_name: "Copy Agent".into(),
            skills_dir: tmp.path().join("global").to_string_lossy().into_owned(),
            category: AgentCategory::Coding,
        })
        .unwrap();
    let global = manager
        .deploy_skill(&skill.id, "copy_agent", Some(DeployMode::Copy), false)
        .unwrap();
    assert!(global.copy_hash.is_some());
    let project_path = tmp.path().join("project");
    std::fs::create_dir(&project_path).unwrap();
    let project = manager.add_project(&project_path, None).unwrap();
    let scoped = manager
        .deploy_skill_to_project(
            &skill.id,
            &project.id,
            "claude_code",
            Some(DeployMode::Copy),
            false,
        )
        .unwrap();

    let global_note = global.target_path.join("global-notes.txt");
    let project_note = scoped.target_path.join("project-notes.txt");
    std::fs::write(&global_note, "Local global edit").unwrap();
    std::fs::write(&project_note, "Local project edit").unwrap();
    for result in [
        manager.undeploy_skill(&skill.id, "copy_agent"),
        manager.undeploy_skill_from_project(&skill.id, &project.id, "claude_code"),
        manager.remove_skill(&skill.id),
        manager.remove_project(&project.id),
    ] {
        assert!(result.unwrap_err().to_string().contains("TARGET_CONFLICT"));
    }
    assert_eq!(
        std::fs::read_to_string(&global_note).unwrap(),
        "Local global edit"
    );
    assert_eq!(
        std::fs::read_to_string(&project_note).unwrap(),
        "Local project edit"
    );
    let saved = manager.find_skill(&skill.id).unwrap().unwrap();
    assert_eq!(
        (saved.deployments.len(), saved.project_deployments.len()),
        (1, 1)
    );

    std::fs::remove_file(&global_note).unwrap();
    std::fs::remove_file(&project_note).unwrap();
    manager
        .save_skill_markdown(
            &skill.id,
            "---\nname: Source\ndescription: Library edit only\n---\n",
        )
        .unwrap();
    manager.undeploy_skill(&skill.id, "copy_agent").unwrap();
    manager.remove_project(&project.id).unwrap();
    assert!(!global.target_path.exists());
    assert!(!scoped.target_path.exists());
}

#[test]
fn legacy_global_copy_uses_the_library_fingerprint_when_undeployed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("library");
    let manager = UnoSkillsManager::open_at(root.clone()).unwrap();
    let source = tmp.path().join("source");
    create_skill_dir(&source, "Source", "Original", &[]);
    let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();
    manager
        .add_custom_agent(&CustomAgentRecord {
            key: "legacy_agent".into(),
            display_name: "Legacy Agent".into(),
            skills_dir: tmp.path().join("global").to_string_lossy().into_owned(),
            category: AgentCategory::Coding,
        })
        .unwrap();
    let deployed = manager
        .deploy_skill(&skill.id, "legacy_agent", Some(DeployMode::Copy), false)
        .unwrap();
    drop(manager);
    let db = rusqlite::Connection::open(root.join("unoskills.db")).unwrap();
    db.execute("ALTER TABLE deployments DROP COLUMN copy_hash", [])
        .unwrap();
    drop(db);

    let manager = UnoSkillsManager::open_at(root).unwrap();
    let record = manager
        .find_skill(&skill.id)
        .unwrap()
        .unwrap()
        .deployments
        .remove(0);
    assert!(record.copy_hash.is_none());
    let note = deployed.target_path.join("notes.txt");
    std::fs::write(&note, "Legacy edit").unwrap();
    assert!(
        manager
            .undeploy_skill(&skill.id, "legacy_agent")
            .unwrap_err()
            .to_string()
            .contains("TARGET_CONFLICT")
    );
    assert!(note.exists());
    std::fs::remove_file(note).unwrap();
    manager.undeploy_skill(&skill.id, "legacy_agent").unwrap();
    assert!(!deployed.target_path.exists());
}

#[test]
fn stack_installs_the_declared_source_and_reports_partial_deployment() {
    let tmp = tempfile::tempdir().unwrap();
    let manager = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    manager.set_default_deploy_mode(DeployMode::Copy).unwrap();
    let other = tmp.path().join("other");
    let wanted = tmp.path().join("wanted");
    create_skill_dir(&other, "Shared Name", "Other source", &[]);
    create_skill_dir(&wanted, "Shared Name", "Declared source", &[]);
    manager.install_from_ref(other.to_str().unwrap()).unwrap();

    let mut agents = Vec::new();
    for key in ["working_agent", "blocked_agent"] {
        let directory = tmp.path().join(key);
        manager
            .add_custom_agent(&CustomAgentRecord {
                key: key.into(),
                display_name: key.into(),
                skills_dir: directory.to_string_lossy().into_owned(),
                category: AgentCategory::Coding,
            })
            .unwrap();
        agents.push(key.to_string());
    }
    let blocked = tmp.path().join("blocked_agent/wanted");
    create_skill_dir(&blocked, "Unmanaged", "Keep this skill", &[]);
    let stack = manager.import_stack_toml(&format!(
        "id = 'required-source'\nname = 'Required source'\n[[skills]]\nname = 'Shared Name'\nsource = {}\n",
        toml::Value::String(wanted.to_string_lossy().into_owned())
    )).unwrap();

    let result = manager.install_stack(&stack, &agents).unwrap();
    assert_eq!(result.installed, ["Shared Name"]);
    assert!(result.skipped_existing.is_empty());
    assert!(result.failed.is_empty());
    assert_eq!(result.deployed_agents, ["working_agent"]);
    assert_eq!(result.deployment_failures.len(), 1);
    let failure = &result.deployment_failures[0];
    assert_eq!(failure.agent, "blocked_agent");
    assert!(failure.error.contains("TARGET_CONFLICT"));
    assert!(!tmp.path().join("working_agent/other").exists());
    assert!(
        std::fs::read_to_string(tmp.path().join("working_agent/wanted/SKILL.md"))
            .unwrap()
            .contains("Declared source")
    );
    assert!(
        std::fs::read_to_string(blocked.join("SKILL.md"))
            .unwrap()
            .contains("Keep this skill")
    );

    let notes = tmp.path().join("working_agent/wanted/local-notes.txt");
    std::fs::write(&notes, "Local deployment edit").unwrap();
    let retry = manager.install_stack(&stack, &agents).unwrap();
    assert!(retry.deployed_agents.is_empty());
    assert_eq!(retry.deployment_failures.len(), 2);
    assert_eq!(
        std::fs::read_to_string(notes).unwrap(),
        "Local deployment edit"
    );
}

#[test]
fn stack_install_create_and_stale_save_preserve_an_existing_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let manager = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    let source = tmp.path().join("source");
    create_skill_dir(&source, "Source", "Local manifest member", &[]);
    let member = manager.install_from_ref(source.to_str().unwrap()).unwrap();
    let ids = std::slice::from_ref(&member.id);
    let original = manager
        .create_stack_from_skills("Saved Stack", "", "Local edit", vec![], ids)
        .unwrap();

    let candidate_source = tmp.path().join("candidate");
    create_skill_dir(
        &candidate_source,
        "Candidate",
        "Must not be imported on conflict",
        &[],
    );
    let mut candidate = original.clone();
    candidate.description = "External stack".into();
    candidate.skills[0].source = candidate_source.to_string_lossy().into_owned();
    let raw = manager.export_stack_toml(&candidate).unwrap();
    assert!(
        manager
            .import_stack_toml(&raw)
            .unwrap_err()
            .to_string()
            .contains("STACK_ID_CONFLICT")
    );
    assert!(
        manager
            .install_stack(&candidate, &[])
            .unwrap_err()
            .to_string()
            .contains("STACK_ID_CONFLICT")
    );
    assert!(manager.find_skill("candidate").unwrap().is_none());
    assert!(
        manager
            .create_stack_from_skills("Saved Stack", "", "New value", vec![], ids)
            .unwrap_err()
            .to_string()
            .contains("STACK_ID_CONFLICT")
    );

    // A desktop preview and a later confirm may straddle an edit from another process.
    let second = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    let mut concurrent_edit = original.clone();
    concurrent_edit.description = "New local edit".into();
    second
        .save_local_stack_if_unchanged(&concurrent_edit, Some(&original))
        .unwrap();
    assert!(
        manager
            .save_local_stack_if_unchanged(&candidate, Some(&original))
            .is_err()
    );
    assert_eq!(
        manager.find_local_stack(&original.id).unwrap(),
        Some(concurrent_edit.clone())
    );
    manager
        .save_local_stack_if_unchanged(&candidate, Some(&concurrent_edit))
        .unwrap();
    assert_eq!(
        manager.find_local_stack(&original.id).unwrap(),
        Some(candidate)
    );
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
fn reviewed_update_uses_frozen_files_and_can_restore_after_reopening() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("uno");
    let mgr = UnoSkillsManager::open_at(root.clone()).unwrap();
    let source = tmp.path().join("revision-skill");
    create_skill_dir(&source, "revision-skill", "version one", &["v1"]);
    std::fs::write(source.join("old.txt"), "old reference").unwrap();
    let skill = mgr.install_from_ref(&source.to_string_lossy()).unwrap();

    for agent in ["clean_agent", "edited_agent", "missing_agent", "git_agent"] {
        mgr.add_custom_agent(&CustomAgentRecord {
            key: agent.into(),
            display_name: agent.into(),
            skills_dir: tmp.path().join(agent).to_string_lossy().into(),
            category: AgentCategory::Coding,
        })
        .unwrap();
        mgr.deploy_skill(&skill.id, agent, Some(DeployMode::Copy), false)
            .unwrap();
    }
    let edited = tmp.path().join("edited_agent").join(&skill.slug);
    let missing = tmp.path().join("missing_agent").join(&skill.slug);
    let git_file = tmp.path().join("git_agent").join(&skill.slug).join(".git");
    std::fs::write(&git_file, "local worktree metadata").unwrap();
    let original = std::fs::read_to_string(edited.join("SKILL.md")).unwrap();
    let edited_value = format!("{original}local edit\n");
    std::fs::write(edited.join("SKILL.md"), &edited_value).unwrap();
    std::fs::remove_dir_all(&missing).unwrap();
    create_skill_dir(&source, "revision-skill", "version two", &["v2"]);
    std::fs::remove_file(source.join("old.txt")).unwrap();
    std::fs::write(source.join("new.txt"), "new reference").unwrap();

    let plan = mgr.prepare_skill_update(&skill.id).unwrap().unwrap();
    let files: Vec<_> = plan
        .preview()
        .files
        .iter()
        .map(|item| item.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(files, ["SKILL.md", "new.txt", "old.txt"]);
    assert!(
        plan.preview()
            .deployments
            .iter()
            .any(|item| item.kind == DeploymentImpactKind::Conflict)
    );
    assert!(
        plan.preview()
            .deployments
            .iter()
            .any(|item| item.kind == DeploymentImpactKind::Missing)
    );
    assert!(skill.central_path.join("old.txt").is_file());
    assert!(!mgr.has_skill_rollback(&skill.id));

    create_skill_dir(&source, "revision-skill", "unreviewed version", &["v3"]);
    let updated = mgr.apply_skill_revision(plan).unwrap();
    assert_eq!(updated.skill.description.as_deref(), Some("version two"));
    assert_eq!(updated.refreshed_copies, 1);
    assert_eq!(updated.deployment_issues.len(), 3);
    assert_eq!(
        std::fs::read_to_string(&git_file).unwrap(),
        "local worktree metadata"
    );
    assert!(!missing.exists());
    assert_eq!(
        std::fs::read_to_string(edited.join("SKILL.md")).unwrap(),
        edited_value
    );
    assert!(!skill.central_path.join("old.txt").exists());
    assert!(skill.central_path.join("new.txt").is_file());

    drop(mgr);
    let mgr = UnoSkillsManager::open_at(root).unwrap();
    assert!(mgr.has_skill_rollback(&skill.id));
    let plan = mgr.prepare_skill_rollback(&skill.id).unwrap().unwrap();
    let restored = mgr.apply_skill_revision(plan).unwrap();
    assert_eq!(restored.skill.description.as_deref(), Some("version one"));
    assert_eq!(restored.skill.tags, ["v1"]);
    assert_eq!(restored.refreshed_copies, 1);
    assert_eq!(
        std::fs::read_to_string(&git_file).unwrap(),
        "local worktree metadata"
    );
    assert_eq!(
        std::fs::read_to_string(edited.join("SKILL.md")).unwrap(),
        edited_value
    );
    assert_eq!(
        std::fs::read_to_string(skill.central_path.join("old.txt")).unwrap(),
        "old reference"
    );
    assert!(!skill.central_path.join("new.txt").exists());
}

#[test]
fn reviewed_update_rejects_a_local_edit_after_preview() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    let source = tmp.path().join("revision-skill");
    create_skill_dir(&source, "revision-skill", "version one", &["v1"]);
    let skill = mgr.install_from_ref(&source.to_string_lossy()).unwrap();
    create_skill_dir(&source, "revision-skill", "version two", &["v2"]);
    let plan = mgr.prepare_skill_update(&skill.id).unwrap().unwrap();
    mgr.save_skill_markdown(&skill.id, "# Locally edited")
        .unwrap();
    let error = mgr.apply_skill_revision(plan).unwrap_err().to_string();
    assert!(error.contains("STALE_SKILL_REVISION"));
    assert_eq!(
        std::fs::read_to_string(skill.central_path.join("SKILL.md")).unwrap(),
        "# Locally edited"
    );
    assert!(!mgr.has_skill_rollback(&skill.id));
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
fn deploy_stack_to_project_skips_existing_and_preserves_unmanaged_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let manager = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    manager.set_default_deploy_mode(DeployMode::Copy).unwrap();

    let agent_dir = tmp.path().join("agent-skills");
    std::fs::create_dir(&agent_dir).unwrap();
    let agent_key = "custom_stack_qa";
    manager
        .add_custom_agent(&CustomAgentRecord {
            key: agent_key.into(),
            display_name: "Stack QA".into(),
            skills_dir: agent_dir.to_string_lossy().into(),
            category: AgentCategory::Coding,
        })
        .unwrap();
    let agent = manager
        .list_agents()
        .unwrap()
        .into_iter()
        .find(|agent| agent.key == agent_key)
        .unwrap();
    let project_dir = tmp.path().join("project");
    std::fs::create_dir(&project_dir).unwrap();
    let project = manager.add_project(&project_dir, None).unwrap();
    let project_skills = project.path.join(&agent.project_relative_skills_dir);

    for slug in ["review", "design", "conflict", "same-name"] {
        let path = tmp.path().join("src").join(slug);
        create_skill_dir(&path, slug, "Test skill", &["qa"]);
        manager.install_from_ref(&path.to_string_lossy()).unwrap();
    }
    let mut stack = manager
        .create_stack_from_skills(
            "QA",
            "",
            "",
            Vec::new(),
            &["review".into(), "design".into(), "conflict".into()],
        )
        .unwrap();
    stack.skills.push(StackSkillRef {
        name: "same-name".into(),
        source: "owner/other-repo@same-name".into(),
        note: String::new(),
    });
    stack.skills.push(StackSkillRef {
        name: "missing".into(),
        source: "owner/other-repo@missing".into(),
        note: String::new(),
    });
    manager
        .deploy_skill_to_project(
            "review",
            &project.id,
            agent_key,
            Some(DeployMode::Copy),
            false,
        )
        .unwrap();
    let unmanaged = project_skills.join("conflict");
    std::fs::create_dir(&unmanaged).unwrap();
    let custom = unmanaged.join("SKILL.md");
    std::fs::write(&custom, "user skill").unwrap();

    let status = manager
        .stack_project_deploy_status(&stack, &project.id, agent_key)
        .unwrap();
    assert_eq!(
        (
            status.deployed,
            status.pending,
            status.unavailable,
            status.conflicts
        ),
        (1, 1, 2, 1)
    );
    let result = manager
        .deploy_stack_to_project(&stack, &project.id, agent_key)
        .unwrap();
    assert_eq!(
        (result.deployed, result.already_deployed, result.unavailable),
        (1, 1, 2)
    );
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].0, "conflict");
    assert!(result.failed[0].1.contains("TARGET_CONFLICT"));
    assert_eq!(std::fs::read_to_string(&custom).unwrap(), "user skill");
    assert!(project_skills.join("design/SKILL.md").is_file());
    assert!(!project_skills.join("same-name").exists());
    assert!(std::fs::read_dir(agent_dir).unwrap().next().is_none());

    let retry = manager
        .deploy_stack_to_project(&stack, &project.id, agent_key)
        .unwrap();
    assert_eq!(
        (retry.deployed, retry.already_deployed, retry.failed.len()),
        (0, 2, 1)
    );

    let preserved_dir = project.path.join("preserved-skill");
    std::fs::rename(unmanaged, &preserved_dir).unwrap();
    let retry = manager
        .deploy_stack_to_project(&stack, &project.id, agent_key)
        .unwrap();
    assert_eq!(
        (retry.deployed, retry.already_deployed, retry.unavailable),
        (1, 2, 2)
    );
    assert!(retry.failed.is_empty());
    assert_eq!(
        std::fs::read_to_string(preserved_dir.join("SKILL.md")).unwrap(),
        "user skill"
    );
    assert!(project_skills.join("conflict/extra.txt").is_file());
}

#[test]
fn project_availability_counts_global_and_project_files_without_duplicate_deployment() {
    use unoskills_core::ProjectSkillPlacement;

    let tmp = tempfile::tempdir().unwrap();
    let manager = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    let agent_dir = tmp.path().join("codex-skills");
    manager
        .set_agent_override("codex", agent_dir.to_str())
        .unwrap();
    manager.set_default_deploy_mode(DeployMode::Copy).unwrap();
    let project_dir = tmp.path().join("project");
    std::fs::create_dir(&project_dir).unwrap();
    let project = manager.add_project(&project_dir, None).unwrap();
    let fixture = tmp.path().join("uno-review-fixture");
    create_skill_dir(&fixture, "uno-review-fixture", "Review skill", &["qa"]);
    let skill = manager.install_from_ref(fixture.to_str().unwrap()).unwrap();
    let stack = manager
        .create_stack_from_skills("QA", "", "", vec![], std::slice::from_ref(&skill.id))
        .unwrap();
    let read_status = || manager.project_skill_statuses(&project.id).unwrap()["codex"][&skill.id];
    let preview = || {
        manager
            .stack_project_deploy_status(&stack, &project.id, "codex")
            .unwrap()
    };
    assert!(!read_status().is_available());

    let global = manager
        .deploy_skill(&skill.id, "codex", None, false)
        .unwrap();
    assert!(read_status().is_available());
    assert_eq!(read_status().placement, ProjectSkillPlacement::Missing);
    assert!(read_status().global_available);
    let status = preview();
    assert_eq!(
        (
            status.global_available,
            status.pending_count(false),
            status.pending_count(true)
        ),
        (1, 0, 1)
    );
    let result = manager
        .deploy_stack_to_project(&stack, &project.id, "codex")
        .unwrap();
    assert_eq!((result.global_available, result.deployed), (1, 0));
    assert!(
        manager
            .find_skill(&skill.id)
            .unwrap()
            .unwrap()
            .project_deployments
            .is_empty()
    );

    let result = manager
        .deploy_stack_to_project_at_with_globals(
            &stack,
            &project.id,
            "codex",
            std::path::Path::new(""),
            true,
        )
        .unwrap();
    assert_eq!(result.deployed, 1);
    assert_eq!(preview().deployed, 1);
    assert_eq!(read_status().placement, ProjectSkillPlacement::Managed);
    let target = manager
        .find_skill(&skill.id)
        .unwrap()
        .unwrap()
        .project_deployments[0]
        .target_path
        .clone();
    manager
        .undeploy_skill_from_project(&skill.id, &project.id, "codex")
        .unwrap();
    assert!(!target.exists());
    assert!(read_status().is_available());
    assert!(global.target_path.join("SKILL.md").is_file());

    unoskills_core::sync::copy_dir_recursive(&skill.central_path, &target).unwrap();
    assert_eq!(read_status().placement, ProjectSkillPlacement::Local);
    assert_eq!(preview().project_local, 1);
    assert_eq!(
        manager
            .deploy_stack_to_project(&stack, &project.id, "codex")
            .unwrap()
            .project_local,
        1
    );

    std::fs::write(
        global.target_path.join("SKILL.md"),
        "different global skill",
    )
    .unwrap();
    std::fs::write(target.join("SKILL.md"), "different project skill").unwrap();
    let status = read_status();
    assert_eq!(status.placement, ProjectSkillPlacement::Conflict);
    assert!(!status.global_available);
    assert!(!status.is_available());
    assert_eq!(preview().conflicts, 1);
    assert!(
        manager
            .deploy_skill_to_project(&skill.id, &project.id, "codex", None, false)
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "different project skill"
    );
}

#[test]
fn project_stack_deployment_repairs_missing_files_and_preserves_replacements() {
    let tmp = tempfile::tempdir().unwrap();
    let manager = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    let agent_path = tmp.path().join("global-codex");
    manager
        .set_agent_override("codex", agent_path.to_str())
        .unwrap();
    let review = tmp.path().join("review");
    create_skill_dir(&review, "review", "Review skill", &["qa"]);
    let skill = manager.install_from_ref(review.to_str().unwrap()).unwrap();
    let stack = manager
        .create_stack_from_skills("QA", "", "", vec![], std::slice::from_ref(&skill.id))
        .unwrap();

    for mode in [DeployMode::Copy, DeployMode::Symlink] {
        manager.set_default_deploy_mode(mode).unwrap();
        let path = tmp.path().join(mode.as_str());
        std::fs::create_dir(&path).unwrap();
        let project = manager.add_project(&path, None).unwrap();
        let target = project.path.join(".codex/skills/review");
        let deploy = || {
            manager
                .deploy_stack_to_project(&stack, &project.id, "codex")
                .unwrap()
        };
        let status = || {
            manager
                .stack_project_deploy_status(&stack, &project.id, "codex")
                .unwrap()
        };
        assert_eq!(deploy().deployed, 1);
        assert_eq!((status().deployed, status().pending), (1, 0));
        let recorded_mode = manager
            .find_skill(&skill.id)
            .unwrap()
            .unwrap()
            .project_deployments
            .iter()
            .find(|dep| dep.project_id == project.id)
            .unwrap()
            .mode;

        if mode == DeployMode::Copy {
            let original = std::fs::read_to_string(target.join("SKILL.md")).unwrap();
            manager
                .save_skill_markdown(
                    &skill.id,
                    &format!("{original}\nA newer library version.\n"),
                )
                .unwrap();
            assert_eq!(status().deployed, 1);
            assert_eq!(
                std::fs::read_to_string(target.join("SKILL.md")).unwrap(),
                original
            );
        }

        std::fs::rename(&target, path.join("previous-deployment")).unwrap();
        let missing = status();
        assert_eq!(
            (missing.deployed, missing.pending, missing.conflicts),
            (0, 1, 0)
        );
        manager
            .set_default_deploy_mode(if recorded_mode == DeployMode::Copy {
                DeployMode::Symlink
            } else {
                DeployMode::Copy
            })
            .unwrap();
        let repaired = deploy();
        assert_eq!((repaired.deployed, repaired.already_deployed), (1, 0));
        assert!(repaired.failed.is_empty());
        assert_eq!(
            std::fs::read_to_string(target.join("extra.txt")).unwrap(),
            "supporting asset"
        );
        assert_eq!(status().deployed, 1);
        assert_eq!(
            manager
                .find_skill(&skill.id)
                .unwrap()
                .unwrap()
                .project_deployments
                .iter()
                .find(|dep| dep.project_id == project.id)
                .unwrap()
                .mode,
            recorded_mode
        );

        std::fs::rename(&target, path.join("repaired-deployment")).unwrap();
        std::fs::create_dir(&target).unwrap();
        let marker = target.join("SKILL.md");
        std::fs::write(&marker, "project-specific recipe").unwrap();
        let occupied = status();
        assert_eq!(
            (occupied.deployed, occupied.pending, occupied.conflicts),
            (0, 0, 1)
        );
        let refused = deploy();
        assert_eq!(refused.deployed, 0);
        assert!(refused.failed[0].1.contains("TARGET_CONFLICT"));
        assert_eq!(
            std::fs::read_to_string(&marker).unwrap(),
            "project-specific recipe"
        );
    }
    assert_eq!(
        manager.list_skills().unwrap()[0].project_deployments.len(),
        2
    );
}

#[test]
fn project_stack_status_supports_copy_records_created_before_fingerprints() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("uno");
    let manager = UnoSkillsManager::open_at(root.clone()).unwrap();
    manager.set_default_deploy_mode(DeployMode::Copy).unwrap();
    let agent_path = tmp.path().join("global-codex");
    manager
        .set_agent_override("codex", agent_path.to_str())
        .unwrap();
    let review = tmp.path().join("review");
    create_skill_dir(&review, "review", "Review skill", &["qa"]);
    let skill = manager.install_from_ref(review.to_str().unwrap()).unwrap();
    let stack = manager
        .create_stack_from_skills("QA", "", "", vec![], std::slice::from_ref(&skill.id))
        .unwrap();
    let project_path = tmp.path().join("project");
    std::fs::create_dir(&project_path).unwrap();
    let project = manager.add_project(&project_path, None).unwrap();
    manager
        .deploy_stack_to_project(&stack, &project.id, "codex")
        .unwrap();
    drop(manager);

    let conn = rusqlite::Connection::open(root.join("unoskills.db")).unwrap();
    conn.execute_batch(
        "ALTER TABLE project_deployments RENAME TO scoped_deployments;
         CREATE TABLE project_deployments (
             id TEXT PRIMARY KEY,
             skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
             project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
             agent_key TEXT NOT NULL,
             target_path TEXT NOT NULL,
             mode TEXT NOT NULL,
             synced_at INTEGER NOT NULL,
             UNIQUE (skill_id, project_id, agent_key)
         );
         INSERT INTO project_deployments
             SELECT id, skill_id, project_id, agent_key, target_path, mode, synced_at
             FROM scoped_deployments;
         DROP TABLE scoped_deployments;",
    )
    .unwrap();
    drop(conn);
    let manager = UnoSkillsManager::open_at(root).unwrap();
    let status = manager
        .stack_project_deploy_status(&stack, &project.id, "codex")
        .unwrap();
    assert_eq!(
        (status.deployed, status.pending, status.conflicts),
        (1, 0, 0)
    );
    assert!(
        manager.list_skills().unwrap()[0].project_deployments[0]
            .copy_hash
            .is_none()
    );
    assert!(manager.list_skills().unwrap()[0].is_deployed_to_project(&project.id, "codex"));
}

#[test]
fn project_stack_deployment_keeps_root_and_module_placements_independent() {
    use std::path::Path;
    use unoskills_core::project_analysis::SkillLocation;

    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("uno");
    let manager = UnoSkillsManager::open_at(data.clone()).unwrap();
    manager.set_default_deploy_mode(DeployMode::Copy).unwrap();
    let agent_path = tmp.path().join("codex");
    std::fs::create_dir(&agent_path).unwrap();
    manager
        .set_agent_override("codex", agent_path.to_str())
        .unwrap();
    let source = tmp.path().join("review");
    create_skill_dir(&source, "review", "Review skill", &["qa"]);
    let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();
    let stack = manager
        .create_stack_from_skills("QA", "", "", vec![], std::slice::from_ref(&skill.id))
        .unwrap();
    let root = tmp.path().join("workspace");
    let module = Path::new("apps").join("shop");
    std::fs::create_dir_all(root.join(&module)).unwrap();
    std::fs::write(root.join(&module).join("package.json"), "{}").unwrap();
    let project = manager.add_project(&root, None).unwrap();
    let root_target = project.path.join(".codex/skills/review/SKILL.md");
    let module_target = project
        .path
        .join(&module)
        .join(".codex/skills/review/SKILL.md");
    assert!(
        manager
            .project_module_paths(&project.id)
            .unwrap()
            .contains(&module)
    );

    for invalid in [
        Path::new("../review"),
        source.as_path(),
        Path::new("apps/missing"),
    ] {
        assert!(
            manager
                .stack_project_deploy_status_at(&stack, &project.id, "codex", invalid)
                .is_err()
        );
        assert!(
            manager
                .deploy_stack_to_project_at(&stack, &project.id, "codex", invalid)
                .is_err()
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&source, root.join("linked")).unwrap();
        assert!(
            manager
                .deploy_stack_to_project_at(&stack, &project.id, "codex", Path::new("linked"))
                .is_err()
        );
        let agent_dir = root.join(&module).join(".codex");
        std::os::unix::fs::symlink(&source, &agent_dir).unwrap();
        assert!(
            manager
                .deploy_stack_to_project_at(&stack, &project.id, "codex", &module)
                .is_err()
        );
        std::fs::remove_file(agent_dir).unwrap();
    }

    assert_eq!(
        manager
            .deploy_stack_to_project(&stack, &project.id, "codex")
            .unwrap()
            .deployed,
        1
    );
    assert!(!module_target.exists());
    assert_eq!(
        manager
            .stack_project_deploy_status_at(&stack, &project.id, "codex", &module)
            .unwrap()
            .pending,
        1
    );
    std::fs::create_dir_all(module_target.parent().unwrap()).unwrap();
    std::fs::write(&module_target, "project-specific skill").unwrap();
    assert_eq!(
        manager
            .stack_project_deploy_status_at(&stack, &project.id, "codex", &module)
            .unwrap()
            .conflicts,
        1
    );
    let conflict = manager
        .deploy_stack_to_project_at(&stack, &project.id, "codex", &module)
        .unwrap();
    assert!(conflict.failed[0].1.contains("TARGET_CONFLICT"));
    assert_eq!(
        std::fs::read_to_string(&module_target).unwrap(),
        "project-specific skill"
    );
    std::fs::remove_dir_all(module_target.parent().unwrap()).unwrap();
    assert_eq!(
        manager
            .deploy_stack_to_project_at(&stack, &project.id, "codex", &module)
            .unwrap()
            .deployed,
        1
    );
    assert!(root_target.is_file() && module_target.is_file());
    drop(manager);

    let manager = UnoSkillsManager::open_at(data).unwrap();
    assert_eq!(manager.list_projects().unwrap().len(), 1);
    let skill = manager.list_skills().unwrap().pop().unwrap();
    assert_eq!(skill.project_deployments.len(), 2);
    assert!(
        skill
            .project_deployments
            .iter()
            .any(|dep| dep.scope_path == module)
    );
    let status = manager
        .deploy_stack_to_project_at(&stack, &project.id, "codex", &module)
        .unwrap();
    assert_eq!((status.deployed, status.already_deployed), (0, 1));
    let inventory: Vec<_> = manager
        .inspect_project_skills_at(&project.id, "codex", &module)
        .unwrap()
        .into_iter()
        .filter(|skill| skill.location != SkillLocation::Global)
        .collect();
    assert_eq!(inventory.len(), 1);
    assert_eq!(inventory[0].location, SkillLocation::ProjectDeployment);
    assert_eq!(inventory[0].path, module_target.parent().unwrap());
    manager
        .undeploy_skill_from_project(&skill.id, &project.id, "codex")
        .unwrap();
    assert!(!root_target.exists() && module_target.exists());
    assert!(!manager.list_skills().unwrap()[0].is_deployed_to_project(&project.id, "codex"));
    manager.remove_project(&project.id).unwrap();
    assert!(!module_target.exists());
    assert!(
        manager.list_skills().unwrap()[0]
            .project_deployments
            .is_empty()
    );
}

#[test]
fn project_stack_deployment_is_scoped_and_persists_after_reopening() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("uno");
    let manager = UnoSkillsManager::open_at(root.clone()).unwrap();
    manager
        .set_default_deploy_mode(DeployMode::Symlink)
        .unwrap();

    for key in ["codex", "claude_code"] {
        let global_dir = tmp.path().join(key);
        std::fs::create_dir(&global_dir).unwrap();
        manager
            .set_agent_override(key, global_dir.to_str())
            .unwrap();
    }
    let project_a_path = tmp.path().join("project-a");
    let project_b_path = tmp.path().join("project-b");
    std::fs::create_dir(&project_a_path).unwrap();
    std::fs::create_dir(&project_b_path).unwrap();
    let project_a = manager.add_project(&project_a_path, None).unwrap();
    let project_b = manager.add_project(&project_b_path, None).unwrap();

    let review = tmp.path().join("review");
    create_skill_dir(&review, "review", "Review skill", &["qa"]);
    let skill = manager.install_from_ref(review.to_str().unwrap()).unwrap();
    let mut stack = manager
        .create_stack_from_skills("QA", "", "", vec![], std::slice::from_ref(&skill.id))
        .unwrap();
    stack.skills[0].name = "older-review-name".into();
    let codex_a = project_a.path.join(".codex/skills/review");
    let claude_a = project_a.path.join(".claude/skills/review");
    let codex_b = project_b.path.join(".codex/skills/review");

    let first = manager
        .deploy_stack_to_project(&stack, &project_a.id, "codex")
        .unwrap();
    assert_eq!(first.deployed, 1);
    assert!(first.failed.is_empty());
    assert!(codex_a.join("extra.txt").is_file());
    assert!(!claude_a.exists());
    assert!(!codex_b.exists());
    #[cfg(unix)]
    assert_eq!(std::fs::read_link(&codex_a).unwrap(), skill.central_path);

    drop(manager);
    let manager = UnoSkillsManager::open_at(root).unwrap();
    let retry = manager
        .deploy_stack_to_project(&stack, &project_a.id, "codex")
        .unwrap();
    assert_eq!((retry.deployed, retry.already_deployed), (0, 1));
    for (project, agent) in [(&project_a, "claude_code"), (&project_b, "codex")] {
        let result = manager
            .deploy_stack_to_project(&stack, &project.id, agent)
            .unwrap();
        assert_eq!((result.deployed, result.already_deployed), (1, 0));
        assert!(result.failed.is_empty());
    }
    assert!(claude_a.join("SKILL.md").is_file());
    assert!(codex_b.join("SKILL.md").is_file());
    assert!(
        std::fs::read_dir(tmp.path().join("codex"))
            .unwrap()
            .next()
            .is_none()
    );
    assert!(
        std::fs::read_dir(tmp.path().join("claude_code"))
            .unwrap()
            .next()
            .is_none()
    );

    stack.skills.clear();
    manager.save_local_stack(&stack).unwrap();
    let skill = manager.list_skills().unwrap().pop().unwrap();
    assert_eq!(skill.project_deployments.len(), 3);
    assert!(skill.deployments.is_empty());
    assert!(codex_a.join("SKILL.md").is_file());
}

#[test]
fn project_stack_deployment_rejects_unavailable_targets_before_writing() {
    let tmp = tempfile::tempdir().unwrap();
    let manager = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    let project_path = tmp.path().join("project");
    std::fs::create_dir(&project_path).unwrap();
    let project = manager.add_project(&project_path, None).unwrap();
    let review = tmp.path().join("review");
    create_skill_dir(&review, "review", "Review skill", &["qa"]);
    let skill = manager.install_from_ref(review.to_str().unwrap()).unwrap();
    let stack = manager
        .create_stack_from_skills("QA", "", "", vec![], std::slice::from_ref(&skill.id))
        .unwrap();

    let agent_path = tmp.path().join("global-codex");
    manager
        .set_agent_override("codex", agent_path.to_str())
        .unwrap();
    manager.set_agent_enabled("codex", false).unwrap();
    let error = manager
        .deploy_stack_to_project(&stack, &project.id, "codex")
        .unwrap_err();
    assert!(error.to_string().contains("no longer available"));
    let error = manager
        .deploy_stack_to_project(&stack, &project.id, "nonexistent_agent")
        .unwrap_err();
    assert!(error.to_string().contains("no longer available"));
    assert!(std::fs::read_dir(&project_path).unwrap().next().is_none());

    manager.set_agent_enabled("codex", true).unwrap();
    std::fs::remove_dir(&project_path).unwrap();
    let error = manager
        .deploy_stack_to_project(&stack, &project.id, "codex")
        .unwrap_err();
    assert!(error.to_string().contains("directory no longer exists"));
    let error = manager
        .deploy_stack_to_project(&stack, "unknown-project", "codex")
        .unwrap_err();
    assert!(error.to_string().contains("Project workspace not found"));
    assert!(!project_path.exists());
    assert!(
        manager.list_skills().unwrap()[0]
            .project_deployments
            .is_empty()
    );
}

#[test]
fn project_stack_deployment_preserves_shared_agent_targets() {
    let tmp = tempfile::tempdir().unwrap();
    let manager = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
    manager.set_default_deploy_mode(DeployMode::Copy).unwrap();
    for agent in ["amp", "replit"] {
        let global_dir = tmp.path().join(agent);
        manager
            .set_agent_override(agent, global_dir.to_str())
            .unwrap();
    }
    let project_dir = tmp.path().join("project");
    std::fs::create_dir(&project_dir).unwrap();
    let project = manager.add_project(&project_dir, None).unwrap();
    let review = tmp.path().join("review");
    create_skill_dir(&review, "review", "Review skill", &["qa"]);
    let skill = manager.install_from_ref(review.to_str().unwrap()).unwrap();
    let stack = manager
        .create_stack_from_skills("QA", "", "", vec![], std::slice::from_ref(&skill.id))
        .unwrap();

    for agent in ["amp", "replit"] {
        let result = manager
            .deploy_stack_to_project(&stack, &project.id, agent)
            .unwrap();
        assert_eq!((result.deployed, result.already_deployed), (1, 0));
        assert!(result.failed.is_empty());
    }
    let skill = manager.list_skills().unwrap().pop().unwrap();
    let target = &skill.project_deployments[0].target_path;
    assert_eq!(skill.project_deployments.len(), 2);
    assert_eq!(target, &skill.project_deployments[1].target_path);
    manager
        .undeploy_skill_from_project(&skill.id, &project.id, "amp")
        .unwrap();
    assert!(target.join("SKILL.md").is_file());
    let retry = manager
        .deploy_stack_to_project(&stack, &project.id, "replit")
        .unwrap();
    assert_eq!((retry.deployed, retry.already_deployed), (0, 1));
    manager
        .undeploy_skill_from_project(&skill.id, &project.id, "replit")
        .unwrap();
    assert!(!target.exists());
    assert!(skill.central_path.join("SKILL.md").is_file());
}

#[test]
#[cfg(unix)]
fn project_deploy_and_removal_refuse_redirected_paths() {
    use std::{fs, os::unix::fs::symlink};

    let tmp = tempfile::tempdir().unwrap();
    let manager = UnoSkillsManager::open_at(tmp.path().join("library")).unwrap();
    let source = tmp.path().join("fixture");
    create_skill_dir(&source, "Fixture", "Project path safety", &[]);
    let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();

    let project_dir = tmp.path().join("project");
    let alias = tmp.path().join("project-alias");
    let original = tmp.path().join("original");
    let outside = tmp.path().join("elsewhere");
    fs::create_dir(&project_dir).unwrap();
    fs::create_dir(&outside).unwrap();
    symlink(&project_dir, &alias).unwrap();
    let project = manager.add_project(&alias, None).unwrap();
    assert_eq!(project.path, project_dir.canonicalize().unwrap());
    let deployment = manager
        .deploy_skill_to_project(
            &skill.id,
            &project.id,
            "codex",
            Some(DeployMode::Copy),
            false,
        )
        .unwrap();

    fs::rename(&project_dir, &original).unwrap();
    symlink(&outside, &project_dir).unwrap();
    let deploy_error = manager
        .deploy_skill_to_project(&skill.id, &project.id, "codex", None, true)
        .unwrap_err();
    assert!(deploy_error.to_string().contains("TARGET_CONFLICT"));
    assert!(
        manager
            .undeploy_skill_from_project(&skill.id, &project.id, "codex")
            .unwrap_err()
            .to_string()
            .contains("TARGET_CONFLICT")
    );
    assert!(fs::read_dir(&outside).unwrap().next().is_none());

    fs::remove_file(&project_dir).unwrap();
    fs::rename(&original, &project_dir).unwrap();
    let agent_dir = original.join(".codex");
    fs::create_dir(&original).unwrap();
    fs::rename(project_dir.join(".codex"), &agent_dir).unwrap();
    symlink(&agent_dir, project_dir.join(".codex")).unwrap();
    assert!(
        manager
            .deploy_skill_to_project(&skill.id, &project.id, "codex", None, false)
            .unwrap_err()
            .to_string()
            .contains("TARGET_CONFLICT")
    );
    assert!(
        manager
            .remove_project(&project.id)
            .unwrap_err()
            .to_string()
            .contains("TARGET_CONFLICT")
    );
    assert!(deployment.target_path.join("SKILL.md").is_file());

    fs::remove_file(project_dir.join(".codex")).unwrap();
    fs::rename(&agent_dir, project_dir.join(".codex")).unwrap();
    let retried = manager
        .deploy_skill_to_project(&skill.id, &project.id, "codex", None, false)
        .unwrap();
    assert_eq!(retried.id, deployment.id);
    assert_eq!(retried.synced_at, deployment.synced_at);
    assert_eq!(retried.mode, DeployMode::Copy);
    manager.remove_project(&project.id).unwrap();
    assert!(!deployment.target_path.exists());
}

#[test]
#[cfg(unix)]
fn project_redeploy_preserves_a_link_added_to_a_managed_copy() {
    use std::{fs, os::unix::fs::symlink};

    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("fixture");
    let project_dir = tmp.path().join("project");
    let manager = UnoSkillsManager::open_at(tmp.path().join("library")).unwrap();
    create_skill_dir(&source, "Fixture", "Managed project copy", &[]);
    fs::create_dir(&project_dir).unwrap();
    let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();
    let project = manager.add_project(&project_dir, None).unwrap();
    let deployment = manager
        .deploy_skill_to_project(
            &skill.id,
            &project.id,
            "codex",
            Some(DeployMode::Copy),
            false,
        )
        .unwrap();
    let local_link = deployment.target_path.join("local-link");
    let outside = tmp.path().join("local.txt");
    fs::write(&outside, "Local project data").unwrap();
    symlink(&outside, &local_link).unwrap();

    for mode in [None, Some(DeployMode::Symlink)] {
        assert!(
            manager
                .deploy_skill_to_project(&skill.id, &project.id, "codex", mode, false)
                .unwrap_err()
                .to_string()
                .contains("TARGET_CONFLICT")
        );
    }
    assert_eq!(fs::read_link(&local_link).unwrap(), outside);
    assert_eq!(
        fs::read_to_string(&local_link).unwrap(),
        "Local project data"
    );
    fs::remove_file(&local_link).unwrap();
    let changed = manager
        .deploy_skill_to_project(
            &skill.id,
            &project.id,
            "codex",
            Some(DeployMode::Symlink),
            false,
        )
        .unwrap();
    assert_eq!(changed.id, deployment.id);
    assert_eq!(
        fs::read_link(&changed.target_path).unwrap(),
        skill.central_path
    );
    manager
        .undeploy_skill_from_project(&skill.id, &project.id, "codex")
        .unwrap();
    assert!(skill.central_path.join("SKILL.md").is_file());
}

#[test]
fn project_copy_rejects_a_destination_inside_the_skill_even_when_parents_are_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("fixture");
    create_skill_dir(&source, "Fixture", "Overlapping project", &[]);
    let manager = UnoSkillsManager::open_at(tmp.path().join("library")).unwrap();
    let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();
    let project = manager.add_project(&skill.central_path, None).unwrap();
    let error = manager
        .deploy_skill_to_project(
            &skill.id,
            &project.id,
            "codex",
            Some(DeployMode::Copy),
            false,
        )
        .unwrap_err();
    assert!(error.to_string().contains("inside source"));
    assert!(!skill.central_path.join(".codex").exists());
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
fn backup_rejects_unstaged_files_and_retries_after_the_remote_branch_appears() {
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    fixture_git(tmp.path(), &["init", "--bare", "remote.git"]);
    let remote = tmp.path().join("remote.git");
    let url = remote.to_str().unwrap();
    let author = UnoSkillsManager::open_at(tmp.path().join("author")).unwrap();
    let publisher = UnoSkillsManager::open_at(tmp.path().join("publisher")).unwrap();
    let source = tmp.path().join("fixture");
    create_skill_dir(&source, "Fixture", "Staging and retry", &[]);
    let skill = author.install_from_ref(source.to_str().unwrap()).unwrap();
    let ignored = skill.central_path.join(".staging-manual.txt");
    fs::write(&ignored, "A file the author expected to export").unwrap();
    fs::write(skill.central_path.join("extra.txt"), "Early draft").unwrap();
    author.configure_git_backup(url, Some("main")).unwrap();
    let error = author.git_backup_push().unwrap_err().to_string();
    assert!(error.contains("BACKUP_STAGE_CONFLICT"), "{error}");
    assert!(error.contains(".staging-manual.txt"), "{error}");
    assert_eq!(fixture_git(&remote, &["for-each-ref"]), "");

    publisher
        .install_from_ref(source.to_str().unwrap())
        .unwrap();
    assert!(publisher.git_backup_push_to(url, "main").unwrap().pushed);
    let head = fixture_git(&remote, &["rev-parse", "refs/heads/main"]);
    fs::remove_file(&ignored).unwrap();
    fs::copy(
        source.join("extra.txt"),
        skill.central_path.join("extra.txt"),
    )
    .unwrap();
    assert!(!author.git_backup_push().unwrap().pushed);
    assert_eq!(
        fixture_git(&remote, &["rev-parse", "refs/heads/main"]),
        head
    );
    fs::write(skill.central_path.join("new-asset.txt"), "A portable asset").unwrap();
    assert!(author.git_backup_push().unwrap().pushed);
    assert_eq!(
        fixture_git(
            &remote,
            &["show", "refs/heads/main:skills/fixture/new-asset.txt"]
        ),
        "A portable asset"
    );
    assert_eq!(
        fixture_git(
            &remote,
            &["show", "refs/heads/main:skills/fixture/extra.txt"]
        ),
        "supporting asset"
    );
}

#[test]
#[cfg(unix)]
fn backup_refuses_an_external_file_link_and_succeeds_after_it_is_removed() {
    use std::{fs, os::unix::fs::symlink};

    let tmp = tempfile::tempdir().unwrap();
    fixture_git(tmp.path(), &["init", "--bare", "remote.git"]);
    let remote = tmp.path().join("remote.git");
    let manager = UnoSkillsManager::open_at(tmp.path().join("library")).unwrap();
    let source = tmp.path().join("fixture");
    create_skill_dir(&source, "Fixture", "Link safety", &[]);
    let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();
    let outside = tmp.path().join("outside.txt");
    let link = skill.central_path.join("attachment.txt");
    fs::write(&outside, "Synthetic external data").unwrap();
    symlink(&outside, &link).unwrap();
    let error = manager
        .git_backup_push_to(remote.to_str().unwrap(), "main")
        .unwrap_err()
        .to_string();
    assert!(error.contains("BACKUP_PATH_CONFLICT"), "{error}");
    assert!(error.contains("attachment.txt"), "{error}");
    assert_eq!(fixture_git(&remote, &["for-each-ref"]), "");
    assert_eq!(fs::read_link(&link).unwrap(), outside);

    fs::remove_file(&link).unwrap();
    fs::write(
        skill.central_path.join("cache.db"),
        "Intentionally excluded",
    )
    .unwrap();
    assert!(manager.git_backup_push().unwrap().pushed);
    let tree = fixture_git(
        &remote,
        &["ls-tree", "-r", "--name-only", "refs/heads/main"],
    );
    assert_eq!(
        tree.lines().collect::<Vec<_>>(),
        ["skills/fixture/SKILL.md", "skills/fixture/extra.txt"]
    );
    assert!(skill.central_path.join("cache.db").is_file());
}

#[test]
fn backup_refuses_to_report_a_skill_with_no_entry_as_exported() {
    let tmp = tempfile::tempdir().unwrap();
    fixture_git(tmp.path(), &["init", "--bare", "remote.git"]);
    let remote = tmp.path().join("remote.git");
    let manager = UnoSkillsManager::open_at(tmp.path().join("library")).unwrap();
    let source = tmp.path().join("fixture");
    create_skill_dir(&source, "Fixture", "Entry safety", &[]);
    let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();
    std::fs::rename(
        skill.central_path.join("SKILL.md"),
        skill.central_path.join("README.md"),
    )
    .unwrap();
    let error = manager
        .git_backup_push_to(remote.to_str().unwrap(), "main")
        .unwrap_err()
        .to_string();
    assert!(error.contains("BACKUP_PATH_CONFLICT"), "{error}");
    assert_eq!(fixture_git(&remote, &["for-each-ref"]), "");
    std::fs::rename(
        skill.central_path.join("README.md"),
        skill.central_path.join("skill.md"),
    )
    .unwrap();
    assert!(manager.git_backup_push().unwrap().pushed);
    let reader = UnoSkillsManager::open_at(tmp.path().join("reader")).unwrap();
    assert_eq!(
        reader
            .git_backup_pull_from(remote.to_str().unwrap(), "main")
            .unwrap()
            .skills_imported,
        1
    );
}

#[test]
#[cfg(unix)]
fn backup_does_not_recreate_a_branch_deleted_between_fetch_and_push() {
    use std::{fs, os::unix::fs::PermissionsExt};

    let tmp = tempfile::tempdir().unwrap();
    fixture_git(tmp.path(), &["init", "--bare", "remote.git"]);
    let remote = tmp.path().join("remote.git");
    let root = tmp.path().join("library");
    let manager = UnoSkillsManager::open_at(root.clone()).unwrap();
    let source = tmp.path().join("fixture");
    create_skill_dir(&source, "Fixture", "Concurrent deletion", &[]);
    let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();
    assert!(
        manager
            .git_backup_push_to(remote.to_str().unwrap(), "backup")
            .unwrap()
            .pushed
    );
    let mirror = fs::read_dir(root.join("backup-mirrors"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fixture_git(&mirror, &["config", "core.hooksPath", ".git/hooks"]);
    let hook = mirror.join(".git/hooks/pre-commit");
    fs::write(
        &hook,
        "#!/bin/sh\ngit --git-dir=../../../remote.git update-ref -d refs/heads/backup\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        skill.central_path.join("extra.txt"),
        "Local edits after backup",
    )
    .unwrap();

    let error = manager.git_backup_push().unwrap_err().to_string();
    assert!(error.starts_with("git push "), "{error}");
    assert_eq!(fixture_git(&remote, &["for-each-ref"]), "");
    fs::remove_file(hook).unwrap();
    let retry = manager.git_backup_push().unwrap_err().to_string();
    assert!(retry.contains("BACKUP_REMOTE_CONFLICT"), "{retry}");
    assert_eq!(
        fs::read_to_string(skill.central_path.join("extra.txt")).unwrap(),
        "Local edits after backup"
    );
}

#[test]
fn backup_preserves_lf_and_crlf_with_git_line_ending_defaults_enabled() {
    let tmp = tempfile::tempdir().unwrap();
    fixture_git(tmp.path(), &["init", "--bare", "remote.git"]);
    let remote = tmp.path().join("remote.git");
    let url = remote.to_str().unwrap();
    let first = UnoSkillsManager::open_at(tmp.path().join("first")).unwrap();
    let second = UnoSkillsManager::open_at(tmp.path().join("second")).unwrap();
    for manager in [&first, &second] {
        assert_eq!(
            manager
                .git_backup_pull_from(url, "main")
                .unwrap()
                .skills_imported,
            0
        );
        let mirror = std::fs::read_dir(manager.root_dir().join("backup-mirrors"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fixture_git(&mirror, &["config", "core.autocrlf", "true"]);
        fixture_git(&mirror, &["config", "core.eol", "crlf"]);
    }

    let source = tmp.path().join("fixture");
    create_skill_dir(&source, "Fixture", "Backup line endings", &[]);
    let marker = std::fs::read_to_string(source.join("SKILL.md")).unwrap();
    let crlf_file = "First line\r\nSecond line\r\n";
    std::fs::write(source.join("extra.txt"), crlf_file).unwrap();
    first.install_from_ref(source.to_str().unwrap()).unwrap();
    assert!(first.git_backup_push().unwrap().pushed);
    assert_eq!(
        fixture_git(
            &remote,
            &["show", "refs/heads/main:skills/fixture/SKILL.md"]
        ),
        marker
    );
    assert_eq!(
        fixture_git(
            &remote,
            &["show", "refs/heads/main:skills/fixture/extra.txt"]
        ),
        crlf_file
    );

    assert_eq!(second.git_backup_pull().unwrap().skills_imported, 1);
    let restored = second.find_skill("fixture").unwrap().unwrap().central_path;
    assert_eq!(
        std::fs::read_to_string(restored.join("SKILL.md")).unwrap(),
        marker
    );
    assert_eq!(
        std::fs::read_to_string(restored.join("extra.txt")).unwrap(),
        crlf_file
    );
    assert!(!second.git_backup_push().unwrap().pushed);
}

#[test]
fn backup_retains_previous_git_exports_of_now_excluded_files() {
    let tmp = tempfile::tempdir().unwrap();
    fixture_git(tmp.path(), &["init", "--bare", "remote.git"]);
    fixture_git(tmp.path(), &["init", "-b", "main", "writer"]);
    let remote = tmp.path().join("remote.git");
    let writer = tmp.path().join("writer");
    let tracked_skill = writer.join("skills/fixture");
    create_skill_dir(&tracked_skill, "Fixture", "Previously backed up", &[]);
    let legacy_file = "skills/fixture/old.db";
    std::fs::write(writer.join(legacy_file), "Already tracked data").unwrap();
    fixture_git(&writer, &["add", "-f", "--", "skills"]);
    fixture_git(
        &writer,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "Existing backup",
        ],
    );
    let url = remote.to_str().unwrap();
    fixture_git(&writer, &["push", url, "main"]);

    let manager = UnoSkillsManager::open_at(tmp.path().join("library")).unwrap();
    assert_eq!(
        manager
            .git_backup_pull_from(url, "main")
            .unwrap()
            .skills_imported,
        1
    );
    let local = manager.find_skill("fixture").unwrap().unwrap();
    std::fs::write(local.central_path.join("old.db"), "Private local edit").unwrap();
    std::fs::write(local.central_path.join("extra.txt"), "Portable change").unwrap();
    assert!(manager.git_backup_push().unwrap().pushed);
    assert_eq!(
        fixture_git(
            &remote,
            &["show", &format!("refs/heads/main:{legacy_file}")]
        ),
        "Already tracked data"
    );
    assert_eq!(
        fixture_git(
            &remote,
            &["show", "refs/heads/main:skills/fixture/extra.txt"]
        ),
        "Portable change"
    );
    assert_eq!(
        std::fs::read_to_string(local.central_path.join("old.db")).unwrap(),
        "Private local edit"
    );
}

#[test]
fn failed_backup_target_write_preserves_the_previous_repo_and_branch() {
    let tmp = tempfile::tempdir().unwrap();
    fixture_git(tmp.path(), &["init", "--bare", "old.git"]);
    fixture_git(tmp.path(), &["init", "--bare", "new.git"]);
    let old = tmp.path().join("old.git");
    let new = tmp.path().join("new.git");
    let new_url = new.to_str().unwrap();
    let root = tmp.path().join("library");
    let manager = UnoSkillsManager::open_at(root.clone()).unwrap();
    let source = tmp.path().join("fixture");
    create_skill_dir(&source, "Fixture", "Atomic backup target", &[]);
    manager.install_from_ref(source.to_str().unwrap()).unwrap();
    manager
        .configure_git_backup(old.to_str().unwrap(), Some("retained"))
        .unwrap();
    let connection = rusqlite::Connection::open(root.join("unoskills.db")).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER refuse_branch BEFORE UPDATE ON settings
         WHEN NEW.key = 'git_backup_branch' AND NEW.value = 'denied'
         BEGIN SELECT RAISE(FAIL, 'Fixture rejected branch write'); END;",
        )
        .unwrap();
    for error in [
        manager
            .configure_git_backup(new_url, Some("denied"))
            .unwrap_err(),
        manager.git_backup_push_to(new_url, "denied").unwrap_err(),
        manager.git_backup_pull_from(new_url, "denied").unwrap_err(),
    ] {
        assert!(error.to_string().contains("Fixture rejected branch write"));
    }
    let second_handle = UnoSkillsManager::open_at(root).unwrap();
    let status = second_handle.git_backup_status().unwrap();
    assert_eq!(status.repo_url.as_deref(), old.to_str());
    assert_eq!(status.branch, "retained");
    assert_eq!(fixture_git(&new, &["for-each-ref"]), "");
    assert!(second_handle.git_backup_push().unwrap().pushed);
    assert_eq!(
        fixture_git(&old, &["for-each-ref", "--format=%(refname)"]),
        "refs/heads/retained\n"
    );
}

#[test]
fn backup_switch_keeps_the_previous_repository_and_branch_out_of_the_new_history() {
    let tmp = tempfile::tempdir().unwrap();
    fixture_git(tmp.path(), &["init", "--bare", "old.git"]);
    fixture_git(tmp.path(), &["init", "--bare", "new.git"]);
    let old = tmp.path().join("old.git");
    let new = tmp.path().join("new.git");
    let library = tmp.path().join("library");
    let manager = UnoSkillsManager::open_at(library.clone()).unwrap();
    let private = tmp.path().join("private-skill");
    create_skill_dir(&private, "Private", "Old repository only", &[]);
    let installed = manager.install_from_ref(private.to_str().unwrap()).unwrap();
    manager
        .configure_git_backup(old.to_str().unwrap(), Some("main"))
        .unwrap();
    assert!(manager.git_backup_push().unwrap().pushed);
    fixture_git(
        &library,
        &[
            "clone",
            "--branch",
            "main",
            old.to_str().unwrap(),
            "sync-repo",
        ],
    );
    manager.remove_skill(&installed.id).unwrap();

    manager
        .configure_git_backup(new.to_str().unwrap(), Some("main"))
        .unwrap();
    let pull = manager.git_backup_pull().unwrap();
    assert_eq!(pull.skills_imported, 0);
    assert!(manager.find_skill("private-skill").unwrap().is_none());
    assert!(
        library
            .join("sync-repo/skills/private-skill/SKILL.md")
            .is_file()
    );

    let shared = tmp.path().join("shared-skill");
    create_skill_dir(&shared, "Shared", "New repository", &[]);
    manager.install_from_ref(shared.to_str().unwrap()).unwrap();
    manager
        .configure_git_backup(new.to_str().unwrap(), Some("fresh"))
        .unwrap();
    assert!(manager.git_backup_push().unwrap().pushed);
    assert_eq!(
        fixture_git(&new, &["rev-list", "--count", "refs/heads/fresh"]).trim(),
        "1"
    );
    let tree = fixture_git(&new, &["ls-tree", "-r", "--name-only", "refs/heads/fresh"]);
    assert!(tree.contains("skills/shared-skill/SKILL.md"));
    assert!(!tree.contains("skills/private-skill"));
}

#[test]
fn backup_push_preserves_unmanaged_files_and_rejects_occupied_skill_paths() {
    let tmp = tempfile::tempdir().unwrap();
    fixture_git(tmp.path(), &["init", "--bare", "remote.git"]);
    fixture_git(tmp.path(), &["init", "-b", "main", "writer"]);
    let remote = tmp.path().join("remote.git");
    let writer = tmp.path().join("writer");
    for (path, content) in [
        (".gitignore", "# keep custom ignores\n"),
        ("skills/notes.md", "Manual notes\n"),
        ("stacks/README.md", "Manual readme\n"),
        ("stacks/config.toml", "custom = true\n"),
        ("skills/occupied/notes.md", "Unrelated directory\n"),
    ] {
        let file = writer.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, content).unwrap();
    }
    fixture_git(&writer, &["add", "-A"]);
    fixture_git(
        &writer,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "Manual files",
        ],
    );
    fixture_git(&writer, &["push", remote.to_str().unwrap(), "main"]);
    let manager = UnoSkillsManager::open_at(tmp.path().join("library")).unwrap();
    manager
        .configure_git_backup(remote.to_str().unwrap(), Some("main"))
        .unwrap();
    let source = tmp.path().join("portable-skill");
    create_skill_dir(&source, "Portable", "Backed up", &[]);
    manager.install_from_ref(source.to_str().unwrap()).unwrap();
    assert!(manager.git_backup_push().unwrap().pushed);
    let tree = fixture_git(
        &remote,
        &["ls-tree", "-r", "--name-only", "refs/heads/main"],
    );
    for retained in [
        ".gitignore",
        "skills/notes.md",
        "stacks/README.md",
        "stacks/config.toml",
        "skills/occupied/notes.md",
        "skills/portable-skill/SKILL.md",
    ] {
        assert!(
            tree.lines().any(|path| path == retained),
            "{retained} missing"
        );
    }
    assert_eq!(
        fixture_git(&remote, &["show", "refs/heads/main:.gitignore"]),
        "# keep custom ignores\n"
    );
    let occupied = tmp.path().join("occupied");
    create_skill_dir(&occupied, "Occupied", "Conflicting local folder", &[]);
    manager
        .install_from_ref(occupied.to_str().unwrap())
        .unwrap();
    let head = fixture_git(&remote, &["rev-parse", "refs/heads/main"]);
    assert!(
        manager
            .git_backup_push()
            .unwrap_err()
            .to_string()
            .contains("BACKUP_PATH_CONFLICT")
    );
    assert_eq!(
        fixture_git(&remote, &["rev-parse", "refs/heads/main"]),
        head
    );
}

#[test]
fn repeated_pull_keeps_remote_deletions_pending_until_locally_resolved() {
    let tmp = tempfile::tempdir().unwrap();
    fixture_git(tmp.path(), &["init", "--bare", "remote.git"]);
    let remote = tmp.path().join("remote.git");
    let primary = UnoSkillsManager::open_at(tmp.path().join("primary")).unwrap();
    let secondary = UnoSkillsManager::open_at(tmp.path().join("secondary")).unwrap();
    for manager in [&primary, &secondary] {
        manager
            .configure_git_backup(remote.to_str().unwrap(), Some("main"))
            .unwrap();
    }
    let source = tmp.path().join("removed-item");
    create_skill_dir(&source, "Removed", "Original", &[]);
    let skill = primary.install_from_ref(source.to_str().unwrap()).unwrap();
    let stack = primary
        .create_stack_from_skills(
            "Removed stack",
            "",
            "",
            vec![],
            std::slice::from_ref(&skill.id),
        )
        .unwrap();
    assert!(primary.git_backup_push().unwrap().pushed);
    assert_eq!(secondary.git_backup_pull().unwrap().skills_imported, 1);
    primary.remove_skill(&skill.id).unwrap();
    primary.delete_local_stack(&stack.id).unwrap();
    assert!(primary.git_backup_push().unwrap().pushed);

    let head = fixture_git(&remote, &["rev-parse", "refs/heads/main"]);
    for _ in 0..2 {
        let pull = secondary.git_backup_pull().unwrap();
        assert_eq!(pull.conflicts.len(), 2);
        assert!(pull.conflicts[0].contains("skills/removed-item"));
        assert!(pull.conflicts[1].contains("stacks/removed-stack"));
        assert!(secondary.find_skill("removed-item").unwrap().is_some());
        assert!(secondary.find_local_stack(&stack.id).unwrap().is_some());
        assert!(
            secondary
                .git_backup_push()
                .unwrap_err()
                .to_string()
                .contains("BACKUP_REMOTE_CONFLICT")
        );
        assert_eq!(
            fixture_git(&remote, &["rev-parse", "refs/heads/main"]),
            head
        );
    }
    secondary.remove_skill("removed-item").unwrap();
    secondary.delete_local_stack(&stack.id).unwrap();
    assert!(!secondary.git_backup_push().unwrap().pushed);
    assert!(secondary.git_backup_pull().unwrap().conflicts.is_empty());
}

fn fixture_git(dir: &std::path::Path, args: &[&str]) -> String {
    let result = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[test]
fn stale_backup_push_preserves_new_and_changed_remote_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let remote = tmp.path().join("backup.git");
    let init = std::process::Command::new("git")
        .args(["init", "--bare", "--quiet"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(init.success());
    let head = || {
        let result = std::process::Command::new("git")
            .arg("--git-dir")
            .arg(&remote)
            .args(["rev-parse", "refs/heads/main"])
            .output()
            .unwrap();
        assert!(result.status.success());
        result.stdout
    };
    let primary = UnoSkillsManager::open_at(tmp.path().join("primary")).unwrap();
    let second = UnoSkillsManager::open_at(tmp.path().join("second")).unwrap();
    for manager in [&primary, &second] {
        manager
            .configure_git_backup(remote.to_str().unwrap(), Some("main"))
            .unwrap();
    }
    let shared = tmp.path().join("shared");
    create_skill_dir(&shared, "Shared", "Initial", &[]);
    let shared = primary.install_from_ref(shared.to_str().unwrap()).unwrap();
    primary.git_backup_push().unwrap();
    second.git_backup_pull().unwrap();

    let remote_only = tmp.path().join("remote-only");
    create_skill_dir(&remote_only, "Remote Only", "New remote skill", &[]);
    primary
        .install_from_ref(remote_only.to_str().unwrap())
        .unwrap();
    primary
        .save_skill_markdown(
            &shared.id,
            "---\nname: Shared\ndescription: Remote changes\n---\n",
        )
        .unwrap();
    assert!(primary.git_backup_push().unwrap().pushed);
    let before = head();

    let second_only = tmp.path().join("second-only");
    create_skill_dir(&second_only, "Second Only", "New local skill", &[]);
    second
        .install_from_ref(second_only.to_str().unwrap())
        .unwrap();
    let error = second.git_backup_push().unwrap_err().to_string();
    assert!(error.contains("BACKUP_REMOTE_CONFLICT"));
    assert!(error.contains("skills/shared"));
    assert!(error.contains("skills/remote-only"));
    assert_eq!(head(), before);

    let pull = second.git_backup_pull().unwrap();
    assert_eq!((pull.skills_imported, pull.skills_updated), (1, 1));
    assert!(pull.conflicts.is_empty());
    assert!(second.git_backup_push().unwrap().pushed);
    let pull = primary.git_backup_pull().unwrap();
    assert_eq!(pull.skills_imported, 1);
    assert!(primary.find_skill("remote-only").unwrap().is_some());
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

    // A local edit through the app updates SQLite's hash, so that hash alone
    // does not show whether it is safe to replace the library files.
    let beta_b = mgr_b.find_skill("beta-skill").unwrap().unwrap();
    let local_beta = "---\nname: beta-skill\ndescription: Local beta edit\n---\n";
    mgr_b.save_skill_markdown(&beta_b.id, local_beta).unwrap();
    let deployed_alpha = custom_dir_b.join("alpha-skill/SKILL.md");
    std::fs::write(&deployed_alpha, "Local deployment edit").unwrap();
    let mut local_stack = mgr_b.list_local_stacks().unwrap().remove(0);
    local_stack.description = "Local stack edit".into();
    mgr_b.save_local_stack(&local_stack).unwrap();

    let remote_alpha = "---\nname: alpha-skill\ndescription: Alpha v3 remote\n---\n";
    let remote_beta = "---\nname: beta-skill\ndescription: Beta v2 remote\n---\n";
    mgr_a.save_skill_markdown(&alpha.id, remote_alpha).unwrap();
    mgr_a.save_skill_markdown(&beta.id, remote_beta).unwrap();
    let mut remote_stack = mgr_a.list_local_stacks().unwrap().remove(0);
    remote_stack.description = "Remote stack edit".into();
    mgr_a.save_local_stack(&remote_stack).unwrap();
    assert!(mgr_a.git_backup_push().unwrap().pushed);

    // Reopening the manager must preserve the last synchronized fingerprints.
    let mgr_b = UnoSkillsManager::open_at(tmp.path().join("machine-b")).unwrap();
    let pull3 = mgr_b.git_backup_pull().unwrap();
    assert_eq!(pull3.skills_updated, 1);
    assert_eq!(pull3.refreshed_copies, 0);
    assert_eq!(pull3.stacks_imported, 0);
    assert_eq!(pull3.conflicts.len(), 3);
    assert!(
        pull3
            .conflicts
            .iter()
            .any(|issue| issue.contains("beta-skill"))
    );
    assert!(
        pull3
            .conflicts
            .iter()
            .any(|issue| issue.contains("sync-combo.toml"))
    );
    assert!(
        pull3.conflicts.iter().any(
            |issue| issue.contains(deployed_alpha.parent().unwrap().to_string_lossy().as_ref())
        )
    );
    assert_eq!(
        std::fs::read_to_string(&deployed_alpha).unwrap(),
        "Local deployment edit"
    );
    assert_eq!(
        std::fs::read_to_string(beta_b.central_path.join("SKILL.md")).unwrap(),
        local_beta
    );
    assert_eq!(
        std::fs::read_to_string(alpha_b.central_path.join("SKILL.md")).unwrap(),
        remote_alpha
    );
    assert_eq!(mgr_b.list_local_stacks().unwrap().remove(0), local_stack);
    assert!(mgr_b.has_skill_rollback(&alpha_b.id));

    // Retrying a pull does not promote skipped items to a new baseline.
    let retry = mgr_b.git_backup_pull().unwrap();
    assert_eq!(retry.skills_updated, 0);
    assert_eq!(retry.conflicts, pull3.conflicts);
    assert_eq!(mgr_b.list_local_stacks().unwrap().remove(0), local_stack);
}
