import type {
  MarketInstallStatus,
  MarketplaceSkillItem,
  ProjectSkillStatus,
  Skill,
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

export function compactLibraryPath(path: string): string {
  const windows = !path.startsWith("/") && path.includes("\\");
  const separator = windows ? "\\" : "/";
  const parts = path.split(windows ? /[\\/]/ : "/").filter(Boolean);
  return parts.length > 3 ? ["…", ...parts.slice(-3)].join(separator) : path;
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
