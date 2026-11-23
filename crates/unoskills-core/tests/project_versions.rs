use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use unoskills_core::{
    DeployMode, ProjectDeployOutcome, ProjectVersionIntent, ProjectWorkspace, Skill, StackManifest,
    StackSkillRef, StackSource, UnoSkillsManager,
};

struct Fixture {
    temp: TempDir,
    manager: UnoSkillsManager,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let manager = UnoSkillsManager::open_at(temp.path().join("data")).unwrap();
        let codex = temp.path().join("codex");
        fs::create_dir(&codex).unwrap();
        manager.set_agent_override("codex", codex.to_str()).unwrap();
        manager.set_default_deploy_mode(DeployMode::Copy).unwrap();
        Self { temp, manager }
    }

    fn skill(&self, slug: &str) -> (PathBuf, Skill) {
        let source = self.temp.path().join("sources").join(slug);
        fs::create_dir_all(source.join("docs/empty")).unwrap();
        fs::write(
            source.join("SKILL.md"),
            format!("# {slug}\nOriginal instructions\n"),
        )
        .unwrap();
        fs::write(source.join("docs/reference.txt"), "Original reference\n").unwrap();
        let skill = self
            .manager
            .install_from_ref(source.to_str().unwrap())
            .unwrap();
        (source, skill)
    }

    fn project(&self, name: &str) -> ProjectWorkspace {
        let path = self.temp.path().join(name);
        fs::create_dir_all(path.join("apps/web")).unwrap();
        self.manager.add_project(&path, None).unwrap()
    }
}

#[test]
fn stack_confirmation_applies_only_reviewed_rows_and_preserves_changed_targets() {
    let fixture = Fixture::new();
    let manager = &fixture.manager;
    let project = fixture.project("project");
    let scope = Path::new("apps/web");
    let directory = project.path.join(scope).join(".codex/skills");
    let mut stack = StackManifest {
        id: "qa".into(),
        name: "QA".into(),
        author: String::new(),
        description: String::new(),
        skills: Vec::new(),
        tags: Vec::new(),
        recommended_agents: Vec::new(),
        verified: false,
        source: StackSource::Local,
    };
    for slug in [
        "reviewer",
        "race",
        "safe",
        "global",
        "unavailable",
        "local",
        "safe",
        "changed-copy",
    ] {
        if slug != "unavailable" {
            fixture.skill(slug);
        }
        stack.skills.push(StackSkillRef {
            name: slug.into(),
            source: String::new(),
            note: String::new(),
        });
    }
    manager
        .deploy_skill("global", "codex", None, false)
        .unwrap();
    let copied = manager
        .deploy_skill_to_project_at("changed-copy", &project.id, scope, "codex", None, false)
        .unwrap();
    fs::write(
        copied.target_path.join("docs/reference.txt"),
        "Local reference",
    )
    .unwrap();
    fs::create_dir_all(directory.join("local")).unwrap();
    fs::write(directory.join("local/SKILL.md"), "Keep this skill").unwrap();

    let preview = manager
        .preview_project_stack(&stack, &project.id, "codex", scope)
        .unwrap();
    assert_eq!(
        preview
            .rows
            .iter()
            .map(|row| row.outcome)
            .collect::<Vec<_>>(),
        [
            ProjectDeployOutcome::Occupied,
            ProjectDeployOutcome::Duplicate,
            ProjectDeployOutcome::Occupied,
            ProjectDeployOutcome::Ready,
            ProjectDeployOutcome::Ready,
            ProjectDeployOutcome::Ready,
            ProjectDeployOutcome::GlobalOnly,
            ProjectDeployOutcome::Unavailable
        ]
    );
    assert_eq!(
        preview.rows[3].target.as_deref(),
        Some(Path::new("apps/web/.codex/skills/reviewer"))
    );
    assert!(!directory.join("safe").exists());

    // The source and two different destinations change while the dialog is open.
    fs::write(
        manager.skills_dir().join("reviewer/docs/reference.txt"),
        "Updated library version",
    )
    .unwrap();
    fs::create_dir(directory.join("race")).unwrap();
    fs::write(directory.join("race/SKILL.md"), "Appeared after preview").unwrap();
    fs::rename(directory.join("local"), project.path.join("saved-local")).unwrap();
    let result = manager
        .apply_project_stack_preview(&stack, &project.id, "codex", scope, preview, false)
        .unwrap();
    assert_eq!(result.deployed, 1);
    assert_eq!(result.global_available, 1);
    assert_eq!(
        result
            .skipped
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["local", "reviewer", "race"]
    );
    assert!(result.failed.is_empty());
    assert!(directory.join("safe/SKILL.md").is_file());
    let safe = manager.find_skill("safe").unwrap().unwrap();
    assert!(
        safe.project_deployments[0]
            .copy_hash
            .as_deref()
            .unwrap()
            .starts_with("v2:")
    );
    assert!(!directory.join("local").exists());
    assert!(!directory.join("global").exists());
    assert_eq!(
        fs::read_to_string(directory.join("race/SKILL.md")).unwrap(),
        "Appeared after preview"
    );
    assert_eq!(
        fs::read_to_string(project.path.join("saved-local/SKILL.md")).unwrap(),
        "Keep this skill"
    );
    assert_eq!(
        fs::read_to_string(copied.target_path.join("docs/reference.txt")).unwrap(),
        "Local reference"
    );

    let root = manager
        .preview_project_stack(&stack, &project.id, "codex", Path::new(""))
        .unwrap();
    assert!(
        manager
            .apply_project_stack_preview(&stack, &project.id, "codex", scope, root, true)
            .is_err()
    );
    let global_member = stack.skills.remove(3);
    stack.skills = vec![global_member];
    let preview = manager
        .preview_project_stack(&stack, &project.id, "codex", scope)
        .unwrap();
    assert_eq!(
        manager
            .apply_project_stack_preview(&stack, &project.id, "codex", scope, preview, true)
            .unwrap()
            .deployed,
        1
    );
}

