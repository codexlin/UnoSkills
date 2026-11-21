#[path = "../src/nsh/library/mod.rs"]
mod library;

mod nsh {
    pub fn dispatch(
        manager: &unoskills_core::UnoSkillsManager,
        command: &str,
        input: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        super::library::dispatch(manager, command, input)
    }
}

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use unoskills_core::{AgentCategory, CustomAgentRecord, DeployMode, UnoSkillsManager};

static COUNTER: AtomicU64 = AtomicU64::new(1);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(label: &str) -> Self {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "unoskills-nsh-library-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp workspace root");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn manager(&self) -> UnoSkillsManager {
        UnoSkillsManager::open_at(self.root.join("unoskills-home"))
            .expect("open isolated UnoSkillsManager")
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_skill_fixture(dir: &Path, body: &str) {
    fs::create_dir_all(dir).expect("create skill fixture dir");
    fs::write(dir.join("SKILL.md"), body).expect("write SKILL.md");
}

#[test]
fn library_install_markdown_preview_edit_lint_update_and_remove() {
    let ws = TempWorkspace::new("lifecycle");
    let mgr = ws.manager();

    // Empty state
    let empty_state = nsh::dispatch(&mgr, "library.state", json!({})).expect("empty library state");
    assert_eq!(empty_state["skills"].as_array().unwrap().len(), 0);
    assert_eq!(empty_state["deploy_mode"], "symlink");

    // Reject README-only directory (strict SKILL.md invariant)
    let readme_only = ws.path().join("readme-only");
    fs::create_dir_all(&readme_only).unwrap();
    fs::write(readme_only.join("README.md"), "# Not a skill").unwrap();
    let err = nsh::dispatch(
        &mgr,
        "discover.install",
        json!({ "source_ref": readme_only.to_string_lossy() }),
    )
    .expect_err("README-only folder must be rejected");
    assert!(err.to_string().contains("SKILL.md"));

    // Install valid local skill with YAML frontmatter
    let src = ws.path().join("alpha-skill");
    write_skill_fixture(
        &src,
        "---\nname: Alpha Skill\ndescription: Helps test NSH library flows\ntags:\n  - rust\n  - nsh\n---\n\n# Alpha Heading\n\nBody paragraph.\n",
    );
    let installed = nsh::dispatch(
        &mgr,
        "discover.install",
        json!({ "source_ref": src.to_string_lossy() }),
    )
    .expect("install valid local skill");
    let skill_id = installed["id"].as_str().unwrap().to_string();
    assert_eq!(installed["name"], "Alpha Skill");

    // Get markdown: raw preserves frontmatter, preview strips frontmatter
    let md = nsh::dispatch(
        &mgr,
        "library.get_markdown",
        json!({ "skill_id": skill_id }),
    )
    .expect("get markdown");
    assert_eq!(md["entry_file"], "SKILL.md");
    assert!(md["raw_markdown"].as_str().unwrap().starts_with("---\n"));
    let preview = md["preview_markdown"].as_str().unwrap();
    assert!(!preview.contains("tags:"));
    assert!(preview.contains("# Alpha Heading"));
    assert_eq!(md["lint"]["valid"], true);

    // Save markdown updates content, metadata, and lint report
    let updated_content = "---\nname: Alpha Renamed\ndescription: Updated description\ntags:\n  - updated\n---\n\n# Updated Body\n\nMore details here.\n";
    let saved = nsh::dispatch(
        &mgr,
        "library.save_markdown",
        json!({ "skill_id": skill_id, "content": updated_content }),
    )
    .expect("save markdown");
    assert_eq!(saved["skill"]["name"], "Alpha Renamed");
    assert!(
        saved["preview_markdown"]
            .as_str()
            .unwrap()
            .contains("# Updated Body")
    );
    assert!(
        !saved["preview_markdown"]
            .as_str()
            .unwrap()
            .contains("Updated description")
    );

    // Lint skill directly
    let lint = nsh::dispatch(&mgr, "library.lint_skill", json!({ "skill_id": skill_id }))
        .expect("lint skill");
    assert_eq!(lint["valid"], true);

    // Update skill from local source detects diff (since we edited central copy)
    let update_res = nsh::dispatch(
        &mgr,
        "library.update_skill",
        json!({ "skill_id": skill_id }),
    )
    .expect("update skill");
    assert_eq!(update_res["updated"], true);
    assert_eq!(update_res["skill"]["name"], "Alpha Skill");

    // Second update with no source changes reports updated == false
    let noop_update = nsh::dispatch(
        &mgr,
        "library.update_skill",
        json!({ "skill_id": skill_id }),
    )
    .expect("second update skill");
    assert_eq!(noop_update["updated"], false);

    // Remove skill cleans up DB and central directory
    let removed = nsh::dispatch(
        &mgr,
        "library.remove_skill",
        json!({ "skill_id": skill_id }),
    )
    .expect("remove skill");
    assert_eq!(removed["removed"], true);
    let after = nsh::dispatch(&mgr, "library.state", json!({})).expect("state after remove");
    assert_eq!(after["skills"].as_array().unwrap().len(), 0);
}

#[test]
fn library_global_deploy_conflict_force_and_shared_dir_refcount() {
    let ws = TempWorkspace::new("deploy");
    let mgr = ws.manager();

    // Register two custom agents sharing the same physical skills directory
    let shared_dir = ws.path().join("shared-agent-skills");
    fs::create_dir_all(&shared_dir).unwrap();
    mgr.add_custom_agent(&CustomAgentRecord {
        key: "agent-a".into(),
        display_name: "Agent A".into(),
        category: AgentCategory::Coding,
        skills_dir: shared_dir.to_string_lossy().into_owned(),
    })
    .unwrap();
    mgr.add_custom_agent(&CustomAgentRecord {
        key: "agent-b".into(),
        display_name: "Agent B".into(),
        category: AgentCategory::Coding,
        skills_dir: shared_dir.to_string_lossy().into_owned(),
    })
    .unwrap();

    // Install skill
    let src = ws.path().join("beta-skill");
    write_skill_fixture(&src, "---\nname: Beta Skill\n---\n\n# Beta\n");
    let installed = nsh::dispatch(
        &mgr,
        "discover.install",
        json!({ "source_ref": src.to_string_lossy() }),
    )
    .unwrap();
    let skill_id = installed["id"].as_str().unwrap().to_string();
    let slug = installed["slug"].as_str().unwrap().to_string();

    // Create unmanaged conflicting folder at target path
    let conflict_target = shared_dir.join(&slug);
    fs::create_dir_all(&conflict_target).unwrap();
    fs::write(conflict_target.join("custom.txt"), "user data").unwrap();

    // Default deploy (force=false) must refuse with TARGET_CONFLICT
    let err = nsh::dispatch(
        &mgr,
        "library.deploy_skill",
        json!({ "skill_id": skill_id, "agent_key": "agent-a" }),
    )
    .expect_err("unmanaged conflict must fail without force");
    assert!(err.to_string().contains("TARGET_CONFLICT"));
    assert!(conflict_target.join("custom.txt").exists());

    // User-confirmed deploy (force=true) succeeds
    let dep_a = nsh::dispatch(
        &mgr,
        "library.deploy_skill",
        json!({ "skill_id": skill_id, "agent_key": "agent-a", "force": true }),
    )
    .expect("force deploy agent-a");
    assert_eq!(dep_a["agent_key"], "agent-a");

    // Deploy to agent-b sharing the same directory succeeds without conflict
    let dep_b = nsh::dispatch(
        &mgr,
        "library.deploy_skill",
        json!({ "skill_id": skill_id, "agent_key": "agent-b" }),
    )
    .expect("deploy agent-b to shared dir");
    assert_eq!(dep_b["agent_key"], "agent-b");

    // Undeploy agent-a: on-disk target MUST stay because agent-b still references it!
    nsh::dispatch(
        &mgr,
        "library.undeploy_skill",
        json!({ "skill_id": skill_id, "agent_key": "agent-a" }),
    )
    .expect("undeploy agent-a");
    assert!(fs::symlink_metadata(&conflict_target).is_ok());

    // Undeploy agent-b: last reference removed, on-disk link must now be cleaned up!
    nsh::dispatch(
        &mgr,
        "library.undeploy_skill",
        json!({ "skill_id": skill_id, "agent_key": "agent-b" }),
    )
    .expect("undeploy agent-b");
    assert!(fs::symlink_metadata(&conflict_target).is_err());
}

#[test]
fn library_project_scope_statuses_deploy_conflict_and_detach() {
    let ws = TempWorkspace::new("project");
    let mgr = ws.manager();

    // Register an installed custom agent so project_skill_statuses includes it
    let agent_skills = ws.path().join("custom-agent-skills");
    fs::create_dir_all(&agent_skills).unwrap();
    mgr.add_custom_agent(&CustomAgentRecord {
        key: "my-agent".into(),
        display_name: "My Agent".into(),
        category: AgentCategory::Coding,
        skills_dir: agent_skills.to_string_lossy().into_owned(),
    })
    .unwrap();

    // Install skill
    let src = ws.path().join("gamma-skill");
    write_skill_fixture(&src, "---\nname: Gamma Skill\n---\n\n# Gamma\n");
    let installed = nsh::dispatch(
        &mgr,
        "discover.install",
        json!({ "source_ref": src.to_string_lossy() }),
    )
    .unwrap();
    let skill_id = installed["id"].as_str().unwrap().to_string();
    let slug = installed["slug"].as_str().unwrap().to_string();

    // Attach project
    let project_dir = ws.path().join("demo-repo");
    fs::create_dir_all(&project_dir).unwrap();
    let project = nsh::dispatch(
        &mgr,
        "library.attach_project",
        json!({ "path": project_dir.to_string_lossy(), "name": "Demo Repo" }),
    )
    .expect("attach project");
    let project_id = project["id"].as_str().unwrap().to_string();
    assert_eq!(project["name"], "Demo Repo");

    // Check initial project statuses: missing
    let statuses = nsh::dispatch(
        &mgr,
        "library.project_statuses",
        json!({ "project_id": project_id }),
    )
    .expect("project statuses");
    assert_eq!(
        statuses["statuses"]["my-agent"][&skill_id]["placement"],
        "missing"
    );

    // Create conflicting unmanaged folder inside project skills dir (.my-agent/skills/<slug>)
    let project_target = project_dir.join(".my-agent").join("skills").join(&slug);
    fs::create_dir_all(&project_target).unwrap();
    fs::write(project_target.join("README.md"), "unmanaged").unwrap();

    // Status should report conflict
    let state_with_proj = nsh::dispatch(&mgr, "library.state", json!({ "project_id": project_id }))
        .expect("library state with project");
    assert_eq!(
        state_with_proj["project_statuses"]["my-agent"][&skill_id]["placement"],
        "conflict"
    );

    // Non-force deploy fails with TARGET_CONFLICT
    let err = nsh::dispatch(
        &mgr,
        "library.deploy_project_skill",
        json!({
            "skill_id": skill_id,
            "project_id": project_id,
            "agent_key": "my-agent"
        }),
    )
    .expect_err("project deploy conflict must refuse");
    assert!(err.to_string().contains("TARGET_CONFLICT"));

    // Force deploy resolves conflict and becomes managed
    let dep = nsh::dispatch(
        &mgr,
        "library.deploy_project_skill",
        json!({
            "skill_id": skill_id,
            "project_id": project_id,
            "agent_key": "my-agent",
            "force": true
        }),
    )
    .expect("force deploy project skill");
    assert_eq!(dep["project_id"], project_id);

    let after_deploy = nsh::dispatch(
        &mgr,
        "library.project_statuses",
        json!({ "project_id": project_id }),
    )
    .unwrap();
    assert_eq!(
        after_deploy["statuses"]["my-agent"][&skill_id]["placement"],
        "managed"
    );

    // Detach project cleans up managed project target while keeping central skill
    nsh::dispatch(
        &mgr,
        "library.detach_project",
        json!({ "project_id": project_id }),
    )
    .expect("detach project");
    assert!(fs::symlink_metadata(&project_target).is_err());
    assert!(mgr.find_skill(&skill_id).unwrap().is_some());
}

#[test]
fn discover_scan_and_adopt_local_skills() {
    let ws = TempWorkspace::new("adopt");
    let mgr = ws.manager();
    mgr.set_default_deploy_mode(DeployMode::Copy).unwrap();

    // Create a custom agent directory containing an unmanaged local skill
    let agent_skills = ws.path().join("local-agent-skills");
    let unmanaged_skill = agent_skills.join("local-helper");
    write_skill_fixture(
        &unmanaged_skill,
        "---\nname: Local Helper\ndescription: Unmanaged local skill\n---\n\n# Local Helper\n",
    );
    mgr.add_custom_agent(&CustomAgentRecord {
        key: "local-agent".into(),
        display_name: "Local Agent".into(),
        category: AgentCategory::Coding,
        skills_dir: agent_skills.to_string_lossy().into_owned(),
    })
    .unwrap();

    // Scan adoptable
    let scan = nsh::dispatch(&mgr, "discover.scan_adoptable", json!({})).expect("scan adoptable");
    let candidates = scan["candidates"].as_array().unwrap();
    let ours: Vec<_> = candidates
        .iter()
        .filter(|c| c["agent_key"] == "local-agent")
        .cloned()
        .collect();
    assert_eq!(ours.len(), 1);
    assert_eq!(ours[0]["name"], "Local Helper");

    // Adopt selected candidate
    let adopted = nsh::dispatch(&mgr, "discover.adopt", json!({ "candidates": ours }))
        .expect("adopt candidate");
    assert_eq!(adopted["adopted_count"], 1);
    assert_eq!(adopted["failed"].as_array().unwrap().len(), 0);
    assert_eq!(adopted["adopted"][0]["name"], "Local Helper");

    // Re-scan should no longer list the adopted local-agent skill
    let rescan = nsh::dispatch(&mgr, "discover.scan_adoptable", json!({})).expect("rescan");
    let remaining: Vec<_> = rescan["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["agent_key"] == "local-agent")
        .collect();
    assert!(remaining.is_empty());
}

#[test]
fn rejects_invalid_commands_and_payloads() {
    let ws = TempWorkspace::new("validation");
    let mgr = ws.manager();

    assert!(nsh::dispatch(&mgr, "unknown.cmd", json!({})).is_err());
    assert!(nsh::dispatch(&mgr, "library.unknown", json!({})).is_err());
    assert!(nsh::dispatch(&mgr, "discover.unknown", json!({})).is_err());
    assert!(nsh::dispatch(&mgr, "discover.install", json!({ "source_ref": "   " })).is_err());
    assert!(nsh::dispatch(&mgr, "discover.leaderboard", json!({ "board": "bogus" })).is_err());
    assert!(nsh::dispatch(&mgr, "discover.search", json!({ "query": "" })).is_err());
    assert!(nsh::dispatch(&mgr, "discover.adopt", json!({ "candidates": [] })).is_err());
    assert!(
        nsh::dispatch(
            &mgr,
            "library.deploy_skill",
            json!({ "skill_id": "x", "agent_key": "y", "mode": "hardlink" })
        )
        .is_err()
    );
    assert!(
        nsh::dispatch(
            &mgr,
            "library.reveal_path",
            json!({ "path": ws.path().to_string_lossy() })
        )
        .is_err()
    );
}

#[test]
fn library_scoped_monorepo_root_and_subproject_targets_are_independent() {
    let ws = TempWorkspace::new("monorepo-scope");
    let mgr = ws.manager();

    let agent_skills = ws.path().join("mono-agent-skills");
    fs::create_dir_all(&agent_skills).unwrap();
    mgr.add_custom_agent(&CustomAgentRecord {
        key: "mono-agent".into(),
        display_name: "Mono Agent".into(),
        category: AgentCategory::Coding,
        skills_dir: agent_skills.to_string_lossy().into_owned(),
    })
    .unwrap();

    let src = ws.path().join("delta-skill");
    write_skill_fixture(&src, "---\nname: Delta Skill\n---\n\n# Delta\n");
    let installed = nsh::dispatch(
        &mgr,
        "discover.install",
        json!({ "source_ref": src.to_string_lossy() }),
    )
    .unwrap();
    let skill_id = installed["id"].as_str().unwrap().to_string();
    let slug = installed["slug"].as_str().unwrap().to_string();

    // Create a monorepo with root and nested apps/web subproject
    let repo_root = ws.path().join("monorepo");
    let web_dir = repo_root.join("apps").join("web");
    fs::create_dir_all(&web_dir).unwrap();

    let project = nsh::dispatch(
        &mgr,
        "library.attach_project",
        json!({ "path": repo_root.to_string_lossy(), "name": "Monorepo" }),
    )
    .unwrap();
    let project_id = project["id"].as_str().unwrap().to_string();

    let root_target = repo_root.join(".mono-agent").join("skills").join(&slug);
    let web_target = web_dir.join(".mono-agent").join("skills").join(&slug);

    // Path traversal scope must be rejected
    assert!(
        nsh::dispatch(
            &mgr,
            "library.project_statuses",
            json!({ "project_id": project_id, "scope_path": "../outside" })
        )
        .is_err()
    );

    // Deploy ONLY to apps/web -> root target must remain absent
    let web_dep = nsh::dispatch(
        &mgr,
        "library.deploy_project_skill",
        json!({
            "skill_id": skill_id,
            "project_id": project_id,
            "scope_path": "apps/web",
            "agent_key": "mono-agent"
        }),
    )
    .expect("deploy skill to apps/web");
    assert_eq!(web_dep["scope_path"], "apps/web");
    assert!(fs::symlink_metadata(&web_target).is_ok());
    assert!(fs::symlink_metadata(&root_target).is_err());

    // Scoped status queries agree with filesystem: apps/web=managed, root=missing
    let web_state = nsh::dispatch(
        &mgr,
        "library.state",
        json!({ "project_id": project_id, "scope_path": "apps/web" }),
    )
    .unwrap();
    assert_eq!(web_state["scope_path"], "apps/web");
    assert_eq!(
        web_state["project_statuses"]["mono-agent"][&skill_id]["placement"],
        "managed"
    );

    let root_statuses = nsh::dispatch(
        &mgr,
        "library.project_statuses",
        json!({ "project_id": project_id, "scope_path": "" }),
    )
    .unwrap();
    assert_eq!(
        root_statuses["statuses"]["mono-agent"][&skill_id]["placement"],
        "missing"
    );

    // Deploy separately to root; both root and apps/web now exist independently
    nsh::dispatch(
        &mgr,
        "library.deploy_project_skill",
        json!({
            "skill_id": skill_id,
            "project_id": project_id,
            "scope_path": "",
            "agent_key": "mono-agent"
        }),
    )
    .expect("deploy skill to root");
    assert!(fs::symlink_metadata(&root_target).is_ok());
    assert!(fs::symlink_metadata(&web_target).is_ok());

    // Undeploy from apps/web -> removes ONLY apps/web, leaving root intact!
    nsh::dispatch(
        &mgr,
        "library.undeploy_project_skill",
        json!({
            "skill_id": skill_id,
            "project_id": project_id,
            "scope_path": "apps/web",
            "agent_key": "mono-agent"
        }),
    )
    .expect("undeploy skill from apps/web");
    assert!(fs::symlink_metadata(&web_target).is_err());
    assert!(fs::symlink_metadata(&root_target).is_ok());

    let root_after = nsh::dispatch(
        &mgr,
        "library.project_statuses",
        json!({ "project_id": project_id, "scope_path": "" }),
    )
    .unwrap();
    assert_eq!(
        root_after["statuses"]["mono-agent"][&skill_id]["placement"],
        "managed"
    );

    // Conflict in apps/web must fail without force, and force=true replaces ONLY apps/web
    fs::create_dir_all(&web_target).unwrap();
    fs::write(web_target.join("notes.txt"), "unmanaged").unwrap();
    let conflict_err = nsh::dispatch(
        &mgr,
        "library.deploy_project_skill",
        json!({
            "skill_id": skill_id,
            "project_id": project_id,
            "scope_path": "apps/web",
            "agent_key": "mono-agent"
        }),
    )
    .expect_err("apps/web conflict must refuse");
    assert!(conflict_err.to_string().contains("TARGET_CONFLICT"));

    nsh::dispatch(
        &mgr,
        "library.deploy_project_skill",
        json!({
            "skill_id": skill_id,
            "project_id": project_id,
            "scope_path": "apps/web",
            "agent_key": "mono-agent",
            "force": true
        }),
    )
    .expect("force deploy apps/web conflict");
    assert!(fs::symlink_metadata(&web_target).is_ok());
    assert!(fs::symlink_metadata(&root_target).is_ok());
}

#[test]
fn library_get_markdown_propagates_invalid_utf8_and_missing_file_errors() {
    let ws = TempWorkspace::new("md-errors");
    let mgr = ws.manager();

    let src_dir = ws.path().join("src-bad-md");
    write_skill_fixture(
        &src_dir,
        "---\nname: Bad Markdown\ndescription: Read failure regression\n---\n\n# Initial\n",
    );
    let installed = nsh::dispatch(
        &mgr,
        "discover.install",
        json!({ "source_ref": src_dir.to_string_lossy() }),
    )
    .expect("install skill");
    let skill_id = installed["id"].as_str().unwrap().to_string();
    let central_path = std::path::PathBuf::from(installed["central_path"].as_str().unwrap());

    // Corrupt SKILL.md with invalid UTF-8 bytes and verify checked read fails with an error
    fs::write(central_path.join("SKILL.md"), [0xff_u8, 0xfe_u8, 0x00_u8]).unwrap();
    let utf8_err = nsh::dispatch(
        &mgr,
        "library.get_markdown",
        json!({ "skill_id": skill_id }),
    )
    .expect_err("invalid UTF-8 SKILL.md must fail");
    let utf8_msg = format!("{utf8_err:#}");
    assert!(
        utf8_msg.contains("UTF-8") || utf8_msg.contains("Failed to read"),
        "unexpected error: {utf8_msg}"
    );

    // Removing SKILL.md must also propagate a read error instead of returning empty text
    fs::remove_file(central_path.join("SKILL.md")).unwrap();
    let missing_err = nsh::dispatch(
        &mgr,
        "library.get_markdown",
        json!({ "skill_id": skill_id }),
    )
    .expect_err("missing SKILL.md must fail");
    assert!(
        format!("{missing_err:#}").contains("Failed to read"),
        "unexpected missing-file error: {missing_err:#}"
    );
}

#[cfg(unix)]
#[test]
fn library_scope_path_preserves_exact_whitespace() {
    let ws = TempWorkspace::new("scope-whitespace");
    let mgr = ws.manager();
    mgr.set_default_deploy_mode(DeployMode::Copy).unwrap();

    let agent_dir = ws.path().join("space-agent-skills");
    fs::create_dir_all(&agent_dir).unwrap();
    mgr.add_custom_agent(&CustomAgentRecord {
        key: "space-agent".into(),
        display_name: "Space Agent".into(),
        category: AgentCategory::Coding,
        skills_dir: agent_dir.to_string_lossy().into_owned(),
    })
    .unwrap();

    let src_dir = ws.path().join("src-space-skill");
    write_skill_fixture(
        &src_dir,
        "---\nname: Space Skill\ndescription: Scoped whitespace preservation\n---\n\n# Space\n",
    );
    let installed = nsh::dispatch(
        &mgr,
        "discover.install",
        json!({ "source_ref": src_dir.to_string_lossy() }),
    )
    .unwrap();
    let skill_id = installed["id"].as_str().unwrap().to_string();
    let slug = installed["slug"].as_str().unwrap().to_string();

    let repo_dir = ws.path().join("space-repo");
    let exact_scope = " apps/web ";
    fs::create_dir_all(repo_dir.join(exact_scope)).unwrap();
    let attached = nsh::dispatch(
        &mgr,
        "library.attach_project",
        json!({ "path": repo_dir.to_string_lossy(), "name": "Space Repo" }),
    )
    .unwrap();
    let project_id = attached["id"].as_str().unwrap().to_string();

    nsh::dispatch(
        &mgr,
        "library.deploy_project_skill",
        json!({
            "skill_id": skill_id,
            "project_id": project_id,
            "scope_path": exact_scope,
            "agent_key": "space-agent"
        }),
    )
    .expect("deploy to scope with leading/trailing spaces");

    let scoped_target = repo_dir
        .join(exact_scope)
        .join(".space-agent/skills")
        .join(&slug);
    assert!(fs::symlink_metadata(&scoped_target).is_ok());

    let state = nsh::dispatch(
        &mgr,
        "library.state",
        json!({ "project_id": project_id, "scope_path": exact_scope }),
    )
    .unwrap();
    assert_eq!(state["scope_path"], exact_scope);
    assert_eq!(
        state["project_statuses"]["space-agent"][&skill_id]["placement"],
        "managed"
    );
}

#[test]
fn desktop_revisions_apply_only_a_live_preview() {
    let ws = TempWorkspace::new("revisions");
    let mgr = ws.manager();
    let source = ws.path().join("revision-demo");
    write_skill_fixture(&source, "# Original");
    let skill = mgr.install_from_ref(&source.to_string_lossy()).unwrap();
    write_skill_fixture(&source, "# New version");
    let previews = library::RevisionPreviews::default();
    let call = |command, input| previews.dispatch(&mgr, command, &input).unwrap();
    let plan = call(
        "library.prepare_skill_update",
        json!({ "skill_id": skill.id }),
    )
    .unwrap();
    assert_eq!(plan["files"][0]["path"], "SKILL.md");
    assert_eq!(
        mgr.read_skill_markdown_checked(&skill.id).unwrap().1,
        "# Original"
    );
    call(
        "library.cancel_skill_revision",
        json!({ "preview_id": plan["id"] }),
    )
    .unwrap();
    let failure = call(
        "library.apply_skill_revision",
        json!({ "preview_id": plan["id"] }),
    )
    .unwrap_err();
    assert!(failure.to_string().contains("STALE_SKILL_REVISION"));

    let plan = call(
        "library.prepare_skill_update",
        json!({ "skill_id": skill.id }),
    )
    .unwrap();
    let result = call(
        "library.apply_skill_revision",
        json!({ "preview_id": plan["id"] }),
    )
    .unwrap();
    assert_eq!(result["updated"], true);
    assert_eq!(
        mgr.read_skill_markdown_checked(&skill.id).unwrap().1,
        "# New version"
    );
    let state = nsh::dispatch(&mgr, "library.state", json!({})).unwrap();
    assert_eq!(state["rollback_skill_ids"], json!([skill.id]));

    let plan = call(
        "library.prepare_skill_rollback",
        json!({ "skill_id": skill.id }),
    )
    .unwrap();
    assert_eq!(plan["rollback"], true);
    call(
        "library.apply_skill_revision",
        json!({ "preview_id": plan["id"] }),
    )
    .unwrap();
    assert_eq!(
        mgr.read_skill_markdown_checked(&skill.id).unwrap().1,
        "# Original"
    );
}
