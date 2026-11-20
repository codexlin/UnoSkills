use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::domain::{ProjectWorkspace, Skill, StackManifest, StackSkillRef, StackSource};
use crate::manager::{
    StackProjectTarget, UnoSkillsManager, inspect_stack_project_target, project_skills_dir,
};
use crate::parser::{is_valid_skill_dir, sanitize_slug};
use crate::stacks::load_builtin_featured_stacks;

const PROJECT_RULES: &str = include_str!("../data/project_recommendations.toml");
const MAX_DIRECTORIES: usize = 128;
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Technology {
    NodeJs,
    TypeScript,
    React,
    NextJs,
    Playwright,
    Python,
    FastApi,
    Django,
    Rust,
    Go,
}

impl Technology {
    pub fn label(self) -> &'static str {
        match self {
            Self::NodeJs => "Node.js",
            Self::TypeScript => "TypeScript",
            Self::React => "React",
            Self::NextJs => "Next.js",
            Self::Playwright => "Playwright",
            Self::Python => "Python",
            Self::FastApi => "FastAPI",
            Self::Django => "Django",
            Self::Rust => "Rust",
            Self::Go => "Go",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationPurpose {
    ReactPerformance,
    Accessibility,
    Testing,
    Debugging,
}

#[derive(Clone, Debug)]
pub struct ProjectRecommendation {
    pub skill: StackSkillRef,
    pub technology: Technology,
    pub purpose: RecommendationPurpose,
    pub installed_slug: Option<String>,
}

pub struct ProjectModule {
    pub path: PathBuf,
    pub evidence: BTreeMap<Technology, PathBuf>,
    pub added: Vec<Technology>,
    pub recommendations: Vec<ProjectRecommendation>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SkillLocation {
    ProjectDeployment,
    ProjectChanged,
    ProjectLocal,
    Global,
}

pub struct ProjectSkill {
    pub name: String,
    pub path: PathBuf,
    pub location: SkillLocation,
}

#[derive(Deserialize)]
struct Rule {
    technologies: Vec<Technology>,
    skill: String,
    purpose: RecommendationPurpose,
    local_tags: Vec<String>,
}

#[derive(Deserialize)]
struct Rules {
    rules: Vec<Rule>,
}

type Evidence = BTreeMap<PathBuf, BTreeMap<Technology, PathBuf>>;
type DetectionSnapshot = BTreeMap<String, Vec<Technology>>;

impl UnoSkillsManager {
    pub fn analyze_project(&self, project_id: &str) -> Result<Vec<ProjectModule>> {
        let project = self.analysis_project(project_id)?;
        let evidence = detect_technologies(&project.path)?;
        let key = format!("project_technology_scan:{project_id}");
        let previous: Option<DetectionSnapshot> = self
            .get_setting(&key)?
            .and_then(|previous| serde_json::from_str(&previous).ok());
        let skills = self.list_skills()?;
        let featured = load_builtin_featured_stacks()?;
        let Rules { rules } = toml::from_str(PROJECT_RULES)?;
        let modules = evidence
            .iter()
            .map(|(path, evidence)| {
                let added = previous.as_ref().map_or_else(Vec::new, |previous| {
                    evidence
                        .keys()
                        .copied()
                        .filter(|technology| {
                            !previous
                                .get(path.to_string_lossy().as_ref())
                                .is_some_and(|old| old.contains(technology))
                        })
                        .collect()
                });
                ProjectModule {
                    path: path.clone(),
                    recommendations: recommend(evidence, &skills, &featured, &rules),
                    evidence: evidence.clone(),
                    added,
                }
            })
            .collect();
        let snapshot: DetectionSnapshot = evidence
            .iter()
            .map(|(path, technologies)| {
                (
                    path.to_string_lossy().into_owned(),
                    technologies.keys().copied().collect(),
                )
            })
            .collect();
        self.set_setting(&key, &serde_json::to_string(&snapshot)?)?;
        Ok(modules)
    }

    pub fn inspect_project_skills(
        &self,
        project_id: &str,
        agent_key: &str,
    ) -> Result<Vec<ProjectSkill>> {
        let project = self.analysis_project(project_id)?;
        let agent = self
            .list_agents()?
            .into_iter()
            .find(|agent| agent.key == agent_key && agent.installed && agent.enabled)
            .context("Agent is unavailable")?;
        let local = project.path.join(project_skills_dir(&agent));
        let skills = self.list_skills()?;
        let managed = skills
            .iter()
            .flat_map(|skill| {
                skill
                    .project_deployments
                    .iter()
                    .filter(|dep| dep.project_id == project.id && dep.agent_key == agent.key)
                    .map(move |dep| (dep.target_path.as_path(), skill))
            })
            .collect::<BTreeMap<_, _>>();
        let mut inventory = Vec::new();
        let mut seen = HashSet::new();
        for (root, project_scope) in std::iter::once((&local, true)).chain(
            std::iter::once(&agent.skills_dir)
                .chain(&agent.additional_scan_dirs)
                .map(|root| (root, false)),
        ) {
            let entries = match fs::read_dir(root) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(error).with_context(|| format!("Couldn't list {}", root.display()));
                }
            };
            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                if is_valid_skill_dir(&path) && seen.insert(path.clone()) {
                    let location = if !project_scope {
                        SkillLocation::Global
                    } else if let Some(skill) = managed.get(path.as_path()) {
                        match inspect_stack_project_target(skill, &project, &agent, &path) {
                            StackProjectTarget::Deployed => SkillLocation::ProjectDeployment,
                            _ => SkillLocation::ProjectChanged,
                        }
                    } else {
                        SkillLocation::ProjectLocal
                    };
                    inventory.push(ProjectSkill {
                        name: entry.file_name().to_string_lossy().into_owned(),
                        path,
                        location,
                    });
                }
            }
        }
        inventory
            .sort_by(|a, b| (a.location, &a.name, &a.path).cmp(&(b.location, &b.name, &b.path)));
        Ok(inventory)
    }

