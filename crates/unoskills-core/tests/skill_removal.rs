use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;
use unoskills_core::{
    AdoptCandidate, DeployMode, ProjectDeployment, ProjectWorkspace, Skill, UnoSkillsManager,
};

struct Fixture {
    temporary: TempDir,
    manager: UnoSkillsManager,
    skill: Skill,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let manager = UnoSkillsManager::open_at(temporary.path().join("library")).unwrap();
        let agent = temporary.path().join("agent");
        fs::create_dir(&agent).unwrap();
        manager.set_agent_override("codex", agent.to_str()).unwrap();
        let skill = install(&manager, temporary.path(), "reviewer", "Zebra Reviewer");
        Self {
            temporary,
            manager,
            skill,
        }
    }

    fn global(&self, mode: DeployMode) -> PathBuf {
        self.manager
            .deploy_skill(&self.skill.id, "codex", Some(mode), false)
            .unwrap()
            .target_path
    }

    fn project(&self, name: &str) -> (ProjectWorkspace, ProjectDeployment) {
        let path = self.temporary.path().join(name);
        fs::create_dir(&path).unwrap();
        let project = self.manager.add_project(&path, None).unwrap();
        let deployment = self
            .manager
            .deploy_skill_to_project(
                &self.skill.id,
                &project.id,
                "codex",
                Some(DeployMode::Copy),
                false,
            )
            .unwrap();
        (project, deployment)
    }

    fn saved(&self) -> Skill {
        self.manager.find_skill(&self.skill.id).unwrap().unwrap()
    }

    fn lock(&self, project: &ProjectWorkspace, skill: &Skill) -> String {
        self.manager
            .lock_project_skill(&project.id, Path::new(""), "codex", &skill.id)
            .unwrap();
        self.manager
            .find_skill(&skill.id)
            .unwrap()
            .unwrap()
            .project_deployments
            .into_iter()
            .find(|dep| dep.project_id == project.id)
            .unwrap()
            .locked_hash
            .unwrap()
    }

    fn database(&self) -> Connection {
        Connection::open(self.manager.root_dir().join("unoskills.db")).unwrap()
    }

    fn snapshot(&self, hash: &str) -> PathBuf {
        self.manager.root_dir().join(".project-versions").join(hash)
    }

    fn lock_count(&self, hash: &str) -> i64 {
        self.database()
            .query_row(
                "SELECT COUNT(*) FROM project_skill_locks WHERE version_hash = ?1",
                [hash],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn assert_intact(&self, global: &Path, project: &Path) {
        for target in [global, project, &self.skill.central_path] {
            assert!(target.join("SKILL.md").is_file());
            assert!(target.join("helper.sh").is_file());
        }
        let saved = self.saved();
        assert_eq!(
            (saved.deployments.len(), saved.project_deployments.len()),
            (1, 1)
        );
    }
}

fn install(manager: &UnoSkillsManager, root: &Path, slug: &str, name: &str) -> Skill {
    let source = root.join(slug);
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("SKILL.md"),
        format!("---\nname: {name}\n---\nTest instructions\n"),
    )
    .unwrap();
    fs::write(source.join("helper.sh"), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    set_mode(&source.join("helper.sh"), 0o644);
    manager.install_from_ref(source.to_str().unwrap()).unwrap()
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(unix)]
fn with_read_only_directory<T>(path: &Path, run: impl FnOnce() -> T) -> Option<T> {
    let saved = fs::metadata(path).unwrap().permissions();
    set_mode(path, 0o500);
    let result = match tempfile::tempdir_in(path) {
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => Some(run()),
        Ok(_) => {
            eprintln!("The current user can write to a read-only directory");
            None
        }
        Err(error) => panic!("Unexpected permission probe error: {error}"),
    };
    fs::set_permissions(path, saved).unwrap();
    result
}

#[test]
fn exact_skill_identity_wins_and_ambiguous_display_names_are_rejected() {
    let f = Fixture::new();
    let other = install(&f.manager, f.temporary.path(), "other", "reviewer");
    let by_id = install(&f.manager, f.temporary.path(), "id-collision", &f.skill.id);
    for query in ["reviewer", "  REVIEWER  ", &f.skill.id, "zebra REVIEWER"] {
        assert_eq!(f.manager.find_skill(query).unwrap().unwrap().id, f.skill.id);
    }
    f.manager.remove_skill("reviewer").unwrap();
    assert!(f.manager.find_skill(&by_id.id).unwrap().is_some());
    assert_eq!(
        f.manager.find_skill("Reviewer").unwrap().unwrap().id,
        other.id
    );

    let duplicate = install(&f.manager, f.temporary.path(), "another", "Reviewer");
    let error = f.manager.remove_skill("reviewer").unwrap_err().to_string();
    assert!(
        error.contains("AMBIGUOUS_SKILL")
            && error.contains(&other.id)
            && error.contains(&duplicate.id)
    );
    assert!(other.central_path.join("SKILL.md").is_file());
    assert!(duplicate.central_path.join("SKILL.md").is_file());
}

#[cfg(unix)]
#[test]
fn read_only_project_preflight_leaves_global_project_and_library_intact() {
    let f = Fixture::new();
    let global = f.global(DeployMode::Copy);
    let (_, project) = f.project("project");
    let parent = project.target_path.parent().unwrap();
    let Some(result) = with_read_only_directory(parent, || f.manager.remove_skill(&f.skill.id))
    else {
        return;
    };

    let error = result.unwrap_err();
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::PermissionDenied,
    );
    f.assert_intact(&global, &project.target_path);
    f.manager.remove_skill(&f.skill.id).unwrap();
    assert!(!global.exists() && !project.target_path.exists() && !f.skill.central_path.exists());
    assert!(f.manager.find_skill(&f.skill.id).unwrap().is_none());
}

