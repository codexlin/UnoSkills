import { signal } from "nsh";
import { AlertDialog } from "nsh/base";
import type { ScreenProps } from "../shared";
import { joinPath } from "./format";
import {
  deployProjectSkill, deploySkill, errorMessage, isTargetConflictError, undeployProjectSkill, undeploySkill,
} from "./host";
import {
  confirmConflictDesc, getStrings, toastDeployed, toastProjectDeployed, toastProjectUndeployed, toastUndeployed,
} from "./strings";
import type { AgentInfo, ProjectWorkspace, Skill } from "./types";

interface DeploymentTarget {
  label: string;
  path: string;
  install(force: boolean): Promise<string>;
  remove(): Promise<string>;
}

export function createDeployments(screen: Pick<ScreenProps, "locale" | "notify" | "refresh">, reload: () => Promise<void>) {
  const pending = signal(false);

  async function toggle(target: DeploymentTarget, deployed: boolean) {
    if (pending()) return;
    pending.set(true);
    try {
      let message: string;
      try {
        message = await (deployed ? target.remove() : target.install(false));
      } catch (error) {
        if (deployed || !isTargetConflictError(error)) throw error;
        const t = getStrings(screen.locale);
        const { action } = await AlertDialog.show({
          title: t.confirm_conflict_title,
          description: confirmConflictDesc(screen.locale, target.label, target.path),
          actions: [
            { id: "cancel", label: t.btn_cancel, role: "cancel" },
            { id: "replace", label: t.btn_replace, role: "destructive" },
          ],
        });
        if (action !== "replace") return;
        message = await target.install(true);
      }
      screen.notify(message, "success");
      screen.refresh();
      await reload();
    } catch (error) {
      screen.notify(errorMessage(error), "error");
    } finally {
      pending.set(false);
    }
  }

  function global(skill: Skill, agent: AgentInfo, deployed: boolean) {
    return toggle({
      label: agent.display_name,
      path: joinPath(agent.skills_dir, skill.slug),
      async install(force) {
        const result = await deploySkill(skill.id, agent.key, { force });
        return toastDeployed(screen.locale, result.agent_key, result.mode);
      },
      async remove() {
        await undeploySkill(skill.id, agent.key);
        return toastUndeployed(screen.locale, agent.key);
      },
    }, deployed);
  }

  function project(skill: Skill, workspace: ProjectWorkspace, scopePath: string, agent: AgentInfo, deployed: boolean) {
    const label = scopePath ? `${workspace.name} · ${scopePath}` : workspace.name;
    const folder = scopePath ? joinPath(workspace.path, scopePath) : workspace.path;
    const relativeDir = agent.project_relative_skills_dir.trim() || ".agents/skills";
    return toggle({
      label: agent.display_name,
      path: joinPath(folder, relativeDir, skill.slug),
      async install(force) {
        const result = await deployProjectSkill(skill.id, workspace.id, agent.key, { scopePath, force });
        return toastProjectDeployed(screen.locale, label, result.agent_key, result.mode);
      },
      async remove() {
        await undeployProjectSkill(skill.id, workspace.id, agent.key, scopePath);
        return toastProjectUndeployed(screen.locale, label, agent.key);
      },
    }, deployed);
  }

  return { pending, global, project };
}