#[test]
fn project_lock_isolates_two_projects_and_applies_the_frozen_review_after_a_library_restore() {
    let fixture = Fixture::new();
    let manager = &fixture.manager;
    let (source, skill) = fixture.skill("reviewer");
    let project = fixture.project("locked");
    let other = fixture.project("tracking");
    let scope = Path::new("apps/web");
    for key in ["cline", "antigravity"] {
        let global = fixture.temp.path().join(key);
        fs::create_dir(&global).unwrap();
        manager.set_agent_override(key, global.to_str()).unwrap();
    }
    let pinned = manager
        .deploy_skill_to_project_at(
            &skill.id,
            &project.id,
            scope,
            "cline",
            Some(DeployMode::Symlink),
            false,
        )
        .unwrap();
    let peer = manager
        .deploy_skill_to_project_at(&skill.id, &project.id, scope, "antigravity", None, false)
        .unwrap();
    let separate = manager
        .deploy_skill_to_project_at(
            &skill.id,
            &project.id,
            scope,
            "codex",
            Some(DeployMode::Copy),
            false,
        )
        .unwrap();
    let following = manager
        .deploy_skill_to_project(
            &skill.id,
            &other.id,
            "codex",
            Some(DeployMode::Symlink),
            false,
        )
        .unwrap();
    assert_eq!(peer.target_path, pinned.target_path);
    assert_ne!(separate.target_path, pinned.target_path);
    manager
        .lock_project_skill(&project.id, scope, "cline", &skill.id)
        .unwrap();
    assert!(pinned.target_path.join("docs/empty").is_dir());
    assert!(separate.target_path.join("docs/empty").is_dir());

    let manager = UnoSkillsManager::open_at(manager.root_dir().to_owned()).unwrap();
    let inventory = || {
        manager
            .inspect_project_skills_at(&project.id, "cline", scope)
            .unwrap()
    };
    let original_hash = inventory().remove(0).locked_hash.unwrap();
    manager
        .save_skill_markdown(&skill.id, "# reviewer\nLibrary editor changed this\n")
        .unwrap();
    assert!(
        fs::read_to_string(following.target_path.join("SKILL.md"))
            .unwrap()
            .contains("Library editor")
    );
    assert!(
        fs::read_to_string(pinned.target_path.join("SKILL.md"))
            .unwrap()
            .contains("Original instructions")
    );
    fs::write(source.join("SKILL.md"), "# reviewer\nUpstream update\n").unwrap();
    fs::write(source.join("docs/reference.txt"), "Updated reference\n").unwrap();
    manager.update_skill(&skill.id).unwrap();
    assert_eq!(inventory()[0].library_different, Some(true));
    assert_eq!(
        fs::read_to_string(following.target_path.join("docs/reference.txt")).unwrap(),
        "Updated reference\n"
    );
    assert_eq!(
        fs::read_to_string(pinned.target_path.join("docs/reference.txt")).unwrap(),
        "Original reference\n"
    );

    let prepare = |intent| {
        manager
            .prepare_project_version(&project.id, scope, "cline", &skill.id, intent)
            .unwrap()
    };
    let reviewed = prepare(ProjectVersionIntent::Upgrade);
    let serialized = serde_json::to_value(reviewed.review()).unwrap();
    let diff = serialized["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "docs/reference.txt" || file["path"] == "docs\\reference.txt")
        .unwrap()["diff"]
        .as_str()
        .unwrap();
    assert!(diff.contains("- Original reference\n"));
    assert!(diff.contains("+ Updated reference\n"));
    let candidate = reviewed.review().candidate_hash.clone();
    manager
        .apply_skill_revision(manager.prepare_skill_rollback(&skill.id).unwrap().unwrap())
        .unwrap();
    assert_eq!(
        fs::read_to_string(following.target_path.join("docs/reference.txt")).unwrap(),
        "Original reference\n"
    );
    assert_eq!(
        inventory()[0].locked_hash.as_deref(),
        Some(original_hash.as_str())
    );
    manager.apply_project_version(reviewed).unwrap();
    assert_eq!(
        fs::read_to_string(pinned.target_path.join("docs/reference.txt")).unwrap(),
        "Updated reference\n"
    );
    assert_eq!(
        fs::read_to_string(separate.target_path.join("docs/reference.txt")).unwrap(),
        "Updated reference\n"
    );
    assert_eq!(
        inventory()[0].locked_hash.as_deref(),
        Some(candidate.as_str())
    );
    let skill_after = manager.find_skill(&skill.id).unwrap().unwrap();
    assert!(
        skill_after
            .project_deployments
            .iter()
            .filter(|dep| dep.project_id == project.id)
            .all(|dep| dep.locked_hash.as_deref() == Some(candidate.as_str()))
    );
    assert!(
        manager
            .inspect_project_skills(&project.id, "codex")
            .unwrap()
            .iter()
            .all(|skill| skill.locked_hash.is_none())
    );

    let follow = prepare(ProjectVersionIntent::Follow);
    let local = pinned.target_path.join("docs/reference.txt");
    fs::write(&local, "Project edit").unwrap();
    assert!(manager.apply_project_version(follow).is_err());
    assert_eq!(fs::read_to_string(&local).unwrap(), "Project edit");
    assert!(
        manager
            .deploy_skill_to_project_at(&skill.id, &project.id, scope, "cline", None, true)
            .is_err()
    );
    fs::write(&local, "Updated reference\n").unwrap();

    let review = prepare(ProjectVersionIntent::Upgrade);
    let frozen = fs::read_dir(manager.root_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".project-version-review-")
        })
        .unwrap()
        .join("snapshot/SKILL.md");
    fs::write(&frozen, "Tampered snapshot").unwrap();
    assert!(manager.apply_project_version(review).is_err());
    assert_eq!(fs::read_to_string(&local).unwrap(), "Updated reference\n");
    assert!(!frozen.exists());

    // An aborted database commit restores the old project files and the shared binding.
    let conn = rusqlite::Connection::open(manager.root_dir().join("unoskills.db")).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_version BEFORE DELETE ON project_skill_locks BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
    assert!(
        manager
            .apply_project_version(prepare(ProjectVersionIntent::Follow))
            .is_err()
    );
    assert_eq!(fs::read_to_string(&local).unwrap(), "Updated reference\n");
    assert_eq!(
        fs::read_to_string(separate.target_path.join("docs/reference.txt")).unwrap(),
        "Updated reference\n"
    );
    assert_eq!(
        inventory()[0].locked_hash.as_deref(),
        Some(candidate.as_str())
    );
    conn.execute_batch("DROP TRIGGER fail_version").unwrap();
    manager
        .apply_project_version(prepare(ProjectVersionIntent::Follow))
        .unwrap();
    assert_eq!(fs::read_to_string(&local).unwrap(), "Original reference\n");
    assert!(inventory()[0].locked_hash.is_none());
}

