import { HostError, call, errorMessage } from "../shared";
import type {
  AdoptBatchResponse,
  AdoptCandidate,
  AdoptionMergeRequest,
  AdoptionMergeResponse,
  DeployMode,
  Deployment,
  LeaderboardResponse,
  LibraryStateResponse,
  MarketBoard,
  ProjectDeployment,
  ProjectStatusesResponse,
  ProjectWorkspace,
  ReviewedDeployment,
  SaveMarkdownResponse,
  ScanAdoptableResponse,
  SearchResponse,
  Skill,
  SkillLintReport,
  SkillMarkdownResponse,
  SkillRevisionPreview,
  SkillUpdateResult,
} from "./types";

export { errorMessage };

export function isTargetConflictError(error: unknown): boolean {
  if (error instanceof HostError && error.code === "TARGET_CONFLICT") {
    return true;
  }
  const withCode = error as { code?: string } | null | undefined;
  if (withCode?.code === "TARGET_CONFLICT") {
    return true;
  }
  return errorMessage(error).includes("TARGET_CONFLICT");
}

export function pickFolder(title?: string): Promise<string | null> {
  return call<{ title?: string }, string | null>("dialog.folder", title ? { title } : {});
}

export function revealPath(path: string): Promise<void> {
  return call<{ path: string }, void>("dialog.reveal", { path });
}

export function loadLibraryState(
  projectId?: string | null,
  scopePath?: string,
): Promise<LibraryStateResponse> {
  return call<{ project_id?: string | null; scope_path?: string }, LibraryStateResponse>(
    "library.state",
    {
      project_id: projectId ?? null,
      scope_path: projectId ? (scopePath ?? "") : "",
    },
  );
}

export function loadProjectStatuses(
  projectId: string,
  scopePath = "",
): Promise<ProjectStatusesResponse> {
  return call<{ project_id: string; scope_path: string }, ProjectStatusesResponse>(
    "library.project_statuses",
    {
      project_id: projectId,
      scope_path: scopePath,
    },
  );
}

export function getSkillMarkdown(skillId: string): Promise<SkillMarkdownResponse> {
  return call<{ skill_id: string }, SkillMarkdownResponse>("library.get_markdown", {
    skill_id: skillId,
  });
}

export function saveSkillMarkdown(skillId: string, content: string): Promise<SaveMarkdownResponse> {
  return call<{ skill_id: string; content: string }, SaveMarkdownResponse>("library.save_markdown", {
    skill_id: skillId,
    content,
  });
}

export function lintSkill(skillId: string): Promise<SkillLintReport> {
  return call<{ skill_id: string }, SkillLintReport>("library.lint_skill", {
    skill_id: skillId,
  });
}

export function prepareSkillRevision(skillId: string, rollback: boolean): Promise<SkillRevisionPreview | null> {
  return call<{ skill_id: string }, SkillRevisionPreview | null>(
    rollback ? "library.prepare_skill_rollback" : "library.prepare_skill_update", { skill_id: skillId },
  );
}

export function applySkillRevision(previewId: string): Promise<SkillUpdateResult> {
  return call<{ preview_id: string }, SkillUpdateResult>("library.apply_skill_revision", { preview_id: previewId });
}

export function cancelSkillRevision(previewId: string): Promise<void> {
  return call<{ preview_id: string }, void>("library.cancel_skill_revision", { preview_id: previewId });
}

export function removeSkill(skillId: string, deployments: ReviewedDeployment[]): Promise<{ removed: boolean; skill_id: string }> {
  return call<{ skill_id: string; deployments: ReviewedDeployment[] }, { removed: boolean; skill_id: string }>("library.remove_skill", {
    skill_id: skillId,
    deployments,
  });
}

export function deploySkill(
  skillId: string,
  agentKey: string,
  options?: { mode?: DeployMode | null; force?: boolean },
): Promise<Deployment> {
  return call<
    { skill_id: string; agent_key: string; mode?: DeployMode | null; force?: boolean },
    Deployment
  >("library.deploy_skill", {
    skill_id: skillId,
    agent_key: agentKey,
    mode: options?.mode ?? null,
    force: options?.force ?? false,
  });
}

export function undeploySkill(
  skillId: string,
  agentKey: string,
): Promise<{ undeployed: boolean; skill_id: string; agent_key: string }> {
  return call<
    { skill_id: string; agent_key: string },
    { undeployed: boolean; skill_id: string; agent_key: string }
  >("library.undeploy_skill", {
    skill_id: skillId,
    agent_key: agentKey,
  });
}

export function deployProjectSkill(
  skillId: string,
  projectId: string,
  agentKey: string,
  options?: { scopePath?: string; mode?: DeployMode | null; force?: boolean },
): Promise<ProjectDeployment> {
  return call<
    {
      skill_id: string;
      project_id: string;
      scope_path: string;
      agent_key: string;
      mode?: DeployMode | null;
      force?: boolean;
    },
    ProjectDeployment
  >("library.deploy_project_skill", {
    skill_id: skillId,
    project_id: projectId,
    scope_path: options?.scopePath ?? "",
    agent_key: agentKey,
    mode: options?.mode ?? null,
    force: options?.force ?? false,
  });
}

export function undeployProjectSkill(
  skillId: string,
  projectId: string,
  agentKey: string,
  scopePath: string,
  deployment: ReviewedDeployment,
): Promise<{
  undeployed: boolean;
  skill_id: string;
  project_id: string;
  scope_path: string;
  agent_key: string;
}> {
  return call<
    { skill_id: string; project_id: string; scope_path: string; agent_key: string; deployment: ReviewedDeployment },
    {
      undeployed: boolean;
      skill_id: string;
      project_id: string;
      scope_path: string;
      agent_key: string;
    }
  >("library.undeploy_project_skill", {
    skill_id: skillId,
    project_id: projectId,
    scope_path: scopePath,
    agent_key: agentKey,
    deployment,
  });
}

export function attachProject(path: string, name?: string | null): Promise<ProjectWorkspace> {
  return call<{ path: string; name?: string | null }, ProjectWorkspace>("library.attach_project", {
    path,
    name: name ?? null,
  });
}

export function detachProject(projectId: string): Promise<{ detached: boolean; project_id: string }> {
  return call<{ project_id: string }, { detached: boolean; project_id: string }>(
    "library.detach_project",
    { project_id: projectId },
  );
}

export function installFromSource(sourceRef: string): Promise<Skill> {
  return call<{ source_ref: string }, Skill>("discover.install", { source_ref: sourceRef });
}

export function fetchLeaderboard(board: MarketBoard, page = 0): Promise<LeaderboardResponse> {
  return call<{ board: MarketBoard; page: number }, LeaderboardResponse>("discover.leaderboard", {
    board,
    page,
  });
}

export function searchMarketplace(query: string, limit = 60): Promise<SearchResponse> {
  return call<{ query: string; limit: number }, SearchResponse>("discover.search", {
    query,
    limit,
  });
}

export function scanAdoptableSkills(): Promise<ScanAdoptableResponse> {
  return call<Record<string, never>, ScanAdoptableResponse>("discover.scan_adoptable", {});
}

export function adoptLocalSkills(candidates: AdoptCandidate[]): Promise<AdoptBatchResponse> {
  return call<{ candidates: AdoptCandidate[] }, AdoptBatchResponse>("discover.adopt", {
    candidates,
  });
}

export function mergeLocalSkills(requests: AdoptionMergeRequest[]): Promise<AdoptionMergeResponse> {
  return call<{ requests: AdoptionMergeRequest[] }, AdoptionMergeResponse>("discover.merge_adoption", { requests });
}
