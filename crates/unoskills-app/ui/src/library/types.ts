export type DeployMode = "symlink" | "copy";
export type SourceType = "local" | "git" | "skillssh" | "adopted";
export type AgentCategory = "coding" | "assistant";
export type MarketBoard = "trending" | "hot" | "all-time";
export type MarketInstallStatus = "available" | "installed" | "same_name";
export type ProjectSkillPlacement = "missing" | "managed" | "local" | "conflict" | "unavailable";

export interface Deployment {
  id: string;
  skill_id: string;
  agent_key: string;
  target_path: string;
  mode: DeployMode;
  synced_at: number;
}

export interface ProjectDeployment {
  id: string;
  skill_id: string;
  project_id: string;
  agent_key: string;
  scope_path?: string;
  target_path: string;
  mode: DeployMode;
  copy_hash?: string | null;
  synced_at: number;
}

export interface Skill {
  id: string;
  slug: string;
  name: string;
  description?: string | null;
  source_type: SourceType;
  source_ref?: string | null;
  source_subpath?: string | null;
  source_branch?: string | null;
  central_path: string;
  content_hash?: string | null;
  estimated_tokens: number;
  tags: string[];
  deployments: Deployment[];
  project_deployments: ProjectDeployment[];
  created_at: number;
  updated_at: number;
}

export interface AgentInfo {
  key: string;
  display_name: string;
  category: AgentCategory;
  skills_dir: string;
  detect_dir: string;
  additional_scan_dirs: string[];
  project_relative_skills_dir: string;
  installed: boolean;
  enabled: boolean;
  is_custom: boolean;
  has_path_override: boolean;
  recursive_scan: boolean;
}

export interface ProjectWorkspace {
  id: string;
  name: string;
  path: string;
  created_at: number;
}

export interface ProjectSkillStatus {
  placement: ProjectSkillPlacement;
  global_available: boolean;
  retained_pin?: boolean;
}

export type ReviewedDeployment = { id: string; target_path: string };

export type ProjectSkillStatusMap = Record<string, Record<string, ProjectSkillStatus>>;

export interface SkillLintReport {
  valid: boolean;
  entry_file?: string | null;
  estimated_tokens: number;
  file_count: number;
  warnings: string[];
}

export interface SkillUpdateResult {
  skill: Skill;
  updated: boolean;
  refreshed_copies: number;
  deployment_issues?: DeploymentImpact[];
}

export interface DeploymentImpact {
  path: string;
  agents: string[];
  kind: "linked" | "current" | "refresh_copy" | "missing" | "conflict" | "refresh_failed";
}

export interface SkillRevisionPreview {
  id: string;
  skill_id: string;
  skill_name: string;
  rollback: boolean;
  files: { path: string; kind: "added" | "modified" | "removed" }[];
  deployments: DeploymentImpact[];
}

export interface LibraryStateResponse {
  skills: Skill[];
  rollback_skill_ids?: string[];
  agents: AgentInfo[];
  projects: ProjectWorkspace[];
  deploy_mode: DeployMode;
  scope_path?: string;
  project_statuses: ProjectSkillStatusMap | null;
  project_status_error: string | null;
}

export interface ProjectStatusesResponse {
  project_id: string;
  scope_path?: string;
  statuses: ProjectSkillStatusMap;
}

export interface SkillMarkdownResponse {
  skill_id: string;
  entry_file: string;
  raw_markdown: string;
  preview_markdown: string;
  lint: SkillLintReport | null;
}

export interface SaveMarkdownResponse {
  skill: Skill;
  entry_file: string;
  raw_markdown: string;
  preview_markdown: string;
  lint: SkillLintReport | null;
}

export interface MarketplaceSkillItem {
  id: string;
  skill_id: string;
  name: string;
  source: string;
  installs: number;
  description?: string | null;
  is_official: boolean;
  install_ref: string;
  detail_url?: string | null;
}

export interface LeaderboardResponse {
  board: MarketBoard;
  page: number;
  total: number | null;
  has_more: boolean;
  items: MarketplaceSkillItem[];
}

export interface SearchResponse {
  query: string;
  items: MarketplaceSkillItem[];
}

export interface AdoptCandidate {
  agent_key: string;
  agent_name: string;
  name: string;
  description?: string | null;
  path: string;
  estimated_tokens: number;
}

export interface ScanAdoptableResponse {
  candidates: AdoptCandidate[];
}

export interface AdoptFailure {
  name: string;
  path: string;
  error: string;
}

export interface AdoptBatchResponse {
  adopted: Skill[];
  adopted_count: number;
  failed: AdoptFailure[];
}