#[test]
fn legacy_pins_survive_the_last_undeploy_and_database_reopen() {
    let fixture = Fixture::new();
    let (_, skill) = fixture.skill("reviewer");
    let project = fixture.project("repo");
    let scope = Path::new("apps/web");
    let manager = fixture.manager;
    let nested = manager
        .deploy_skill_to_project_at(&skill.id, &project.id, scope, "codex", None, false)
        .unwrap();
    manager
        .lock_project_skill(&project.id, scope, "codex", &skill.id)
        .unwrap();
    let root = manager.root_dir().to_owned();
    drop(manager);

    // Recreate the previous schema with the lock stored on its deployment.
    let db = rusqlite::Connection::open(root.join("unoskills.db")).unwrap();
    db.execute_batch(
        "ALTER TABLE project_deployments ADD COLUMN locked_hash TEXT;
        UPDATE project_deployments SET locked_hash = (SELECT version_hash FROM project_skill_locks);
        DROP TABLE project_skill_locks;",
    )
    .unwrap();
    drop(db);
    let manager = UnoSkillsManager::open_at(root.clone()).unwrap();
    let original = manager
        .find_skill(&skill.id)
        .unwrap()
        .unwrap()
        .project_deployments[0]
        .locked_hash
        .clone();
    assert!(original.is_some());
    manager
        .undeploy_skill_from_project_at(&skill.id, &project.id, scope, "codex")
        .unwrap();
    assert!(!nested.target_path.exists());
    manager
        .save_skill_markdown(&skill.id, "# reviewer\nNew library version\n")
        .unwrap();
    manager
        .deploy_skill(&skill.id, "codex", None, false)
        .unwrap();
    drop(manager);

    let manager = UnoSkillsManager::open_at(root).unwrap();
    let root_placement = manager
        .deploy_skill_to_project(&skill.id, &project.id, "codex", None, false)
        .unwrap();
    assert!(root_placement.locked_hash.is_none());
    assert!(
        fs::read_to_string(root_placement.target_path.join("SKILL.md"))
            .unwrap()
            .contains("New library version")
    );
    let stack = manager
        .create_stack_from_skills("Reviewer", "", "", vec![], std::slice::from_ref(&skill.id))
        .unwrap();
    let preview = manager
        .preview_project_stack(&stack, &project.id, "codex", scope)
        .unwrap();
    assert_eq!(preview.rows[0].outcome, ProjectDeployOutcome::Ready);
    let applied = manager
        .apply_project_stack_preview(&stack, &project.id, "codex", scope, preview, false)
        .unwrap();
    assert_eq!(applied.deployed, 1);
    let skill_record = manager.find_skill(&skill.id).unwrap().unwrap();
    let restored = skill_record
        .project_deployments
        .iter()
        .find(|dep| dep.scope_path == scope)
        .unwrap();
    assert_eq!(restored.locked_hash, original);
    assert!(
        fs::read_to_string(nested.target_path.join("SKILL.md"))
            .unwrap()
            .contains("Original instructions")
    );

    manager
        .apply_project_version(
            manager
                .prepare_project_version(
                    &project.id,
                    scope,
                    "codex",
                    &skill.id,
                    ProjectVersionIntent::Follow,
                )
                .unwrap(),
        )
        .unwrap();
    manager
        .undeploy_skill_from_project_at(&skill.id, &project.id, scope, "codex")
        .unwrap();
    let redeployed = manager
        .deploy_skill_to_project_at(&skill.id, &project.id, scope, "codex", None, false)
        .unwrap();
    assert!(redeployed.locked_hash.is_none());
    assert!(
        fs::read_to_string(redeployed.target_path.join("SKILL.md"))
            .unwrap()
            .contains("New library version")
    );
}