    pub fn save_project_skill_stack(
        &self,
        project_id: &str,
        name: String,
        skills: Vec<StackSkillRef>,
    ) -> Result<StackManifest> {
        self.analysis_project(project_id)?;
        ensure!(!skills.is_empty(), "Select at least one skill");
        let stack = StackManifest {
            id: format!(
                "{}-{}",
                sanitize_slug(&name).unwrap_or_else(|| "project".into()),
                uuid::Uuid::new_v4().simple()
            ),
            name,
            author: String::new(),
            description: String::new(),
            tags: Vec::new(),
            recommended_agents: Vec::new(),
            verified: false,
            source: StackSource::Local,
            skills,
        };
        self.save_local_stack(&stack)?;
        Ok(stack)
    }

    fn analysis_project(&self, project_id: &str) -> Result<ProjectWorkspace> {
        let project = self
            .list_projects()?
            .into_iter()
            .find(|project| project.id == project_id)
            .context("Project workspace not found")?;
        ensure!(
            project.path.is_dir(),
            "Project directory is missing: {}",
            project.path.display()
        );
        Ok(project)
    }
}

fn detect_technologies(root: &Path) -> Result<Evidence> {
    let root = root.canonicalize()?;
    let mut result = BTreeMap::new();
    for entry in WalkDir::new(&root)
        .max_depth(2)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0
                || (entry.file_type().is_dir()
                    && !entry.file_name().to_string_lossy().starts_with('.')
                    && !matches!(
                        entry.file_name().to_str(),
                        Some(
                            "node_modules"
                                | "target"
                                | "dist"
                                | "build"
                                | "vendor"
                                | "venv"
                                | "__pycache__"
                        )
                    ))
        })
        .filter_map(Result::ok)
        .take(MAX_DIRECTORIES)
    {
        let relative = entry.path().strip_prefix(&root)?.to_owned();
        let evidence = inspect_manifests(entry.path(), &relative);
        if !evidence.is_empty() {
            result.insert(relative, evidence);
        }
    }
    Ok(result)
}

fn read_manifest(dir: &Path, name: &str) -> Option<String> {
    let path = dir.join(name);
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    let mut content = String::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_string(&mut content)
        .ok()?;
    (content.len() as u64 <= MAX_MANIFEST_BYTES).then_some(content)
}

