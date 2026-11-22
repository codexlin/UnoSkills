import { call } from "../shared";

export type Project = { id: string; name: string; path: string };
export type Agent = { key: string; display_name: string; project_relative_skills_dir: string };
export type ProjectTargets = { projects: Project[]; agents: Agent[] };
export type ProjectScope = { project_id: string; scope_path: string; agent_key: string };
export type StackSource = "Featured" | "Community" | "Local";
export type StackRef = { stack_id: string; source: StackSource };
export type StackMember = { name: string; source: string; note: string };
export type Stack = {
  id: string;
  name: string;
  author: string;
  description: string;
  tags: string[];
  recommended_agents: string[];
  verified: boolean;
  source: StackSource;
  skills: StackMember[];
};
export type LibrarySkill = { id: string; name: string; slug: string; description: string | null };
export type MemberStatus = { member: StackMember; installed: LibrarySkill | null; source_conflict: boolean };
export type RecommendationPurpose = "react_performance" | "accessibility" | "testing" | "debugging";
export type Recommendation = {
  member: StackMember;
  purpose: RecommendationPurpose;
  technology: string;
  installed_slug: string | null;
};
export type Module = {
  path: string;
  evidence: { technology: string; path: string; added: boolean }[];
  recommendations: Recommendation[];
};
export type Location = "project_deployment" | "project_changed" | "project_local" | "global";
export type InventoryRow = {
  name: string; path: string; location: Location;
  skill_id?: string | null; locked_hash?: string | null; library_different?: boolean | null;
};
export type VersionTarget = ProjectScope & { skill_id: string };
export type VersionIntent = "upgrade" | "follow";
export type VersionReview = {
  id: string; skill_id: string; skill_name: string; intent: VersionIntent;
  current_hash: string; candidate_hash: string;
  files: { path: string; kind: "added" | "modified" | "removed"; diff: string | null }[];
};
export type Failure = [name: string, message: string];
export type InstallResult = { installed: string[]; skipped_existing: string[]; failed: Failure[] };
export type DeployOutcome = "keep_local" | "occupied" | "unsafe_path" | "other_source" |
  "duplicate" | "already_present" | "register" | "ready" | "global_only" | "unavailable";
export type DeployRow = { index: number; name: string; target: string | null; outcome: DeployOutcome };
export type DeployStatus = {
  deployed: number;
  global_available: number;
  project_local: number;
  pending: number;
  unavailable: number;
  conflicts: number;
  rows: DeployRow[];
};
export type DeployResult = Omit<DeployStatus, "pending" | "conflicts" | "rows"> & {
  already_deployed: number;
  registered: number;
  failed: Failure[];
  skipped: Failure[];
};
export type StackFormData = { name: string; description: string; author: string; tags: string[]; skill_ids: string[] };

type Commands = {
  "dialog.folder": [{ title: string }, string | null];
  "dialog.open": [{ title: string; extensions: string[] }, string | null];
  "dialog.save": [{ title: string; name: string; extensions: string[] }, string | null];
  "project.list": [null, ProjectTargets];
  "project.attach": [{ path: string }, Project];
  "project.detach": [{ project_id: string }, null];
  "project.scan": [{ project_id: string }, Module[]];
  "project.scopes": [{ project_id: string }, string[]];
  "project.inventory": [ProjectScope, InventoryRow[]];
  "project.lock_skill": [VersionTarget, null];
  "project.prepare_version": [VersionTarget & { intent: VersionIntent }, VersionReview];
  "project.apply_version": [{ preview_id: string }, null];
  "project.cancel_version": [{ preview_id: string }, null];
  "project.save_recommendations": [{ project_id: string; scope_path: string; members: StackMember[] }, Stack];
  "stack.list": [null, { featured: Stack[]; local: Stack[] }];
  "stack.community": [null, Stack[]];
  "stack.library": [null, LibrarySkill[]];
  "stack.members": [StackRef, MemberStatus[]];
  "stack.create": [StackFormData, Stack];
  "stack.edit": [StackFormData & { stack_id: string; previous: Stack; remove_indices: number[] }, Stack];
  "stack.delete": [{ stack_id: string }, null];
  "stack.read_toml": [{ path: string }, string];
  "stack.preview_import": [{ toml: string }, { stack: Stack; previous: string | null }];
  "stack.import": [{ toml: string; previous: string | null }, Stack];
  "stack.export": [StackRef & { path?: string }, string];
  "stack.share_gist": [StackRef, string];
  "stack.install": [StackRef, InstallResult];
  "stack.install_member": [StackRef & { member_source: string; member_name: string }, LibrarySkill];
  "stack.preview_deploy": [StackRef & ProjectScope, DeployStatus];
  "stack.confirm_deploy": [StackRef & ProjectScope & { include_global: boolean; reviewed: DeployStatus }, DeployResult];
};

export function request<K extends keyof Commands>(command: K, args: Commands[K][0]): Promise<Commands[K][1]> {
  return call<Commands[K][0], Commands[K][1]>(command, args);
}

export async function attachProjectFromPicker(title: string): Promise<Project | null> {
  const path = await request("dialog.folder", { title });
  return path ? request("project.attach", { path }) : null;
}

export function stackRef(stack: Stack): StackRef {
  return { stack_id: stack.id, source: stack.source };
}