#[test]
fn shared_stack_registration_is_reviewed_and_preserves_tracking_and_pinned_files() {
    for pinned in [false, true] {
        let fixture = Fixture::new();
        let manager = &fixture.manager;
        let (_, skill) = fixture.skill("reviewer");
        let project = fixture.project("repo");
        let scope = Path::new("");
        for agent in ["cline", "antigravity"] {
            manager
                .set_agent_override(agent, fixture.temp.path().join(agent).to_str())
                .unwrap();
        }
        let deployment = manager
            .deploy_skill_to_project(&skill.id, &project.id, "cline", None, false)
            .unwrap();
        if pinned {
            manager
                .lock_project_skill(&project.id, scope, "cline", &skill.id)
                .unwrap();
            manager
                .save_skill_markdown(&skill.id, "# reviewer\nCurrent library version\n")
                .unwrap();
        }
        let stack = manager
            .create_stack_from_skills("Reviewer", "", "", vec![], std::slice::from_ref(&skill.id))
            .unwrap();
        let preview = || {
            manager
                .preview_project_stack(&stack, &project.id, "antigravity", scope)
                .unwrap()
        };
        let confirm = |review| {
            manager
                .apply_project_stack_preview(
                    &stack,
                    &project.id,
                    "antigravity",
                    scope,
                    review,
                    false,
                )
                .unwrap()
        };
        let target = deployment.target_path;
        let file = target.join("SKILL.md");
        let bytes = fs::read(&file).unwrap();
        let first = preview();
        assert_eq!(first.rows[0].outcome, ProjectDeployOutcome::Register);
        assert_eq!(first.status.pending_count(false), 1);
        fs::write(&file, "Modified after review").unwrap();
        let stale = confirm(first);
        assert_eq!(stale.skipped.len(), 1);
        assert_eq!(stale.registered, 0);
        assert_eq!(fs::read_to_string(&file).unwrap(), "Modified after review");
        fs::write(&file, &bytes).unwrap();
        let timestamp = fs::metadata(&file).unwrap().modified().unwrap();
        let applied = confirm(preview());
        assert_eq!((applied.deployed, applied.registered), (0, 1));
        assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), timestamp);
        assert_eq!(
            preview().rows[0].outcome,
            ProjectDeployOutcome::AlreadyPresent
        );
        assert_eq!(
            manager
                .find_skill(&skill.id)
                .unwrap()
                .unwrap()
                .project_deployments
                .iter()
                .find(|dep| dep.agent_key == "antigravity")
                .unwrap()
                .locked_hash
                .is_some(),
            pinned
        );
        manager
            .undeploy_skill_from_project(&skill.id, &project.id, "cline")
            .unwrap();
        assert_eq!(fs::read(&file).unwrap(), bytes);
        manager
            .undeploy_skill_from_project(&skill.id, &project.id, "antigravity")
            .unwrap();
        assert!(!target.exists());
        if pinned {
            let restored = manager
                .deploy_skill_to_project(&skill.id, &project.id, "antigravity", None, false)
                .unwrap();
            assert!(restored.locked_hash.is_some());
            assert_eq!(fs::read(&file).unwrap(), bytes);
        }
    }
}