fn inspect_manifests(dir: &Path, relative: &Path) -> BTreeMap<Technology, PathBuf> {
    use Technology::*;
    let mut result = BTreeMap::new();
    let mut mark = |technology, file| {
        result
            .entry(technology)
            .or_insert_with(|| relative.join(file));
    };
    if let Some(package) = read_manifest(dir, "package.json")
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .filter(|value| value.is_object())
    {
        mark(NodeJs, "package.json");
        for (dependency, technology) in [
            ("typescript", TypeScript),
            ("react", React),
            ("next", NextJs),
            ("@playwright/test", Playwright),
            ("playwright", Playwright),
        ] {
            if ["dependencies", "devDependencies", "peerDependencies"]
                .iter()
                .any(|section| {
                    package
                        .get(section)
                        .and_then(|deps| deps.get(dependency))
                        .is_some()
                })
            {
                mark(technology, "package.json");
            }
        }
    }
    if read_manifest(dir, "tsconfig.json").is_some() {
        mark(TypeScript, "tsconfig.json");
    }
    if let Some(config) = [
        "playwright.config.ts",
        "playwright.config.js",
        "playwright.config.mjs",
    ]
    .into_iter()
    .find(|config| read_manifest(dir, config).is_some())
    {
        mark(Playwright, config);
    }
    if let Some(toml) =
        read_manifest(dir, "pyproject.toml").and_then(|text| text.parse::<toml::Table>().ok())
    {
        mark(Python, "pyproject.toml");
        let poetry = toml
            .get("tool")
            .and_then(|tool| tool.get("poetry"))
            .and_then(|poetry| poetry.get("dependencies"));
        let dependencies = toml
            .get("project")
            .and_then(|project| project.get("dependencies"))
            .and_then(toml::Value::as_array);
        for (package, technology) in [("fastapi", FastApi), ("django", Django)] {
            if poetry
                .and_then(toml::Value::as_table)
                .is_some_and(|dependencies| {
                    dependencies
                        .keys()
                        .any(|name| name.eq_ignore_ascii_case(package))
                })
                || dependencies.is_some_and(|dependencies| {
                    dependencies
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .any(|requirement| python_requirement_matches(requirement, package))
                })
            {
                mark(technology, "pyproject.toml");
            }
        }
    }
    if let Some(requirements) = read_manifest(dir, "requirements.txt") {
        mark(Python, "requirements.txt");
        for (package, technology) in [("fastapi", FastApi), ("django", Django)] {
            if requirements
                .lines()
                .any(|line| python_requirement_matches(line, package))
            {
                mark(technology, "requirements.txt");
            }
        }
    }
    if read_manifest(dir, "Cargo.toml")
        .and_then(|text| text.parse::<toml::Table>().ok())
        .is_some_and(|manifest| {
            manifest.contains_key("package") || manifest.contains_key("workspace")
        })
    {
        mark(Rust, "Cargo.toml");
    }
    if read_manifest(dir, "go.mod").is_some_and(|text| {
        text.lines()
            .any(|line| line.trim_start().starts_with("module "))
    }) {
        mark(Go, "go.mod");
    }
    result
}

fn python_requirement_matches(requirement: &str, name: &str) -> bool {
    requirement
        .trim_start()
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')))
        .next()
        .is_some_and(|package| package.eq_ignore_ascii_case(name))
}

