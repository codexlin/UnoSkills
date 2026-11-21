import {
  type InputRef,
  Text,
  View,
  batch,
  cleanup,
  computed,
  effect,
  keymap,
  signal,
  windowSize,
} from "nsh";
import {
  AlertDialog,
  Menu,
  MenuItem,
  MenuSeparator,
  ResizablePanel,
  ResizablePanelGroup,
  Select,
} from "nsh/base";
import { Label } from "../controls";
import type { ScreenProps } from "../shared";
import { LibraryListPane } from "./LibraryListPane";
import { SkillInspector } from "./SkillInspector";
import { ActionButton, AlertBanner, PageHeader, SearchField, hintTooltip } from "./controls";
import { isProjectSkillAvailable, isSkillDeployedTo, joinPath } from "./format";
import {
  attachProject,
  deployProjectSkill,
  deploySkill,
  detachProject,
  errorMessage,
  getSkillMarkdown,
  isTargetConflictError,
  loadLibraryState,
  loadProjectStatuses,
  pickFolder,
  removeSkill,
  saveSkillMarkdown,
  undeployProjectSkill,
  undeploySkill,
  updateSkill,
} from "./host";
import {
  busyUpdatingSkill,
  confirmConflictDesc,
  confirmDeleteSkillDesc,
  confirmDetachProjectDesc,
  getStrings,
  toastDeployed,
  toastProjectAttached,
  toastProjectDeployed,
  toastProjectUndeployed,
  toastSavedSkill,
  toastSkillUpToDate,
  toastSkillUpdated,
  toastUndeployed,
} from "./strings";
import type {
  AgentInfo,
  ProjectSkillStatus,
  ProjectSkillStatusMap,
  ProjectWorkspace,
  Skill,
  SkillLintReport,
} from "./types";

const ALL_AGENTS_VALUE = "__all_agents__";

