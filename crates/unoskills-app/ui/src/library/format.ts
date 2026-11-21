import type {
  MarketInstallStatus,
  MarketplaceSkillItem,
  ProjectSkillStatus,
  Skill,
  SourceType,
} from "./types";

export const TOKEN_BUDGET_WARN_THRESHOLD = 4000;

export function formatTokensShort(count: number): string {
  if (count >= 1000) {
    return `~${(count / 1000).toFixed(1)}k`;
  }
  return `~${count}`;
}

export function formatInstallCount(n: number): string {
  if (n >= 1_000_000) {
    return `${(n / 1_000_000).toFixed(1)}M`;
  }
  if (n >= 1_000) {
    return `${(n / 1_000).toFixed(1)}k`;
  }
  return String(n);
}

export function compactSource(source: string, sourceType: SourceType): string {
  let repository: string | null = null;
  if (sourceType === "skillssh") {
    const atIndex = source.indexOf("@");
    repository = atIndex >= 0 ? source.slice(0, atIndex) : source;
  } else if (sourceType === "git") {
    const prefixes = ["https://github.com/", "http://github.com/", "git@github.com:"];
    for (const prefix of prefixes) {
      if (source.startsWith(prefix)) {
        repository = source.slice(prefix.length);
        break;
      }
    }
  } else {
    const parts = source.replace(/\\/g, "/").split("/").filter(Boolean);
    if (parts.length >= 3) {
      const container = parts[parts.length - 3];
      const folder = parts[parts.length - 2];
      return `…/${container}/${folder}`;
    }
  }

  if (repository) {
    const slashIdx = repository.indexOf("/");
    if (slashIdx > 0) {
      const owner = repository.slice(0, slashIdx);
      const rest = repository.slice(slashIdx + 1);
      const firstSegment = rest.split("/")[0];
      if (firstSegment) {
        const repo = firstSegment.endsWith(".git")
          ? firstSegment.slice(0, -4)
          : firstSegment;
        return `${owner}/${repo}`;
      }
    }
  }

  return source;
}

export function isSkillDeployedTo(skill: Skill, agentKey: string): boolean {
  return skill.deployments.some((dep) => dep.agent_key === agentKey);
}

export function isProjectSkillAvailable(status: ProjectSkillStatus | undefined): boolean | null {
  if (!status) {
    return null;
  }
  switch (status.placement) {
    case "managed":
    case "local":
      return true;
    case "missing":
      return status.global_available;
    case "conflict":
    case "unavailable":
      return false;
  }
}

export function marketInstallStatus(
  item: MarketplaceSkillItem,
  installedSourceRefs: ReadonlySet<string>,
  installedSkillNames: ReadonlySet<string>,
): MarketInstallStatus {
  if (installedSourceRefs.has(item.install_ref.toLowerCase())) {
    return "installed";
  }
  if (
    installedSkillNames.has(item.skill_id.toLowerCase()) ||
    installedSkillNames.has(item.name.toLowerCase())
  ) {
    return "same_name";
  }
  return "available";
}

export function joinPath(base: string, ...segments: string[]): string {
  const sep = base.includes("\\") ? "\\" : "/";
  const trimmedBase = base.replace(/[\\/]+$/, "");
  const cleanSegments = segments.map((segment) => segment.replace(/^[\\/]+|[\\/]+$/g, "")).filter(Boolean);
  return [trimmedBase, ...cleanSegments].join(sep);
}
