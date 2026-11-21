import { batch, computed, signal } from "nsh";
import { isProjectSkillAvailable, isSkillDeployedTo } from "./format";
import { errorMessage, loadProjectStatuses } from "./host";
import { getStrings, type Locale } from "./strings";
import type { LibraryStateResponse, ProjectSkillStatusMap, ProjectWorkspace, Skill } from "./types";

export function createProjectScope(locale: () => Locale) {
  const projects = signal<ProjectWorkspace[]>([]);
  const selection = signal({ projectId: null as string | null, path: "" });
  const statuses = signal<ProjectSkillStatusMap | null>(null);
  const loading = signal(false);
  const error = signal<string | null>(null);
  let request = 0;

  const projectId = () => selection().projectId;
  const path = () => selection().path;
  const project = computed(() => projects().find((entry) => entry.id === projectId()) ?? null);
  const label = () => {
    const active = project();
    if (!active) return getStrings(locale()).scope_global;
    return path() ? `${active.name} · ${path()}` : active.name;
  };
  const statusFor = (skillId: string, agentKey: string) => statuses()?.[agentKey]?.[skillId];
  const isAvailable = (skill: Skill, agentKey: string) => projectId()
    ? isProjectSkillAvailable(statusFor(skill.id, agentKey))
    : isSkillDeployedTo(skill, agentKey);

  function select(id: string | null, scopePath: string) {
    ++request;
    batch(() => {
      selection.set({ projectId: id, path: id ? scopePath : "" });
      statuses.set(null);
      error.set(null);
      loading.set(false);
    });
  }

  function begin(id: string | null) {
    const ticket = ++request;
    loading.set(Boolean(id));
    return ticket;
  }

  function apply(ticket: number, id: string | null, scopePath: string, data: LibraryStateResponse) {
    projects.set(data.projects);
    if (ticket !== request) return;
    const exists = id !== null && data.projects.some((entry) => entry.id === id);
    batch(() => {
      selection.set({ projectId: exists ? id : null, path: exists ? scopePath : "" });
      statuses.set(exists ? data.project_statuses : null);
      error.set(exists ? data.project_status_error : null);
      loading.set(false);
    });
  }

  function finish(ticket: number) {
    if (ticket === request) loading.set(false);
  }

  async function change(id: string | null, scopePath = "") {
    const ticket = batch(() => {
      select(id, scopePath);
      return begin(id);
    });
    if (!id) return;
    try {
      const data = await loadProjectStatuses(id, scopePath);
      if (ticket === request) statuses.set(data.statuses);
    } catch (reason) {
      if (ticket === request) error.set(errorMessage(reason));
    } finally {
      finish(ticket);
    }
  }

  return { projects, projectId, project, path, label, statuses, loading, error, statusFor, isAvailable,
    select, begin, apply, finish, change };
}