#[test]
fn project_review_snapshots_are_temporary_until_applied_and_shared_snapshots_survive() {
    let fixture = Fixture::new();
    let manager = &fixture.manager;
    let (_, skill) = fixture.skill("reviewer");
    let first = fixture.project("first");
    let second = fixture.project("second");
    let scope = Path::new("");
    for project in [&first, &second] {
        manager
            .deploy_skill_to_project(&skill.id, &project.id, "codex", None, false)
            .unwrap();
        manager
            .lock_project_skill(&project.id, scope, "codex", &skill.id)
            .unwrap();
    }
    let durable_count = || {
        fs::read_dir(manager.root_dir().join(".project-versions"))
            .unwrap()
            .count()
    };
    let prepare = |project: &ProjectWorkspace, intent| {
        manager
            .prepare_project_version(&project.id, scope, "codex", &skill.id, intent)
            .unwrap()
    };
    assert_eq!(durable_count(), 1);
    for version in 2..=4 {
        manager
            .save_skill_markdown(&skill.id, &format!("# reviewer\nVersion {version}\n"))
            .unwrap();
        drop(prepare(&first, ProjectVersionIntent::Upgrade));
        drop(prepare(&first, ProjectVersionIntent::Follow));
        assert_eq!(durable_count(), 1);
    }

    let db = rusqlite::Connection::open(manager.root_dir().join("unoskills.db")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_upgrade BEFORE UPDATE ON project_skill_locks BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
    assert!(
        manager
            .apply_project_version(prepare(&first, ProjectVersionIntent::Upgrade))
            .is_err()
    );
    assert_eq!(durable_count(), 1);
    db.execute_batch("DROP TRIGGER fail_upgrade").unwrap();
    manager
        .apply_project_version(prepare(&first, ProjectVersionIntent::Upgrade))
        .unwrap();
    assert_eq!(durable_count(), 2);
    manager
        .apply_project_version(prepare(&second, ProjectVersionIntent::Upgrade))
        .unwrap();
    assert_eq!(durable_count(), 1);
    manager
        .apply_project_version(prepare(&first, ProjectVersionIntent::Follow))
        .unwrap();
    assert_eq!(durable_count(), 1);
    manager
        .apply_project_version(prepare(&second, ProjectVersionIntent::Follow))
        .unwrap();
    assert_eq!(durable_count(), 0);
}
