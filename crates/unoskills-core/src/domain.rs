use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentCategory {
    #[default]
    Coding,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub key: String,
    pub display_name: String,
    #[serde(default)]
    pub category: AgentCategory,
    pub relative_skills_dir: String,
    pub relative_detect_dir: String,
    #[serde(default)]
    pub additional_scan_dirs: Vec<String>,
    #[serde(default)]
    pub project_relative_skills_dir: Option<String>,
    #[serde(default)]
    pub recursive_scan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub key: String,
    pub display_name: String,
    pub category: AgentCategory,
    pub skills_dir: PathBuf,
    pub detect_dir: PathBuf,
    pub additional_scan_dirs: Vec<PathBuf>,
    pub project_relative_skills_dir: String,
    pub installed: bool,
    pub enabled: bool,
    pub is_custom: bool,
    pub has_path_override: bool,
    pub recursive_scan: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeployMode {
    #[default]
    Symlink,
    Copy,
}

impl DeployMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Symlink => "symlink",
            Self::Copy => "copy",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "copy" => Self::Copy,
            _ => Self::Symlink,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    #[default]
    Local,
    Git,
    SkillsSh,
    Adopted,
}

impl SourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Git => "git",
            Self::SkillsSh => "skillssh",
            Self::Adopted => "adopted",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "git" => Self::Git,
            "skillssh" => Self::SkillsSh,
            "adopted" => Self::Adopted,
            _ => Self::Local,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: SourceType,
    pub source_ref: Option<String>,
    pub source_subpath: Option<String>,
    pub source_branch: Option<String>,
    pub central_path: PathBuf,
    pub content_hash: Option<String>,
    pub estimated_tokens: u32,
    pub tags: Vec<String>,
    pub deployments: Vec<Deployment>,
    #[serde(default)]
    pub project_deployments: Vec<ProjectDeployment>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Skill {
    pub fn is_deployed_to(&self, agent_key: &str) -> bool {
        self.deployments.iter().any(|d| d.agent_key == agent_key)
    }

    pub fn is_deployed_to_project(&self, project_id: &str, agent_key: &str) -> bool {
        self.project_deployments
            .iter()
            .any(|d| d.project_id == project_id && d.agent_key == agent_key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    pub id: String,
    pub skill_id: String,
    pub agent_key: String,
    pub target_path: PathBuf,
    pub mode: DeployMode,
    pub synced_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWorkspace {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDeployment {
    pub id: String,
    pub skill_id: String,
    pub project_id: String,
    pub agent_key: String,
    pub target_path: PathBuf,
    pub mode: DeployMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_hash: Option<String>,
    pub synced_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLintReport {
    pub valid: bool,
    pub entry_file: Option<String>,
    pub estimated_tokens: u32,
    pub file_count: usize,
    pub warnings: Vec<String>,
}

/// Provenance of a stack manifest. `Featured` is the serde default so all
/// pre-existing TOML manifests and local stacks keep parsing unchanged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackSource {
    /// Built-in stack shipped inside the app (`featured_stacks.toml`).
    #[default]
    Featured,
    /// Stack fetched from the remote community index (`index.json`).
    Community,
    /// Stack created locally by the user.
    Local,
}

/// Unified declarative manifest for a Skill Stack (used for local presets,
/// backup/export, Gist sharing, and the community Stacks registry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub recommended_agents: Vec<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub source: StackSource,
    #[serde(default)]
    pub skills: Vec<StackSkillRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackSkillRef {
    pub name: String,
    /// Canonical install ref: `owner/repo@skill`, git URL, or local path.
    pub source: String,
    #[serde(default)]
    pub note: String,
}

impl StackSkillRef {
    pub fn matches_source(&self, reference: &str) -> bool {
        let source = self.source.trim();
        !source.is_empty()
            && (source == reference
                || source.strip_prefix("~/").is_some_and(|path| {
                    dirs::home_dir().is_some_and(|home| home.join(path).as_os_str() == reference)
                }))
    }

    /// Match the installed source first, then fall back to the manifest's skill name.
    pub fn installed_in<'a>(&self, skills: &'a [Skill]) -> Option<&'a Skill> {
        let name = self.name.trim().to_lowercase();
        skills
            .iter()
            .find(|skill| {
                skill
                    .source_ref
                    .as_deref()
                    .is_some_and(|source| self.matches_source(source))
            })
            .or_else(|| {
                skills.iter().find(|skill| {
                    skill.id.to_lowercase() == name
                        || skill.slug.to_lowercase() == name
                        || skill.name.to_lowercase() == name
                })
            })
    }

    pub fn has_source_conflict(&self, skill: &Skill) -> bool {
        skill.source_ref.as_ref().is_some_and(|source| {
            !source.eq_ignore_ascii_case(self.source.trim()) && !self.matches_source(source)
        })
    }

    pub fn deployable_in<'a>(&self, skills: &'a [Skill]) -> Option<&'a Skill> {
        self.installed_in(skills)
            .filter(|skill| !self.has_source_conflict(skill))
    }
}

#[derive(Debug, Default)]
pub struct StackProjectDeployStatus {
    pub deployed: usize,
    pub pending: usize,
    pub unavailable: usize,
    pub conflicts: usize,
}

#[derive(Debug, Default)]
pub struct StackProjectDeploySummary {
    pub deployed: usize,
    pub already_deployed: usize,
    pub unavailable: usize,
    pub failed: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackInstallSummary {
    pub stack_id: String,
    pub installed: Vec<String>,
    pub skipped_existing: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub deployed_agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceSkill {
    pub id: String,
    pub skill_id: String,
    pub name: String,
    pub source: String,
    pub installs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub is_official: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarketplacePage {
    pub items: Vec<MarketplaceSkill>,
    pub page: usize,
    pub total: Option<usize>,
    pub has_more: bool,
}

impl MarketplaceSkill {
    pub fn install_ref(&self) -> String {
        if self.source.is_empty() {
            self.skill_id.clone()
        } else {
            format!("{}@{}", self.source, self.skill_id)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdoptCandidate {
    pub agent_key: String,
    pub agent_name: String,
    pub name: String,
    pub description: Option<String>,
    pub path: PathBuf,
    pub estimated_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillUpdateResult {
    pub skill: Skill,
    pub updated: bool,
    pub refreshed_copies: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBackupStatus {
    pub repo_url: Option<String>,
    pub branch: String,
    pub last_pushed_at: Option<i64>,
    pub last_pulled_at: Option<i64>,
    pub local_skill_count: usize,
    pub local_stack_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBackupPushResult {
    pub committed: bool,
    pub pushed: bool,
    pub skills_exported: usize,
    pub stacks_exported: usize,
    pub commit_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBackupPullResult {
    pub skills_imported: usize,
    pub skills_updated: usize,
    pub skills_unchanged: usize,
    pub stacks_imported: usize,
    pub refreshed_copies: usize,
}