#[test]
fn database_failure_restores_staged_deployments_and_library_before_retry() {
    let f = Fixture::new();
    let global = f.global(DeployMode::Symlink);
    let (_, project) = f.project("project");
    let db = f.database();
    db.execute_batch(
        "CREATE TRIGGER deny_removal BEFORE DELETE ON skills
         BEGIN SELECT RAISE(ABORT, 'Simulated database failure'); END;",
    )
    .unwrap();
    let error = f.manager.remove_skill(&f.skill.id).unwrap_err();
    assert!(error.to_string().contains("Simulated database failure"));
    f.assert_intact(&global, &project.target_path);
    db.execute_batch("DROP TRIGGER deny_removal").unwrap();
    f.manager.remove_skill(&f.skill.id).unwrap();
    assert!(fs::symlink_metadata(global).is_err());
    assert!(!project.target_path.exists() && !f.skill.central_path.exists());
}

#[test]
fn permanent_removal_stages_a_shared_target_once_and_tolerates_missing_targets() {
    let f = Fixture::new();
    let global = f.global(DeployMode::Copy);
    f.manager
        .set_agent_override("claude_code", global.parent().unwrap().to_str())
        .unwrap();
    let peer = f
        .manager
        .deploy_skill(&f.skill.id, "claude_code", Some(DeployMode::Copy), false)
        .unwrap();
    assert_eq!(peer.target_path, global);
    assert_eq!(f.saved().deployments.len(), 2);

    let (_, project) = f.project("project");
    fs::remove_dir_all(&project.target_path).unwrap();
    f.manager.remove_skill(&f.skill.id).unwrap();
    assert!(!global.exists() && !f.skill.central_path.exists());
    assert!(f.manager.find_skill(&f.skill.id).unwrap().is_none());
}

#[test]
fn permanent_skill_removal_reclaims_its_snapshot_after_the_last_skill_reference() {
    let f = Fixture::new();
    let (project, deployment) = f.project("project");
    let hash = f.lock(&project, &f.skill);
    let other = install(
        &f.manager,
        f.temporary.path(),
        "same-content",
        &f.skill.name,
    );
    f.manager
        .deploy_skill_to_project(
            &other.id,
            &project.id,
            "codex",
            Some(DeployMode::Copy),
            false,
        )
        .unwrap();
    assert_eq!(f.lock(&project, &other), hash);
    let snapshot = f.snapshot(&hash);
    assert_eq!(f.lock_count(&hash), 2);

    f.manager.remove_skill(&f.skill.id).unwrap();
    assert!(!f.skill.central_path.exists() && !deployment.target_path.exists());
    assert_eq!(f.lock_count(&hash), 1);
    assert!(snapshot.join("helper.sh").is_file());

    f.manager
        .undeploy_skill_from_project(&other.id, &project.id, "codex")
        .unwrap();
    assert_eq!(f.lock_count(&hash), 1);
    assert!(snapshot.join("SKILL.md").is_file());
    f.manager.remove_skill(&other.id).unwrap();
    assert_eq!(f.lock_count(&hash), 0);
    assert!(!snapshot.exists());
}