export function Library(props: ScreenProps) {
  const skills = signal<Skill[]>([]);
  const agents = signal<AgentInfo[]>([]);
  const projects = signal<ProjectWorkspace[]>([]);
  const selectedSkillId = signal<string | null>(null);
  const selectedProjectId = signal<string | null>(null);
  const selectedScopePath = signal<string>("");
  const agentFilter = signal<string | null>(null);
  const searchQuery = signal("");

  const loadingState = signal(true);
  const loadError = signal<string | null>(null);
  const projectStatuses = signal<ProjectSkillStatusMap | null>(null);
  const projectStatusLoading = signal(false);
  const projectStatusError = signal<string | null>(null);

  const inspectorTab = signal<"deployments" | "preview">("deployments");
  const loadedMarkdownSkillId = signal<string | null>(null);
  const editingSkillId = signal<string | null>(null);
  const loadedMarkdown = signal("");
  const draftMarkdown = signal("");
  const previewMarkdown = signal("");
  const markdownLoading = signal(false);
  const markdownError = signal<string | null>(null);
  const savingSkillId = signal<string | null>(null);
  const selectedLint = signal<SkillLintReport | null>(null);

  const updatingSkillId = signal<string | null>(null);
  const deployingKey = signal<string | null>(null);
  const busyLabel = signal<string | null>(null);
  const listPaneWidth = signal(408);

  let searchInputRef: InputRef | undefined;
  let stateReqSeq = 0;
  let projectReqSeq = 0;
  let markdownReqSeq = 0;
  let lastRouteSignature = "";
  let wasActive = false;

  const s = () => getStrings(props.locale);
  const isActive = () =>
    (props as ScreenProps & { active?: boolean }).active ?? props.route.page === "library";

  const activeAgents = computed(() =>
    agents().filter((agent) => agent.installed && agent.enabled),
  );

  const activeProject = computed(() => {
    const id = selectedProjectId();
    return id ? (projects().find((project) => project.id === id) ?? null) : null;
  });

  const selectedSkill = computed(() => {
    const id = selectedSkillId();
    return id ? (skills().find((skill) => skill.id === id) ?? null) : null;
  });

  const isEditingCurrent = () =>
    editingSkillId() !== null && editingSkillId() === selectedSkillId();

  const canEditCurrent = () =>
    !markdownLoading() &&
    savingSkillId() === null &&
    selectedSkillId() !== null &&
    loadedMarkdownSkillId() === selectedSkillId() &&
    markdownError() === null;

  const isDirty = () =>
    editingSkillId() !== null &&
    loadedMarkdownSkillId() === editingSkillId() &&
    draftMarkdown() !== loadedMarkdown();

  const projectStatusFor = (skillId: string, agentKey: string): ProjectSkillStatus | undefined =>
    projectStatuses()?.[agentKey]?.[skillId];

  const isAvailableForAgent = (skill: Skill, agentKey: string): boolean | null => {
    if (selectedProjectId()) {
      return isProjectSkillAvailable(projectStatusFor(skill.id, agentKey));
    }
    return isSkillDeployedTo(skill, agentKey);
  };

  const filteredSkills = computed(() => {
    const query = searchQuery().trim().toLowerCase();
    const filterAgent = agentFilter();
    return skills().filter((skill) => {
      const matchesSearch =
        !query ||
        skill.name.toLowerCase().includes(query) ||
        skill.slug.toLowerCase().includes(query) ||
        (skill.description ? skill.description.toLowerCase().includes(query) : false) ||
        skill.tags.some((tag) => tag.toLowerCase().includes(query));
      if (!matchesSearch) {
        return false;
      }
      return filterAgent ? isAvailableForAgent(skill, filterAgent) === true : true;
    });
  });

  const agentSelectItems = computed(() => [
    { value: ALL_AGENTS_VALUE, title: s().library_all_agents },
    ...activeAgents().map((agent) => ({
      value: agent.key,
      title: agent.display_name,
    })),
  ]);

  const scopeLabel = () => {
    const project = activeProject();
    if (!project) {
      return s().scope_global;
    }
    const scope = selectedScopePath();
    return scope ? `${project.name} · ${scope}` : project.name;
  };

  const selectedAgentLabel = () => {
    const key = agentFilter();
    if (!key) {
      return s().library_all_agents;
    }
    return activeAgents().find((agent) => agent.key === key)?.display_name ?? s().library_all_agents;
  };

  const clearMarkdownState = () => {
    batch(() => {
      loadedMarkdownSkillId.set(null);
      editingSkillId.set(null);
      loadedMarkdown.set("");
      draftMarkdown.set("");
      previewMarkdown.set("");
      selectedLint.set(null);
      markdownError.set(null);
    });
  };

  const confirmDiscardIfDirty = async (): Promise<boolean> => {
    if (!isDirty()) {
      return true;
    }
    const { action } = await AlertDialog.show({
      title: s().library_discard_title,
      actions: [
        { id: "cancel", label: s().btn_cancel, role: "cancel" },
        { id: "discard", label: s().library_discard_action, role: "destructive" },
      ],
    });
    return action === "discard";
  };

  const loadSelectedMarkdown = async (skillId: string | null, preserveEditor: boolean) => {
    const req = ++markdownReqSeq;
    if (!skillId) {
      markdownLoading.set(false);
      clearMarkdownState();
      return;
    }

    const keepingSameDraft =
      preserveEditor &&
      loadedMarkdownSkillId() === skillId &&
      editingSkillId() === skillId;

    if (!keepingSameDraft) {
      clearMarkdownState();
    } else {
      markdownError.set(null);
    }
    markdownLoading.set(true);

    try {
      const data = await getSkillMarkdown(skillId);
      if (req !== markdownReqSeq || selectedSkillId() !== skillId) {
        return;
      }
      batch(() => {
        loadedMarkdownSkillId.set(skillId);
        previewMarkdown.set(data.preview_markdown);
        selectedLint.set(data.lint);
        markdownError.set(null);
        if (!keepingSameDraft) {
          loadedMarkdown.set(data.raw_markdown);
          draftMarkdown.set(data.raw_markdown);
        }
      });
    } catch (error) {
      if (req !== markdownReqSeq || selectedSkillId() !== skillId) {
        return;
      }
      const message = errorMessage(error);
      batch(() => {
        previewMarkdown.set("");
        selectedLint.set(null);
        markdownError.set(message);
        if (!keepingSameDraft) {
          loadedMarkdownSkillId.set(null);
          editingSkillId.set(null);
          loadedMarkdown.set("");
          draftMarkdown.set("");
        }
      });
      props.notify(message, "error");
    } finally {
      if (req === markdownReqSeq) {
        markdownLoading.set(false);
      }
    }
  };

  const refreshProjectStatusesOnly = async (projectId: string | null, scopePath: string) => {
    const req = ++projectReqSeq;
    if (!projectId) {
      batch(() => {
        projectStatuses.set(null);
        projectStatusLoading.set(false);
        projectStatusError.set(null);
      });
      return;
    }

    batch(() => {
      projectStatuses.set(null);
      projectStatusLoading.set(true);
      projectStatusError.set(null);
    });
    try {
      const data = await loadProjectStatuses(projectId, scopePath);
      if (req !== projectReqSeq) {
        return;
      }
      batch(() => {
        projectStatuses.set(data.statuses);
        projectStatusError.set(null);
      });
    } catch (error) {
      if (req !== projectReqSeq) {
        return;
      }
      batch(() => {
        projectStatuses.set(null);
        projectStatusError.set(errorMessage(error));
      });
    } finally {
      if (req === projectReqSeq) {
        projectStatusLoading.set(false);
      }
    }
  };

  const fetchState = async (options?: {
    preferredSkillId?: string | null;
    projectIdOverride?: string | null;
    scopePathOverride?: string;
    notifyRefreshed?: boolean;
    confirmRouteSkillSwitch?: boolean;
  }) => {
    const req = ++stateReqSeq;
    const requestedProject =
      options?.projectIdOverride !== undefined ? options.projectIdOverride : selectedProjectId();
    const requestedScope =
      options?.scopePathOverride !== undefined
        ? options.scopePathOverride
        : requestedProject
          ? selectedScopePath()
          : "";

    loadingState.set(true);
    loadError.set(null);
    if (requestedProject) {
      projectStatusLoading.set(true);
    }

    try {
      const data = await loadLibraryState(requestedProject, requestedScope);
      if (req !== stateReqSeq) {
        return;
      }

      const prevSelected = selectedSkillId();
      let desiredSkillId = options?.preferredSkillId ?? prevSelected;

      // Guard route-driven skill changes against silently discarding unsaved edits.
      if (
        options?.confirmRouteSkillSwitch &&
        desiredSkillId &&
        desiredSkillId !== prevSelected &&
        data.skills.some((skill) => skill.id === desiredSkillId) &&
        isDirty()
      ) {
        const allowDiscard = await confirmDiscardIfDirty();
        if (req !== stateReqSeq) {
          return;
        }
        if (!allowDiscard) {
          desiredSkillId = prevSelected;
        }
      }

      const nextSkillId =
        desiredSkillId && data.skills.some((skill) => skill.id === desiredSkillId)
          ? desiredSkillId
          : (data.skills[0]?.id ?? null);

      const validProjectId =
        requestedProject && data.projects.some((project) => project.id === requestedProject)
          ? requestedProject
          : null;
      const validScopePath = validProjectId ? requestedScope : "";

      const currentAgentFilter = agentFilter();
      const validAgentFilter =
        currentAgentFilter &&
        data.agents.some(
          (agent) => agent.key === currentAgentFilter && agent.installed && agent.enabled,
        )
          ? currentAgentFilter
          : null;

      const preserveEditor =
        nextSkillId !== null &&
        prevSelected === nextSkillId &&
        editingSkillId() === nextSkillId &&
        loadedMarkdownSkillId() === nextSkillId;

      batch(() => {
        skills.set(data.skills);
        agents.set(data.agents);
        projects.set(data.projects);
        selectedSkillId.set(nextSkillId);
        selectedProjectId.set(validProjectId);
        selectedScopePath.set(validScopePath);
        agentFilter.set(validAgentFilter);
        projectStatuses.set(validProjectId ? data.project_statuses : null);
        projectStatusError.set(validProjectId ? data.project_status_error : null);
        projectStatusLoading.set(false);
        if (!preserveEditor) {
          editingSkillId.set(null);
        }
      });

      await loadSelectedMarkdown(nextSkillId, preserveEditor);

      if (options?.notifyRefreshed) {
        props.notify(s().toast_refreshed, "success");
      }
    } catch (error) {
      if (req !== stateReqSeq) {
        return;
      }
      const message = errorMessage(error);
      loadError.set(message);
      projectStatusLoading.set(false);
      props.notify(message, "error");
    } finally {
      if (req === stateReqSeq) {
        loadingState.set(false);
      }
    }
  };

  const selectSkill = async (skillId: string) => {
    if (selectedSkillId() === skillId) {
      return;
    }
    if (!(await confirmDiscardIfDirty())) {
      return;
    }
    selectedSkillId.set(skillId);
    await loadSelectedMarkdown(skillId, false);
  };

  const changeScope = async (projectId: string | null, scopePath = "") => {
    batch(() => {
      selectedProjectId.set(projectId);
      selectedScopePath.set(projectId ? scopePath : "");
    });
    await refreshProjectStatusesOnly(projectId, projectId ? scopePath : "");
  };

  const startEditMarkdown = () => {
    const skillId = selectedSkillId();
    if (!skillId || !canEditCurrent()) {
      return;
    }
    batch(() => {
      draftMarkdown.set(loadedMarkdown());
      editingSkillId.set(skillId);
    });
  };

  const saveMarkdown = async () => {
    const targetSkillId = editingSkillId();
    if (
      !targetSkillId ||
      savingSkillId() !== null ||
      loadedMarkdownSkillId() !== targetSkillId
    ) {
      return;
    }
    const contentToSave = draftMarkdown();
    savingSkillId.set(targetSkillId);
    try {
      const result = await saveSkillMarkdown(targetSkillId, contentToSave);
      batch(() => {
        skills.update((items) =>
          items.map((item) => (item.id === result.skill.id ? result.skill : item)),
        );
        if (selectedSkillId() === targetSkillId) {
          markdownReqSeq += 1;
          loadedMarkdownSkillId.set(targetSkillId);
          loadedMarkdown.set(result.raw_markdown);
          draftMarkdown.set(result.raw_markdown);
          previewMarkdown.set(result.preview_markdown);
          selectedLint.set(result.lint);
          markdownError.set(null);
          if (editingSkillId() === targetSkillId) {
            editingSkillId.set(null);
          }
        }
      });
      props.notify(
        toastSavedSkill(props.locale, result.skill.name, result.skill.estimated_tokens),
        "success",
      );
      props.refresh();
    } catch (error) {
      props.notify(errorMessage(error), "error");
    } finally {
      if (savingSkillId() === targetSkillId) {
        savingSkillId.set(null);
      }
    }
  };

  const requestCancelEdit = async () => {
    if (!isEditingCurrent()) {
      return;
    }
    if (!(await confirmDiscardIfDirty())) {
      return;
    }
    batch(() => {
      draftMarkdown.set(loadedMarkdown());
      editingSkillId.set(null);
    });
  };

  const handleUpdateSkill = async (skill: Skill) => {
    if (updatingSkillId()) {
      return;
    }
    if (editingSkillId() === skill.id && !(await confirmDiscardIfDirty())) {
      return;
    }
    batch(() => {
      updatingSkillId.set(skill.id);
      busyLabel.set(busyUpdatingSkill(props.locale, skill.name));
    });
    try {
      const outcome = await updateSkill(skill.id);
      if (outcome.updated) {
        if (editingSkillId() === outcome.skill.id) {
          editingSkillId.set(null);
        }
        props.notify(
          toastSkillUpdated(
            props.locale,
            outcome.skill.name,
            outcome.skill.estimated_tokens,
            outcome.refreshed_copies,
          ),
          "success",
        );
        props.refresh();
        await fetchState({ preferredSkillId: outcome.skill.id });
      } else {
        props.notify(toastSkillUpToDate(props.locale, outcome.skill.name), "info");
      }
    } catch (error) {
      props.notify(errorMessage(error), "error");
    } finally {
      batch(() => {
        updatingSkillId.set(null);
        busyLabel.set(null);
      });
    }
  };

  const handleDeleteSkill = async (skill: Skill) => {
    const deployCount = skill.deployments.length + skill.project_deployments.length;
    const { action } = await AlertDialog.show({
      title: s().confirm_delete_skill_title,
      description: confirmDeleteSkillDesc(props.locale, skill.name, deployCount),
      actions: [
        { id: "cancel", label: s().btn_cancel, role: "cancel" },
        { id: "delete", label: s().btn_delete, role: "destructive" },
      ],
    });
    if (action !== "delete") {
      return;
    }
    try {
      await removeSkill(skill.id);
      props.notify(s().toast_removed_skill, "success");
      props.refresh();
      await fetchState();
    } catch (error) {
      props.notify(errorMessage(error), "error");
    }
  };

  const handleAttachProject = async () => {
    try {
      const chosen = await pickFolder(s().prompt_pick_project_folder);
      if (!chosen) {
        return;
      }
      const project = await attachProject(chosen);
      props.notify(toastProjectAttached(props.locale, project.name), "success");
      props.refresh();
      await fetchState({ projectIdOverride: project.id, scopePathOverride: "" });
    } catch (error) {
      props.notify(errorMessage(error), "error");
    }
  };

  const handleDetachProject = async (project: ProjectWorkspace) => {
    const { action } = await AlertDialog.show({
      title: s().confirm_detach_project_title,
      description: confirmDetachProjectDesc(props.locale, project.name, project.path),
      actions: [
        { id: "cancel", label: s().btn_cancel, role: "cancel" },
        { id: "detach", label: s().btn_detach_project, role: "destructive" },
      ],
    });
    if (action !== "detach") {
      return;
    }
    try {
      await detachProject(project.id);
      props.notify(s().toast_project_detached, "success");
      props.refresh();
      await fetchState({ projectIdOverride: null, scopePathOverride: "" });
    } catch (error) {
      props.notify(errorMessage(error), "error");
    }
  };

  const handleGlobalDeployToggle = async (
    skill: Skill,
    agent: AgentInfo,
    currentlyDeployed: boolean,
  ) => {
    const key = `global:${skill.id}:${agent.key}`;
    if (deployingKey()) {
      return;
    }
    deployingKey.set(key);
    try {
      if (currentlyDeployed) {
        await undeploySkill(skill.id, agent.key);
        props.notify(toastUndeployed(props.locale, agent.key), "success");
      } else {
        const dep = await deploySkill(skill.id, agent.key, { force: false });
        props.notify(toastDeployed(props.locale, dep.agent_key, dep.mode), "success");
      }
      props.refresh();
      await fetchState({ preferredSkillId: skill.id });
    } catch (error) {
      if (!currentlyDeployed && isTargetConflictError(error)) {
        const targetPath = joinPath(agent.skills_dir, skill.slug);
        const { action } = await AlertDialog.show({
          title: s().confirm_conflict_title,
          description: confirmConflictDesc(props.locale, agent.display_name, targetPath),
          actions: [
            { id: "cancel", label: s().btn_cancel, role: "cancel" },
            { id: "replace", label: s().btn_replace, role: "destructive" },
          ],
        });
        if (action === "replace") {
          try {
            const dep = await deploySkill(skill.id, agent.key, { force: true });
            props.notify(toastDeployed(props.locale, dep.agent_key, dep.mode), "success");
            props.refresh();
            await fetchState({ preferredSkillId: skill.id });
          } catch (forceError) {
            props.notify(errorMessage(forceError), "error");
          }
        }
      } else {
        props.notify(errorMessage(error), "error");
      }
    } finally {
      deployingKey.set(null);
    }
  };

  const handleProjectDeployToggle = async (
    skill: Skill,
    project: ProjectWorkspace,
    agent: AgentInfo,
    currentlyDeployed: boolean,
  ) => {
    const scopePath = selectedScopePath();
    const key = `project:${project.id}:${scopePath}:${skill.id}:${agent.key}`;
    if (deployingKey()) {
      return;
    }
    deployingKey.set(key);
    const displayScope = scopePath ? `${project.name} · ${scopePath}` : project.name;
    try {
      if (currentlyDeployed) {
        await undeployProjectSkill(skill.id, project.id, agent.key, scopePath);
        props.notify(toastProjectUndeployed(props.locale, displayScope, agent.key), "success");
      } else {
        const dep = await deployProjectSkill(skill.id, project.id, agent.key, {
          scopePath,
          force: false,
        });
        props.notify(
          toastProjectDeployed(props.locale, displayScope, dep.agent_key, dep.mode),
          "success",
        );
      }
      props.refresh();
      await fetchState({
        preferredSkillId: skill.id,
        projectIdOverride: project.id,
        scopePathOverride: scopePath,
      });
    } catch (error) {
      if (!currentlyDeployed && isTargetConflictError(error)) {
        const relDir = agent.project_relative_skills_dir.trim() || ".agents/skills";
        const scopedDir = scopePath ? joinPath(project.path, scopePath) : project.path;
        const targetPath = joinPath(scopedDir, relDir, skill.slug);
        const { action } = await AlertDialog.show({
          title: s().confirm_conflict_title,
          description: confirmConflictDesc(props.locale, agent.display_name, targetPath),
          actions: [
            { id: "cancel", label: s().btn_cancel, role: "cancel" },
            { id: "replace", label: s().btn_replace, role: "destructive" },
          ],
        });
        if (action === "replace") {
          try {
            const dep = await deployProjectSkill(skill.id, project.id, agent.key, {
              scopePath,
              force: true,
            });
            props.notify(
              toastProjectDeployed(props.locale, displayScope, dep.agent_key, dep.mode),
              "success",
            );
            props.refresh();
            await fetchState({
              preferredSkillId: skill.id,
              projectIdOverride: project.id,
              scopePathOverride: scopePath,
            });
          } catch (forceError) {
            props.notify(errorMessage(forceError), "error");
          }
        }
      } else {
        props.notify(errorMessage(error), "error");
      }
    } finally {
      deployingKey.set(null);
    }
  };

  // Sync incoming route parameters and shell revisions while active.
  effect(() => {
    const rev = props.revision;
    const active = isActive();
    const route = props.route;
    const becameActive = active && !wasActive;
    wasActive = active;

    if (!active || route.page !== "library") {
      void rev;
      return;
    }

    const sig = `${route.page}|${route.skillId ?? ""}|${route.projectId ?? ""}|${route.scopePath ?? ""}|${route.agentKey ?? ""}|${route.focusSearch ? "1" : "0"}`;
    const routeChanged = becameActive || sig !== lastRouteSignature;
    lastRouteSignature = sig;

    if (routeChanged) {
      if (route.projectId !== undefined) {
        selectedProjectId.set(route.projectId || null);
        selectedScopePath.set(route.projectId ? (route.scopePath ?? "") : "");
      }
      if (route.agentKey !== undefined) {
        agentFilter.set(route.agentKey || null);
      }
      if (route.focusSearch) {
        queueMicrotask(() => searchInputRef?.focus());
      }
    }

    void fetchState({
      preferredSkillId: routeChanged && route.skillId ? route.skillId : undefined,
      projectIdOverride:
        routeChanged && route.projectId !== undefined ? route.projectId || null : undefined,
      scopePathOverride:
        routeChanged && route.projectId !== undefined
          ? route.projectId
            ? (route.scopePath ?? "")
            : ""
          : undefined,
      confirmRouteSkillSwitch: routeChanged && Boolean(route.skillId),
    });
    void rev;
  });

  // Scope keyboard shortcuts to while Library is the active retained screen.
  effect(() => {
    if (!isActive()) {
      return;
    }
    const unbindSearch = keymap.bind("mod+k", () => {
      searchInputRef?.focus();
    });
    const unbindEdit = keymap.bind(
      {
        "mod+s": () => {
          if (isEditingCurrent()) {
            void saveMarkdown();
          }
        },
        escape: () => {
          if (isEditingCurrent()) {
            void requestCancelEdit();
          }
        },
      },
      { when: "always" },
    );
    cleanup(() => {
      unbindSearch();
      unbindEdit();
    });
  });

  return (
    <View
      style={{
        flex: 1,
        minHeight: 0,
        minWidth: 0,
        backgroundColor: props.theme.background,
      }}
    >
      <PageHeader
        theme={props.theme}
        title={s().nav_library}
        busyText={busyLabel()}
        right={
          <>
            <If when={Boolean(activeProject())}>
              <ActionButton
                theme={props.theme}
                kind="outline"
                icon="◫"
                label={s().project_overview}
                onPress={() => {
                  const project = activeProject();
                  if (!project) {
                    return;
                  }
                  props.navigate({
                    page: "projects",
                    projectId: project.id,
                    scopePath: selectedScopePath(),
                    agentKey: agentFilter() ?? undefined,
                  });
                }}
              />
            </If>
            <ActionButton
              theme={props.theme}
              kind="outline"
              icon="+"
              label={s().btn_install}
              onPress={() => props.navigate({ page: "discover", tab: "install" })}
            />
          </>
        }
      />

      {/* Library Toolbar */}
      <View
        style={{
          minHeight: 48,
          flexDirection: "row",
          flexWrap: "wrap",
          alignItems: "center",
          gap: 8,
          paddingHorizontal: 20,
          paddingVertical: 8,
          borderBottomWidth: 1,
          borderColor: props.theme.border,
          backgroundColor: props.theme.background,
          flexShrink: 0,
        }}
      >
        <SearchField
          theme={props.theme}
          inputRef={(ref) => {
            searchInputRef = ref;
          }}
          value={searchQuery()}
          placeholder={s().ph_search_skills}
          accessibilityLabel={s().ph_search_skills}
          clearLabel={s().adopt_clear_selection}
          minWidth={160}
          maxWidth={280}
          onChange={(value) => searchQuery.set(value)}
        />

        {/* Scope dropdown Menu */}
        <Menu
          offset={4}
          popupStyle={{
            minWidth: 220,
            borderRadius: 10,
            borderWidth: 1,
            borderColor: props.theme.border,
            backgroundColor: props.theme.surface,
            padding: 4,
          }}
          trigger={
            <View
              accessibilityLabel={s().library_scope}
              tooltip={hintTooltip(
                () => props.theme,
                () => s().library_scope,
              )}
              class="transition-colors"
              style={{
                minHeight: 34,
                flexDirection: "row",
                alignItems: "center",
                gap: 6,
                maxWidth: 260,
                paddingHorizontal: 12,
                paddingVertical: 6,
                borderRadius: props.theme.radius ?? 8,
                borderWidth: 1,
                borderColor: props.theme.border,
                backgroundColor: props.theme.background,
              }}
              hoverStyle={{ backgroundColor: props.theme.accent }}
              focusVisibleStyle={{ borderColor: props.theme.primary }}
            >
              <Text
                style={{
                  numberOfLines: 1,
                  textOverflow: "ellipsis",
                  fontSize: 13,
                  color: props.theme.foreground,
                  fontFamily: props.theme.fontFamily,
                  flexShrink: 1,
                }}
              >
                {scopeLabel()}
              </Text>
              <Text style={{ fontSize: 11, color: props.theme.muted }}>▾</Text>
            </View>
          }
        >
          <MenuItem
            role="menuitemradio"
            checked={selectedProjectId() === null}
            onSelect={() => {
              void changeScope(null, "");
            }}
            style={{
              flexDirection: "row",
              alignItems: "center",
              gap: 8,
              paddingHorizontal: 10,
              paddingVertical: 7,
              borderRadius: 6,
            }}
            focusStyle={{ backgroundColor: props.theme.accent }}
            hoverStyle={{ backgroundColor: props.theme.accent }}
          >
            <Text style={{ width: 14, fontSize: 12, color: props.theme.primary }}>
              {selectedProjectId() === null ? "✓" : ""}
            </Text>
            <Label theme={props.theme} size={13}>
              {s().scope_global}
            </Label>
          </MenuItem>

          <If when={Boolean(activeProject() && selectedScopePath())}>
            <MenuItem
              role="menuitemradio"
              checked
              onSelect={() => {
                const project = activeProject();
                if (project) {
                  void changeScope(project.id, selectedScopePath());
                }
              }}
              style={{
                flexDirection: "row",
                alignItems: "center",
                gap: 8,
                paddingHorizontal: 10,
                paddingVertical: 7,
                borderRadius: 6,
              }}
              focusStyle={{ backgroundColor: props.theme.accent }}
              hoverStyle={{ backgroundColor: props.theme.accent }}
            >
              <Text style={{ width: 14, fontSize: 12, color: props.theme.primary }}>✓</Text>
              <Label theme={props.theme} size={13} truncate>
                {scopeLabel()}
              </Label>
            </MenuItem>
          </If>

          <Each of={projects()} key={(project) => project.id}>
            {(project) => (
              <MenuItem
                role="menuitemradio"
                checked={selectedProjectId() === project().id && selectedScopePath() === ""}
                onSelect={() => {
                  void changeScope(project().id, "");
                }}
                style={{
                  flexDirection: "row",
                  alignItems: "center",
                  gap: 8,
                  paddingHorizontal: 10,
                  paddingVertical: 7,
                  borderRadius: 6,
                }}
                focusStyle={{ backgroundColor: props.theme.accent }}
                hoverStyle={{ backgroundColor: props.theme.accent }}
              >
                <Text style={{ width: 14, fontSize: 12, color: props.theme.primary }}>
                  {selectedProjectId() === project().id && selectedScopePath() === "" ? "✓" : ""}
                </Text>
                <Label theme={props.theme} size={13} truncate>
                  {project().name}
                </Label>
              </MenuItem>
            )}
          </Each>

          <MenuSeparator
            style={{ height: 1, marginVertical: 4, backgroundColor: props.theme.border }}
          />

          <MenuItem
            onSelect={() => {
              void handleAttachProject();
            }}
            style={{
              flexDirection: "row",
              alignItems: "center",
              gap: 8,
              paddingHorizontal: 10,
              paddingVertical: 7,
              borderRadius: 6,
            }}
            focusStyle={{ backgroundColor: props.theme.accent }}
            hoverStyle={{ backgroundColor: props.theme.accent }}
          >
            <Text style={{ width: 14, fontSize: 12, color: props.theme.primary }}>+</Text>
            <Label theme={props.theme} size={13}>
              {s().btn_add_project}
            </Label>
          </MenuItem>
        </Menu>

        {/* Agent filter Select */}
        <Select
          items={agentSelectItems()}
          value={agentFilter() ?? ALL_AGENTS_VALUE}
          onChange={({ value }) => {
            agentFilter.set(!value || value === ALL_AGENTS_VALUE ? null : value);
          }}
          accessibilityLabel={s().library_agent_filter}
          offset={4}
          popupStyle={{
            minWidth: 200,
            borderRadius: 10,
            borderWidth: 1,
            borderColor: props.theme.border,
            backgroundColor: props.theme.surface,
            padding: 4,
          }}
          itemStyle={{
            paddingHorizontal: 10,
            paddingVertical: 7,
            borderRadius: 6,
            fontSize: 13,
            color: props.theme.foreground,
            fontFamily: props.theme.fontFamily,
          }}
          trigger={
            <View
              tooltip={hintTooltip(
                () => props.theme,
                () => s().library_agent_filter,
              )}
              class="transition-colors"
              style={{
                minHeight: 34,
                flexDirection: "row",
                alignItems: "center",
                gap: 6,
                maxWidth: 224,
                paddingHorizontal: 12,
                paddingVertical: 6,
                borderRadius: props.theme.radius ?? 8,
                borderWidth: 1,
                borderColor: props.theme.border,
                backgroundColor: props.theme.background,
              }}
              hoverStyle={{ backgroundColor: props.theme.accent }}
              focusVisibleStyle={{ borderColor: props.theme.primary }}
            >
              <Text
                style={{
                  numberOfLines: 1,
                  textOverflow: "ellipsis",
                  fontSize: 13,
                  color: props.theme.foreground,
                  fontFamily: props.theme.fontFamily,
                  flexShrink: 1,
                }}
              >
                {selectedAgentLabel()}
              </Text>
              <Text style={{ fontSize: 11, color: props.theme.muted }}>▾</Text>
            </View>
          }
        />

        <View style={{ flex: 1 }} />

        <ActionButton
          theme={props.theme}
          kind="ghost"
          icon="↻"
          loading={loadingState()}
          accessibilityLabel={s().tooltip_refresh}
          tooltip={s().tooltip_refresh}
          onPress={() => {
            props.refresh();
            void fetchState({ notifyRefreshed: true });
          }}
        />
      </View>

      <If when={Boolean(loadError())}>
        <View style={{ paddingHorizontal: 20, paddingTop: 10 }}>
          <AlertBanner theme={props.theme} tone="error" message={loadError() ?? ""} />
        </View>
      </If>

      {/* Master-Detail Split */}
      <View style={{ flex: 1, minHeight: 0, minWidth: 0 }}>
        <ResizablePanelGroup
          axis="horizontal"
          handleStyle={{ backgroundColor: props.theme.border }}
          handleThumbClass="h-11 w-[3px] rounded-full opacity-0 scale-y-0 transition group-hover:opacity-60 group-hover:scale-y-[0.5] group-active:opacity-100 group-active:scale-y-100"
          handleThumbStyle={{ backgroundColor: props.theme.primary }}
          handleAccessibilityLabel={s().nav_library}
        >
          <ResizablePanel
            size={listPaneWidth()}
            minSize={windowSize().width < 780 ? 240 : 330}
            maxSize={windowSize().width < 780 ? 380 : 560}
            onResize={({ size }) => listPaneWidth.set(size)}
            style={{ minHeight: 0, minWidth: 0, backgroundColor: props.theme.background }}
          >
            <LibraryListPane
              locale={props.locale}
              theme={props.theme}
              skills={skills()}
              filteredSkills={filteredSkills()}
              activeAgents={activeAgents()}
              selectedSkillId={selectedSkillId()}
              projectScope={selectedProjectId() !== null}
              hasProjectStatuses={projectStatuses() !== null}
              projectStatusLoading={projectStatusLoading()}
              projectStatusError={projectStatusError()}
              agentFilter={agentFilter()}
              isAvailableForAgent={isAvailableForAgent}
              onSelectSkill={(id) => {
                void selectSkill(id);
              }}
              onOpenInstall={() => props.navigate({ page: "discover", tab: "install" })}
              onOpenDiscover={() => props.navigate({ page: "discover" })}
            />
          </ResizablePanel>

          <ResizablePanel
            minSize={windowSize().width < 780 ? 280 : 384}
            style={{ minHeight: 0, minWidth: 0, backgroundColor: props.theme.surface }}
          >
            <SkillInspector
              locale={props.locale}
              theme={props.theme}
              skill={selectedSkill()}
              activeAgents={activeAgents()}
              activeProject={activeProject()}
              scopePath={selectedScopePath()}
              scopeLabel={scopeLabel()}
              projectStatusLoading={projectStatusLoading()}
              projectStatusError={projectStatusError()}
              projectStatusFor={projectStatusFor}
              inspectorTab={inspectorTab()}
              editing={isEditingCurrent()}
              canEdit={canEditCurrent()}
              markdownLoading={markdownLoading()}
              markdownError={markdownError()}
              savingMarkdown={savingSkillId() !== null}
              draftMarkdown={draftMarkdown()}
              previewMarkdown={previewMarkdown()}
              selectedLint={selectedLint()}
              updatingSkillId={updatingSkillId()}
              deploying={deployingKey() !== null}
              onTabChange={(tab) => inspectorTab.set(tab)}
              onStartEdit={startEditMarkdown}
              onCancelEdit={() => {
                void requestCancelEdit();
              }}
              onSaveEdit={() => {
                void saveMarkdown();
              }}
              onDraftChange={(value) => draftMarkdown.set(value)}
              onUpdateSkill={(skill) => {
                void handleUpdateSkill(skill);
              }}
              onDeleteSkill={(skill) => {
                void handleDeleteSkill(skill);
              }}
              onGlobalDeployToggle={(skill, agent, deployed) => {
                void handleGlobalDeployToggle(skill, agent, deployed);
              }}
              onProjectDeployToggle={(skill, project, agent, deployed) => {
                void handleProjectDeployToggle(skill, project, agent, deployed);
              }}
              onDetachProject={(project) => {
                void handleDetachProject(project);
              }}
              onNotify={props.notify}
            />
          </ResizablePanel>
        </ResizablePanelGroup>
      </View>
    </View>
  );
}