fn recommend(
    evidence: &BTreeMap<Technology, PathBuf>,
    skills: &[Skill],
    featured: &[StackManifest],
    rules: &[Rule],
) -> Vec<ProjectRecommendation> {
    let mut recommendations: Vec<_> = rules
        .iter()
        .filter_map(|rule| {
            let technology = rule
                .technologies
                .iter()
                .copied()
                .find(|technology| evidence.contains_key(technology))?;
            let curated = featured
                .iter()
                .flat_map(|stack| &stack.skills)
                .find(|skill| skill.name == rule.skill)?;
            let local = curated.deployable_in(skills).or_else(|| {
                skills.iter().find(|skill| {
                    !rule.local_tags.is_empty()
                        && rule.local_tags.iter().all(|tag| {
                            skill
                                .tags
                                .iter()
                                .any(|value| value.eq_ignore_ascii_case(tag))
                        })
                })
            });
            let skill = if let Some(local) = local {
                StackSkillRef {
                    name: local.name.clone(),
                    source: local
                        .source_ref
                        .clone()
                        .unwrap_or_else(|| local.central_path.to_string_lossy().into_owned()),
                    note: String::new(),
                }
            } else if curated.installed_in(skills).is_some() {
                return None;
            } else {
                curated.clone()
            };
            Some(ProjectRecommendation {
                skill,
                technology,
                purpose: rule.purpose,
                installed_slug: local.map(|skill| skill.slug.clone()),
            })
        })
        .collect();
    recommendations.sort_by_key(|item| item.installed_slug.is_none());
    let mut seen = HashSet::new();
    recommendations.retain(|item| seen.insert(item.skill.source.clone()));
    recommendations.truncate(3);
    recommendations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzes_workspace_members_and_tracks_new_technology() {
        let sandbox = tempfile::tempdir().unwrap();
        let manager = UnoSkillsManager::open_at(sandbox.path().join("store")).unwrap();
        let repo = sandbox.path().join("repo");
        let web = repo.join("apps/web");
        let api = repo.join("services/api");
        fs::create_dir_all(&web).unwrap();
        fs::create_dir_all(&api).unwrap();
        fs::write(
            web.join("package.json"),
            r#"{"dependencies":{"next":"15","react":"19"},"devDependencies":{"typescript":"5"}}"#,
        )
        .unwrap();
        fs::write(
            api.join("pyproject.toml"),
            "[project]\nname='api'\ndependencies=['FastAPI[standard]>=0.110']",
        )
        .unwrap();
        let go = repo.join("services/go");
        let rust = repo.join("crates/core");
        fs::create_dir_all(&go).unwrap();
        fs::create_dir_all(&rust).unwrap();
        fs::write(go.join("go.mod"), "module example.org/tool\n").unwrap();
        fs::write(rust.join("Cargo.toml"), "[package]\nname='tool'\n").unwrap();
        fs::create_dir(repo.join("node_modules")).unwrap();
        fs::write(repo.join("node_modules/go.mod"), "module ignored.example\n").unwrap();
        let local_skill = sandbox.path().join("regression-tests");
        fs::create_dir(&local_skill).unwrap();
        fs::write(
            local_skill.join("SKILL.md"),
            "---\nname: regression-tests\ntags: [testing]\n---\n# Test recipe\n",
        )
        .unwrap();
        manager
            .install_from_ref(local_skill.to_str().unwrap())
            .unwrap();
        let project = manager.add_project(&repo, None).unwrap();

        let report = manager.analyze_project(&project.id).unwrap();
        assert_eq!(report.len(), 4);
        let frontend = report
            .iter()
            .find(|module| module.path == Path::new("apps/web"))
            .unwrap();
        assert_eq!(
            frontend.evidence.get(&Technology::NextJs),
            Some(&web.strip_prefix(&repo).unwrap().join("package.json"))
        );
        assert!(frontend.evidence.contains_key(&Technology::TypeScript));
        assert!(frontend.added.is_empty());
        assert_eq!(
            frontend.recommendations[0].installed_slug.as_deref(),
            Some("regression-tests")
        );
        assert!(
            report
                .iter()
                .any(|module| module.evidence.contains_key(&Technology::FastApi))
        );
        assert!(
            report
                .iter()
                .any(|module| module.evidence.contains_key(&Technology::Rust))
        );
        assert!(
            report
                .iter()
                .any(|module| module.evidence.contains_key(&Technology::Go))
        );

        fs::write(web.join("playwright.config.ts"), "export default {};").unwrap();
        let report = manager.analyze_project(&project.id).unwrap();
        let frontend = report
            .iter()
            .find(|module| module.path == Path::new("apps/web"))
            .unwrap();
        assert_eq!(frontend.added, [Technology::Playwright]);
        let selected = frontend
            .recommendations
            .iter()
            .take(2)
            .map(|item| item.skill.clone())
            .collect();
        let saved = manager
            .save_project_skill_stack(&project.id, "Web project".into(), selected)
            .unwrap();
        assert_eq!(saved.skills.len(), 2);
        assert_eq!(manager.list_local_stacks().unwrap()[0].id, saved.id);
        assert!(!repo.join(".codex").exists());
    }

    #[test]
    fn lists_project_files_and_global_skills_at_distinct_paths() {
        let sandbox = tempfile::tempdir().unwrap();
        let manager = UnoSkillsManager::open_at(sandbox.path().join("store")).unwrap();
        let repo = sandbox.path().join("repo");
        let local = repo.join(".qa_agent/skills/review");
        let global = sandbox.path().join("global");
        fs::create_dir_all(&local).unwrap();
        fs::create_dir_all(global.join("review")).unwrap();
        for path in [&local, &global.join("review")] {
            fs::write(path.join("SKILL.md"), "# Review").unwrap();
        }
        fs::create_dir_all(global.join("with-readme")).unwrap();
        fs::write(global.join("with-readme/README.md"), "# Readme").unwrap();
        manager
            .add_custom_agent(&crate::CustomAgentRecord {
                key: "qa_agent".into(),
                display_name: "QA Agent".into(),
                skills_dir: global.to_string_lossy().into_owned(),
                category: crate::AgentCategory::Coding,
            })
            .unwrap();
        let project = manager.add_project(&repo, None).unwrap();
        let files = manager
            .inspect_project_skills(&project.id, "qa_agent")
            .unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].location, SkillLocation::ProjectLocal);
        assert_eq!(files[0].path, project.path.join(".qa_agent/skills/review"));
        assert_eq!(files[1].location, SkillLocation::Global);
        assert_ne!(files[0].path, files[1].path);
    }
}