#[test]
fn project_detachment_reclaims_only_the_final_snapshot_including_durable_pins() {
    let f = Fixture::new();
    let (first, _) = f.project("first");
    let (second, _) = f.project("second");
    let hash = f.lock(&first, &f.skill);
    assert_eq!(f.lock(&second, &f.skill), hash);
    let snapshot = f.snapshot(&hash);

    f.manager.remove_project(&first.id).unwrap();
    assert_eq!(f.lock_count(&hash), 1);
    assert!(snapshot.join("SKILL.md").is_file());
    f.manager
        .undeploy_skill_from_project(&f.skill.id, &second.id, "codex")
        .unwrap();
    assert_eq!(f.lock_count(&hash), 1);
    assert!(snapshot.join("SKILL.md").is_file());
    f.manager.remove_project(&second.id).unwrap();
    assert_eq!(f.lock_count(&hash), 0);
    assert!(!snapshot.exists());
}

#[test]
fn missing_or_modified_snapshots_do_not_block_removal_or_delete_unverified_files() {
    for missing in [true, false] {
        let f = Fixture::new();
        let (project, deployment) = f.project("project");
        let hash = f.lock(&project, &f.skill);
        let snapshot = f.snapshot(&hash);
        let note = snapshot.join("notes.txt");
        if missing {
            fs::remove_dir_all(&snapshot).unwrap();
        } else {
            fs::write(&note, "Local file").unwrap();
        }
        f.manager.remove_project(&project.id).unwrap();
        assert_eq!(f.lock_count(&hash), 0);
        assert!(!deployment.target_path.exists());
        if !missing {
            assert_eq!(fs::read_to_string(note).unwrap(), "Local file");
        }
    }
}

#[cfg(unix)]
#[test]
fn inaccessible_snapshot_cleanup_preserves_the_snapshot_after_detach() {
    let f = Fixture::new();
    let (project, deployment) = f.project("project");
    let hash = f.lock(&project, &f.skill);
    let snapshot = f.snapshot(&hash);
    let repository = snapshot.parent().unwrap();
    let Some(result) =
        with_read_only_directory(repository, || f.manager.remove_project(&project.id))
    else {
        return;
    };

    result.unwrap();
    assert_eq!(f.lock_count(&hash), 0);
    assert!(!deployment.target_path.exists());
    assert!(snapshot.join("SKILL.md").is_file() && snapshot.join("helper.sh").is_file());
}

#[cfg(unix)]
#[test]
fn legacy_pinned_copy_uses_the_snapshot_metadata_when_the_library_has_changed() {
    let f = Fixture::new();
    set_mode(&f.skill.central_path.join("helper.sh"), 0o755);
    let (project, deployment) = f.project("project");
    let hash = f.lock(&project, &f.skill);
    f.database()
        .execute(
            "UPDATE project_deployments SET copy_hash = ?1",
            [f.skill.content_hash.as_deref().unwrap()],
        )
        .unwrap();

    fs::write(
        f.skill.central_path.join("helper.sh"),
        "New library revision",
    )
    .unwrap();
    let helper = deployment.target_path.join("helper.sh");
    set_mode(&helper, 0o744);
    let error = f
        .manager
        .undeploy_skill_from_project(&f.skill.id, &project.id, "codex")
        .unwrap_err();
    assert!(error.to_string().contains("TARGET_CONFLICT"));
    set_mode(&helper, 0o755);
    f.manager
        .undeploy_skill_from_project(&f.skill.id, &project.id, "codex")
        .unwrap();
    assert!(!deployment.target_path.exists());
    assert_eq!(f.lock_count(&hash), 1);
    assert!(f.snapshot(&hash).join("helper.sh").is_file());
}

fn check_copy_edit(change: impl Fn(&Path), revert: impl Fn(&Path)) {
    let f = Fixture::new();
    let global = f.global(DeployMode::Copy);
    let (project, deployment) = f.project("project");
    for path in [&global, &deployment.target_path] {
        change(path);
    }
    for result in [
        f.manager.undeploy_skill(&f.skill.id, "codex"),
        f.manager
            .undeploy_skill_from_project(&f.skill.id, &project.id, "codex"),
        f.manager.remove_skill(&f.skill.id),
    ] {
        assert!(result.unwrap_err().to_string().contains("TARGET_CONFLICT"));
    }
    f.assert_intact(&global, &deployment.target_path);
    for path in [&global, &deployment.target_path] {
        revert(path);
    }
    f.manager.undeploy_skill(&f.skill.id, "codex").unwrap();
    f.manager
        .undeploy_skill_from_project(&f.skill.id, &project.id, "codex")
        .unwrap();
    assert!(!global.exists() && !deployment.target_path.exists());
    assert!(f.saved().deployments.is_empty() && f.saved().project_deployments.is_empty());
}

#[test]
fn added_empty_directories_block_copy_removal() {
    check_copy_edit(
        |path| fs::create_dir(path.join("local-empty")).unwrap(),
        |path| fs::remove_dir(path.join("local-empty")).unwrap(),
    );
}

#[cfg(unix)]
#[test]
fn executable_bit_edits_block_copy_removal() {
    check_copy_edit(
        |path| set_mode(&path.join("helper.sh"), 0o755),
        |path| set_mode(&path.join("helper.sh"), 0o644),
    );
}

#[test]
fn file_content_edits_block_copy_removal() {
    check_copy_edit(
        |path| fs::write(path.join("helper.sh"), "Edited locally").unwrap(),
        |path| {
            let helper = path.join("helper.sh");
            assert_eq!(fs::read_to_string(&helper).unwrap(), "Edited locally");
            fs::write(helper, "#!/bin/sh\nexit 0\n").unwrap();
        },
    );
}

#[test]
fn adopting_a_copy_records_its_tree_fingerprint() {
    let f = Fixture::new();
    let path = f.temporary.path().join("agent/adopted");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("SKILL.md"), "An unmanaged skill").unwrap();
    f.manager.set_default_deploy_mode(DeployMode::Copy).unwrap();
    let skill = f
        .manager
        .adopt_candidate(&AdoptCandidate {
            agent_key: "codex".into(),
            agent_name: "Codex".into(),
            name: "adopted".into(),
            description: None,
            path: path.clone(),
            estimated_tokens: 0,
        })
        .unwrap();
    assert!(
        skill.deployments[0]
            .copy_hash
            .as_deref()
            .unwrap()
            .starts_with("v2:")
    );

    fs::create_dir(path.join("local-empty")).unwrap();
    let result = f.manager.undeploy_skill(&skill.id, "codex");
    assert!(result.unwrap_err().to_string().contains("TARGET_CONFLICT"));
    fs::remove_dir(path.join("local-empty")).unwrap();
    f.manager.undeploy_skill(&skill.id, "codex").unwrap();
    assert!(!path.exists());
}

#[test]
fn legacy_content_digests_need_an_independently_verifiable_tree() {
    let f = Fixture::new();
    let global = f.global(DeployMode::Copy);
    let (project, deployment) = f.project("project");
    let db = f.database();
    for table in ["deployments", "project_deployments"] {
        db.execute(
            &format!("UPDATE {table} SET copy_hash = ?1"),
            [f.skill.content_hash.as_deref().unwrap()],
        )
        .unwrap();
    }
    let empty = global.join("local-empty");
    fs::create_dir(&empty).unwrap();
    assert!(
        f.manager
            .undeploy_skill(&f.skill.id, "codex")
            .unwrap_err()
            .to_string()
            .contains("TARGET_CONFLICT")
    );
    fs::remove_dir(empty).unwrap();
    f.manager.undeploy_skill(&f.skill.id, "codex").unwrap();
    assert!(!global.exists());
    fs::write(f.skill.central_path.join("helper.sh"), "Library edit").unwrap();
    assert!(
        f.manager
            .undeploy_skill_from_project(&f.skill.id, &project.id, "codex")
            .unwrap_err()
            .to_string()
            .contains("TARGET_CONFLICT")
    );
    assert!(deployment.target_path.join("helper.sh").is_file());
    fs::write(
        f.skill.central_path.join("helper.sh"),
        "#!/bin/sh\nexit 0\n",
    )
    .unwrap();
    f.manager
        .undeploy_skill_from_project(&f.skill.id, &project.id, "codex")
        .unwrap();
    assert!(!deployment.target_path.exists());
}
