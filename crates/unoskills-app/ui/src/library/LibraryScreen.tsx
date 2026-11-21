import {
  Clipboard,
  type InputRef,
  List,
  Scroll,
  Text,
  View,
  batch,
  cleanup,
  computed,
  effect,
  keymap,
  openUrl,
  signal,
} from "nsh";
import {
  AlertDialog,
  Button,
  Input,
  Menu,
  MenuItem,
  MenuSeparator,
  ResizablePanel,
  ResizablePanelGroup,
  Select,
  Switch,
  Tab,
  Tabs,
  TextView,
} from "nsh/base";
import type { ScreenProps } from "../shared";
import { ActionButton, AlertBanner, EmptyState, PageHeader, TokenBadge, hintTooltip } from "./controls";
import {
  compactSource,
  isProjectSkillAvailable,
  isSkillDeployedTo,
  joinPath,
} from "./format";
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
  revealPath,
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
  const editingMarkdown = signal(false);
  const loadedMarkdown = signal("");
  const draftMarkdown = signal("");
  const previewMarkdown = signal("");
  const markdownLoading = signal(false);
  const savingMarkdown = signal(false);
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
    if (!id) {
      return null;
    }
    return projects().find((project) => project.id === id) ?? null;
  });

  const selectedSkill = computed(() => {
    const id = selectedSkillId();
    if (!id) {
      return null;
    }
    return skills().find((skill) => skill.id === id) ?? null;
  });

  const isDirty = () => editingMarkdown() && draftMarkdown() !== loadedMarkdown();

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
      if (!filterAgent) {
        return true;
      }
      return isAvailableForAgent(skill, filterAgent) === true;
    });
  });

  const agentSelectItems = computed(() => [
    { value: ALL_AGENTS_VALUE, title: s().library_all_agents },
    ...activeAgents().map((agent) => ({
      value: agent.key,
      title: agent.display_name,
    })),
  ]);

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
      batch(() => {
        markdownLoading.set(false);
        selectedLint.set(null);
        previewMarkdown.set("");
        if (!preserveEditor) {
          loadedMarkdown.set("");
          draftMarkdown.set("");
          editingMarkdown.set(false);
        }
      });
      return;
    }

    markdownLoading.set(true);
    try {
      const data = await getSkillMarkdown(skillId);
      if (req !== markdownReqSeq) {
        return;
      }
      batch(() => {
        previewMarkdown.set(data.preview_markdown);
        selectedLint.set(data.lint);
        if (!preserveEditor) {
          loadedMarkdown.set(data.raw_markdown);
          draftMarkdown.set(data.raw_markdown);
        }
      });
    } catch (error) {
      if (req !== markdownReqSeq) {
        return;
      }
      batch(() => {
        previewMarkdown.set("");
        selectedLint.set(null);
      });
      props.notify(errorMessage(error), "error");
    } finally {
      if (req === markdownReqSeq) {
        markdownLoading.set(false);
      }
    }
  };

  const refreshProjectStatusesOnly = async (projectId: string | null) => {
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
      const data = await loadProjectStatuses(projectId);
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
    notifyRefreshed?: boolean;
  }) => {
    const req = ++stateReqSeq;
    const requestedProject =
      options?.projectIdOverride !== undefined ? options.projectIdOverride : selectedProjectId();
    loadingState.set(true);
    loadError.set(null);
    if (requestedProject) {
      projectStatusLoading.set(true);
    }

    try {
      const data = await loadLibraryState(requestedProject);
      if (req !== stateReqSeq) {
        return;
      }
      const prevSelected = selectedSkillId();
      const targetSkillId = options?.preferredSkillId ?? prevSelected;
      const nextSkillId =
        targetSkillId && data.skills.some((skill) => skill.id === targetSkillId)
          ? targetSkillId
          : (data.skills[0]?.id ?? null);

      const validProjectId =
        requestedProject && data.projects.some((project) => project.id === requestedProject)
          ? requestedProject
          : null;

      const currentAgentFilter = agentFilter();
      const validAgentFilter =
        currentAgentFilter &&
        data.agents.some(
          (agent) => agent.key === currentAgentFilter && agent.installed && agent.enabled,
        )
          ? currentAgentFilter
          : null;

      const preserveEditor = editingMarkdown() && prevSelected !== null && prevSelected === nextSkillId;

      batch(() => {
        skills.set(data.skills);
        agents.set(data.agents);
        projects.set(data.projects);
        selectedSkillId.set(nextSkillId);
        selectedProjectId.set(validProjectId);
        if (!validProjectId) {
          selectedScopePath.set("");
        }
        agentFilter.set(validAgentFilter);
        projectStatuses.set(validProjectId ? data.project_statuses : null);
        projectStatusError.set(validProjectId ? data.project_status_error : null);
        projectStatusLoading.set(false);
        if (!preserveEditor) {
          editingMarkdown.set(false);
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
    batch(() => {
      selectedSkillId.set(skillId);
      editingMarkdown.set(false);
    });
    await loadSelectedMarkdown(skillId, false);
  };

  const changeScope = async (projectId: string | null, scopePath = "") => {
    batch(() => {
      selectedProjectId.set(projectId);
      selectedScopePath.set(projectId ? scopePath : "");
    });
    await refreshProjectStatusesOnly(projectId);
  };

  const saveMarkdown = async () => {
    const skill = selectedSkill();
    if (!skill || savingMarkdown()) {
      return;
    }
    savingMarkdown.set(true);
    try {
      const result = await saveSkillMarkdown(skill.id, draftMarkdown());
      batch(() => {
        loadedMarkdown.set(result.raw_markdown);
        draftMarkdown.set(result.raw_markdown);
        previewMarkdown.set(result.preview_markdown);
        selectedLint.set(result.lint);
        editingMarkdown.set(false);
        skills.update((items) =>
          items.map((item) => (item.id === result.skill.id ? result.skill : item)),
        );
      });
      props.notify(
        toastSavedSkill(props.locale, result.skill.name, result.skill.estimated_tokens),
        "success",
      );
      props.refresh();
    } catch (error) {
      props.notify(errorMessage(error), "error");
    } finally {
      savingMarkdown.set(false);
    }
  };

  const requestCancelEdit = async () => {
    if (!editingMarkdown()) {
      return;
    }
    if (!(await confirmDiscardIfDirty())) {
      return;
    }
    batch(() => {
      draftMarkdown.set(loadedMarkdown());
      editingMarkdown.set(false);
    });
  };

  const handleUpdateSkill = async (skill: Skill) => {
    if (updatingSkillId()) {
      return;
    }
    batch(() => {
      updatingSkillId.set(skill.id);
      busyLabel.set(busyUpdatingSkill(props.locale, skill.name));
    });
    try {
      const outcome = await updateSkill(skill.id);
      if (outcome.updated) {
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
      selectedScopePath.set("");
      await fetchState({ projectIdOverride: project.id });
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
      await fetchState({ projectIdOverride: null });
    } catch (error) {
      props.notify(errorMessage(error), "error");
    }
  };

  const handleGlobalDeployToggle = async (skill: Skill, agent: AgentInfo, currentlyDeployed: boolean) => {
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
    const key = `project:${project.id}:${skill.id}:${agent.key}`;
    if (deployingKey()) {
      return;
    }
    deployingKey.set(key);
    try {
      if (currentlyDeployed) {
        await undeployProjectSkill(skill.id, project.id, agent.key);
        props.notify(toastProjectUndeployed(props.locale, project.name, agent.key), "success");
      } else {
        const dep = await deployProjectSkill(skill.id, project.id, agent.key, { force: false });
        props.notify(
          toastProjectDeployed(props.locale, project.name, dep.agent_key, dep.mode),
          "success",
        );
      }
      props.refresh();
      await fetchState({ preferredSkillId: skill.id, projectIdOverride: project.id });
    } catch (error) {
      if (!currentlyDeployed && isTargetConflictError(error)) {
        const relDir = agent.project_relative_skills_dir.trim() || ".agents/skills";
        const targetPath = joinPath(project.path, relDir, skill.slug);
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
            const dep = await deployProjectSkill(skill.id, project.id, agent.key, { force: true });
            props.notify(
              toastProjectDeployed(props.locale, project.name, dep.agent_key, dep.mode),
              "success",
            );
            props.refresh();
            await fetchState({ preferredSkillId: skill.id, projectIdOverride: project.id });
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

  // Sync incoming route parameters (skillId, projectId, scopePath, agentKey, focusSearch) and shell revisions.
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
        selectedScopePath.set(route.scopePath ?? "");
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
          if (editingMarkdown()) {
            void saveMarkdown();
          }
        },
        escape: () => {
          if (editingMarkdown()) {
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

  const scopeLabel = () => activeProject()?.name ?? s().scope_global;

  const selectedAgentLabel = () => {
    const key = agentFilter();
    if (!key) {
      return s().library_all_agents;
    }
    return activeAgents().find((agent) => agent.key === key)?.display_name ?? s().library_all_agents;
  };

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
              label={`+ ${s().btn_install}`}
              onPress={() => props.navigate({ page: "discover", tab: "install" })}
            />
          </>
        }
      />

      {/* Library Toolbar */}
      <View
        style={{
          flexDirection: "row",
          flexWrap: "wrap",
          alignItems: "center",
          gap: 8,
          paddingHorizontal: 20,
          paddingVertical: 10,
          borderBottomWidth: 1,
          borderColor: props.theme.border,
          backgroundColor: props.theme.background,
          flexShrink: 0,
        }}
      >
        <View style={{ flex: 1, minWidth: 160, maxWidth: 280, flexDirection: "row", alignItems: "center", gap: 4 }}>
          <Input
            ref={searchInputRef}
            value={searchQuery()}
            onChange={({ value }) => searchQuery.set(value)}
            placeholder={s().ph_search_skills}
            accessibilityLabel={s().ph_search_skills}
            style={{
              flex: 1,
              fontSize: 13,
              paddingHorizontal: 10,
              paddingVertical: 6,
              borderRadius: 8,
              borderWidth: 1,
              borderColor: props.theme.border,
              backgroundColor: props.theme.surface,
              color: props.theme.foreground,
              placeholderColor: props.theme.muted,
              fontFamily: props.theme.fontFamily,
            }}
          />
          <If when={searchQuery().length > 0}>
            <ActionButton
              theme={props.theme}
              kind="ghost"
              compact
              label="×"
              accessibilityLabel={s().adopt_clear_selection}
              onPress={() => searchQuery.set("")}
            />
          </If>
        </View>

        {/* Scope dropdown Menu: Global, attached projects, + Add Project */}
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
              style={{
                flexDirection: "row",
                alignItems: "center",
                gap: 6,
                maxWidth: 224,
                paddingHorizontal: 12,
                paddingVertical: 6,
                borderRadius: 8,
                borderWidth: 1,
                borderColor: props.theme.border,
                backgroundColor: props.theme.surface,
              }}
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
            <Text style={{ fontSize: 13, color: props.theme.foreground, fontFamily: props.theme.fontFamily }}>
              {s().scope_global}
            </Text>
          </MenuItem>

          <Each of={projects()} key={(project) => project.id}>
            {(project) => (
              <MenuItem
                role="menuitemradio"
                checked={selectedProjectId() === project().id}
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
                  {selectedProjectId() === project().id ? "✓" : ""}
                </Text>
                <Text
                  style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                    flex: 1,
                    fontSize: 13,
                    color: props.theme.foreground,
                    fontFamily: props.theme.fontFamily,
                  }}
                >
                  {project().name}
                </Text>
              </MenuItem>
            )}
          </Each>

          <MenuSeparator
            style={{
              height: 1,
              marginVertical: 4,
              backgroundColor: props.theme.border,
            }}
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
            <Text style={{ fontSize: 13, color: props.theme.foreground, fontFamily: props.theme.fontFamily }}>
              {s().btn_add_project}
            </Text>
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
              style={{
                flexDirection: "row",
                alignItems: "center",
                gap: 6,
                maxWidth: 224,
                paddingHorizontal: 12,
                paddingVertical: 6,
                borderRadius: 8,
                borderWidth: 1,
                borderColor: props.theme.border,
                backgroundColor: props.theme.surface,
              }}
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
          label="↻"
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
          handleAccessibilityLabel={s().nav_library}
        >
          {/* Left Pane: Virtualized Skill List */}
          <ResizablePanel
            size={listPaneWidth()}
            minSize={320}
            maxSize={560}
            onResize={({ size }) => listPaneWidth.set(size)}
            style={{
              flex: 1,
              minHeight: 0,
              minWidth: 0,
              backgroundColor: props.theme.background,
            }}
          >
            <If
              when={Boolean(
                selectedProjectId() &&
                  agentFilter() &&
                  skills().length > 0 &&
                  (projectStatusLoading() || projectStatusError()),
              )}
              else={
                <View style={{ flex: 1, minHeight: 0, minWidth: 0 }}>
                  <If when={filteredSkills().length > 0}>
                    <View
                      style={{
                        height: 32,
                        flexDirection: "row",
                        alignItems: "center",
                        paddingHorizontal: 16,
                        gap: 8,
                        borderBottomWidth: 1,
                        borderColor: props.theme.border,
                        flexShrink: 0,
                      }}
                    >
                      <Text
                        style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                          flex: 1,
                          minWidth: 0,
                          fontSize: 12,
                          color: props.theme.muted,
                          fontFamily: props.theme.fontFamily,
                        }}
                      >
                        {s().library_skill_column}
                      </Text>
                      <Text
                        style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                          width: 76,
                          textAlign: "right",
                          fontSize: 12,
                          color: props.theme.muted,
                          fontFamily: props.theme.fontFamily,
                          flexShrink: 0,
                        }}
                      >
                        {selectedProjectId() ? s().library_project_available : s().library_deployment}
                      </Text>
                      <Text
                        style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                          width: 96,
                          textAlign: "right",
                          fontSize: 12,
                          color: props.theme.muted,
                          fontFamily: props.theme.fontFamily,
                          flexShrink: 0,
                        }}
                      >
                        {s().library_token_column}
                      </Text>
                    </View>
                  </If>

                  <If
                    when={filteredSkills().length === 0}
                    else={
                      <List
                        each={filteredSkills()}
                        keyExtractor={(skill) => skill.id}
                        estimatedItemSize={52}
                        contentStyle={{ paddingVertical: 6 }}
                        style={{ flex: 1, minHeight: 0 }}
                      >
                        {(skill) => (
                          <View style={{ paddingHorizontal: 8, paddingVertical: 2 }}>
                            <Button
                              selected={selectedSkillId() === skill().id}
                              accessibilityLabel={skill().name}
                              onPress={() => {
                                void selectSkill(skill().id);
                              }}
                              style={{
                                height: 48,
                                flexDirection: "row",
                                alignItems: "center",
                                gap: 8,
                                paddingHorizontal: 10,
                                borderRadius: 8,
                                backgroundColor:
                                  selectedSkillId() === skill().id
                                    ? props.theme.accent
                                    : props.theme.background,
                              }}
                              hoverStyle={{
                                backgroundColor: props.theme.accent,
                              }}
                              selectedStyle={{
                                backgroundColor: props.theme.accent,
                              }}
                            >
                              <View style={{ flex: 1, minWidth: 0, gap: 2 }}>
                                <Text
                                  style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                                    fontSize: 13,
                                    fontWeight: "600",
                                    color: props.theme.foreground,
                                    fontFamily: props.theme.fontFamily,
                                  }}
                                >
                                  {skill().name}
                                </Text>
                                <Text
                                  style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                                    fontSize: 11,
                                    color: props.theme.muted,
                                    fontFamily: props.theme.fontFamily,
                                  }}
                                >
                                  {compactSource(
                                    skill().source_ref ?? skill().source_type,
                                    skill().source_type,
                                  )}
                                </Text>
                              </View>

                              <Text
                                style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                                  width: 76,
                                  textAlign: "right",
                                  fontSize: 13,
                                  color: props.theme.foreground,
                                  fontFamily: props.theme.fontFamily,
                                  flexShrink: 0,
                                }}
                              >
                                {selectedProjectId() && !projectStatuses()
                                  ? "—"
                                  : `${
                                      activeAgents().filter(
                                        (agent) => isAvailableForAgent(skill(), agent.key) === true,
                                      ).length
                                    } / ${activeAgents().length}`}
                              </Text>

                              <View
                                style={{
                                  width: 96,
                                  flexDirection: "row",
                                  justifyContent: "flex-end",
                                  flexShrink: 0,
                                }}
                              >
                                <TokenBadge theme={props.theme} tokens={skill().estimated_tokens} />
                              </View>
                            </Button>
                          </View>
                        )}
                      </List>
                    }
                  >
                    <EmptyState
                      theme={props.theme}
                      title={
                        skills().length === 0 ? s().empty_library_title : s().empty_search_title
                      }
                      description={
                        skills().length === 0 ? s().empty_library_desc : s().empty_search_desc
                      }
                      actions={
                        skills().length === 0 ? (
                          <>
                            <ActionButton
                              theme={props.theme}
                              kind="primary"
                              label={s().btn_install}
                              onPress={() => props.navigate({ page: "discover", tab: "install" })}
                            />
                            <ActionButton
                              theme={props.theme}
                              kind="outline"
                              label={s().btn_discover_adopt}
                              onPress={() => props.navigate({ page: "discover" })}
                            />
                          </>
                        ) : undefined
                      }
                    />
                  </If>
                </View>
              }
            >
              <View style={{ padding: 16 }}>
                <If
                  when={Boolean(projectStatusError())}
                  else={
                    <Text
                      role="summary"
            accessibilityState={{ busy: true }}
                      style={{
                        fontSize: 13,
                        color: props.theme.muted,
                        fontFamily: props.theme.fontFamily,
                      }}
                    >
                      {s().stack_project_checking}
                    </Text>
                  }
                >
                  <AlertBanner
                    theme={props.theme}
                    tone="error"
                    message={projectStatusError() ?? ""}
                  />
                </If>
              </View>
            </If>
          </ResizablePanel>

          {/* Right Pane: Skill Inspector (Deployments / SKILL.md Preview & Quick Edit) */}
          <ResizablePanel
            minSize={384}
            style={{
              flex: 1,
              minHeight: 0,
              minWidth: 0,
              backgroundColor: props.theme.surface,
            }}
          >
            <If
              when={selectedSkill()}
              else={
                <EmptyState
                  theme={props.theme}
                  title={s().empty_select_skill_title}
                  description={s().empty_select_skill_desc}
                />
              }
            >
              {(skill) => (
                <View style={{ flex: 1, minHeight: 0, minWidth: 0 }}>
                  {/* Inspector Header */}
                  <View
                    style={{
                      flexDirection: "row",
                      alignItems: "center",
                      gap: 10,
                      paddingHorizontal: 16,
                      paddingVertical: 12,
                      flexShrink: 0,
                    }}
                  >
                    <View style={{ flex: 1, minWidth: 0, gap: 3 }}>
                      <Text
                        style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                          fontSize: 17,
                          fontWeight: "600",
                          color: props.theme.foreground,
                          fontFamily: props.theme.fontFamily,
                        }}
                      >
                        {skill().name}
                      </Text>
                      <Text
                        tooltip={hintTooltip(
                          () => props.theme,
                          () =>
                            `${skill().source_ref ?? skill().source_type} · ~${skill().estimated_tokens} tokens`,
                        )}
                        style={{
                          numberOfLines: 1,
                          textOverflow: "ellipsis",
                          fontSize: 12,
                          color: props.theme.muted,
                          fontFamily: props.theme.fontFamily,
                        }}
                      >
                        {`${skill().source_ref ?? skill().source_type} · ~${skill().estimated_tokens} tokens`}
                      </Text>
                    </View>

                    <If
                      when={editingMarkdown()}
                      else={
                        <View style={{ flexDirection: "row", alignItems: "center", gap: 6, flexShrink: 0 }}>
                          <If when={inspectorTab() === "preview"}>
                            <ActionButton
                              theme={props.theme}
                              kind="outline"
                              label={s().btn_edit_skill}
                              disabled={markdownLoading()}
                              onPress={() => {
                                draftMarkdown.set(loadedMarkdown());
                                editingMarkdown.set(true);
                              }}
                            />
                          </If>

                          {/* Skill Actions Overflow Menu */}
                          <Menu
                            offset={4}
                            align="end"
                            popupStyle={{
                              minWidth: 200,
                              borderRadius: 10,
                              borderWidth: 1,
                              borderColor: props.theme.border,
                              backgroundColor: props.theme.surface,
                              padding: 4,
                            }}
                            trigger={
                              <View
                                accessibilityLabel={s().library_skill_actions}
                                tooltip={hintTooltip(
                                  () => props.theme,
                                  () => s().library_skill_actions,
                                )}
                                style={{
                                  paddingHorizontal: 10,
                                  paddingVertical: 6,
                                  borderRadius: 8,
                                  borderWidth: 1,
                                  borderColor: props.theme.border,
                                  backgroundColor: props.theme.background,
                                }}
                              >
                                <Text style={{ fontSize: 14, color: props.theme.foreground }}>⋯</Text>
                              </View>
                            }
                          >
                            <If
                              when={
                                skill().source_type !== "adopted" &&
                                Boolean(skill().source_ref && skill().source_ref!.trim().length > 0)
                              }
                            >
                              <MenuItem
                                disabled={updatingSkillId() === skill().id}
                                onSelect={() => {
                                  void handleUpdateSkill(skill());
                                }}
                                style={{
                                  paddingHorizontal: 10,
                                  paddingVertical: 7,
                                  borderRadius: 6,
                                }}
                                focusStyle={{ backgroundColor: props.theme.accent }}
                                hoverStyle={{ backgroundColor: props.theme.accent }}
                                disabledStyle={{ opacity: 0.45 }}
                              >
                                <Text
                                  style={{
                                    fontSize: 13,
                                    color: props.theme.foreground,
                                    fontFamily: props.theme.fontFamily,
                                  }}
                                >
                                  {s().btn_update_skill}
                                </Text>
                              </MenuItem>
                            </If>

                            <MenuItem
                              onSelect={() => {
                                void revealPath(skill().central_path).catch((error) => {
                                  props.notify(errorMessage(error), "error");
                                });
                              }}
                              style={{
                                paddingHorizontal: 10,
                                paddingVertical: 7,
                                borderRadius: 6,
                              }}
                              focusStyle={{ backgroundColor: props.theme.accent }}
                              hoverStyle={{ backgroundColor: props.theme.accent }}
                            >
                              <Text
                                style={{
                                  fontSize: 13,
                                  color: props.theme.foreground,
                                  fontFamily: props.theme.fontFamily,
                                }}
                              >
                                {s().tooltip_reveal_folder}
                              </Text>
                            </MenuItem>

                            <MenuItem
                              onSelect={() => {
                                void Clipboard.writeText(skill().central_path)
                                  .then(() => {
                                    props.notify(s().library_copy_path, "info");
                                  })
                                  .catch((error) => {
                                    props.notify(errorMessage(error), "error");
                                  });
                              }}
                              style={{
                                paddingHorizontal: 10,
                                paddingVertical: 7,
                                borderRadius: 6,
                              }}
                              focusStyle={{ backgroundColor: props.theme.accent }}
                              hoverStyle={{ backgroundColor: props.theme.accent }}
                            >
                              <Text
                                style={{
                                  fontSize: 13,
                                  color: props.theme.foreground,
                                  fontFamily: props.theme.fontFamily,
                                }}
                              >
                                {s().library_copy_path}
                              </Text>
                            </MenuItem>

                            <MenuSeparator
                              style={{
                                height: 1,
                                marginVertical: 4,
                                backgroundColor: props.theme.border,
                              }}
                            />

                            <MenuItem
                              onSelect={() => {
                                void handleDeleteSkill(skill());
                              }}
                              style={{
                                paddingHorizontal: 10,
                                paddingVertical: 7,
                                borderRadius: 6,
                              }}
                              focusStyle={{ backgroundColor: props.theme.accent }}
                              hoverStyle={{ backgroundColor: props.theme.accent }}
                            >
                              <Text
                                style={{
                                  fontSize: 13,
                                  fontWeight: "500",
                                  color: props.theme.danger,
                                  fontFamily: props.theme.fontFamily,
                                }}
                              >
                                {s().btn_delete}
                              </Text>
                            </MenuItem>
                          </Menu>
                        </View>
                      }
                    >
                      <View style={{ flexDirection: "row", alignItems: "center", gap: 6, flexShrink: 0 }}>
                        <ActionButton
                          theme={props.theme}
                          kind="ghost"
                          label={s().btn_cancel}
                          disabled={savingMarkdown()}
                          onPress={() => {
                            void requestCancelEdit();
                          }}
                        />
                        <ActionButton
                          theme={props.theme}
                          kind="primary"
                          label={s().btn_save}
                          loading={savingMarkdown()}
                          onPress={() => {
                            void saveMarkdown();
                          }}
                        />
                      </View>
                    </If>
                  </View>

                  {/* Sub-tabs: Deployments | Preview */}
                  <If when={!editingMarkdown()}>
                    <View
                      style={{
                        paddingHorizontal: 16,
                        borderBottomWidth: 1,
                        borderColor: props.theme.border,
                        flexShrink: 0,
                      }}
                    >
                      <Tabs
                        accessibilityLabel={s().nav_library}
                        style={{ flexDirection: "row", gap: 16 }}
                        indicatorStyle={{
                          bottom: 0,
                          height: 2,
                          backgroundColor: props.theme.primary,
                        }}
                      >
                        <Tab
                          selected={inspectorTab() === "deployments"}
                          onPress={() => inspectorTab.set("deployments")}
                          style={{ paddingVertical: 9 }}
                        >
                          <Text
                            style={{
                              fontSize: 13,
                              fontWeight: inspectorTab() === "deployments" ? "600" : "500",
                              color:
                                inspectorTab() === "deployments"
                                  ? props.theme.foreground
                                  : props.theme.muted,
                              fontFamily: props.theme.fontFamily,
                            }}
                          >
                            {s().library_deployment}
                          </Text>
                        </Tab>
                        <Tab
                          selected={inspectorTab() === "preview"}
                          onPress={() => inspectorTab.set("preview")}
                          style={{ paddingVertical: 9 }}
                        >
                          <Text
                            style={{
                              fontSize: 13,
                              fontWeight: inspectorTab() === "preview" ? "600" : "500",
                              color:
                                inspectorTab() === "preview"
                                  ? props.theme.foreground
                                  : props.theme.muted,
                              fontFamily: props.theme.fontFamily,
                            }}
                          >
                            {s().library_preview}
                          </Text>
                        </Tab>
                      </Tabs>
                    </View>
                  </If>

                  {/* Inspector Body */}
                  <If
                    when={editingMarkdown()}
                    else={
                      <Scroll style={{ flex: 1, minHeight: 0 }} contentStyle={{ padding: 16, gap: 12 }}>
                        <If
                          when={inspectorTab() === "preview"}
                          else={
                            /* Deployments Panel */
                            <View style={{ gap: 10 }}>
                              <View style={{ flexDirection: "row", alignItems: "center", gap: 8 }}>
                                <Text
                                  style={{
                                    flex: 1,
                                    minWidth: 0,
                                    fontSize: 14,
                                    fontWeight: "600",
                                    color: props.theme.foreground,
                                    fontFamily: props.theme.fontFamily,
                                  }}
                                >
                                  {s().library_deployment_targets}
                                </Text>
                                <Text
                                  style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                                    maxWidth: 240,
                                    fontSize: 12,
                                    color: props.theme.muted,
                                    fontFamily: props.theme.fontFamily,
                                    flexShrink: 0,
                                  }}
                                >
                                  {scopeLabel()}
                                </Text>
                              </View>

                              <If when={activeProject()}>
                                {(project) => (
                                  <View
                                    style={{
                                      flexDirection: "row",
                                      alignItems: "center",
                                      gap: 8,
                                      paddingVertical: 4,
                                    }}
                                  >
                                    <Text
                                      style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                                        flex: 1,
                                        minWidth: 0,
                                        fontSize: 12,
                                        color: props.theme.muted,
                                        fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                                      }}
                                    >
                                      {project().path}
                                    </Text>
                                    <ActionButton
                                      theme={props.theme}
                                      kind="ghost"
                                      compact
                                      label={s().tooltip_reveal_folder}
                                      onPress={() => {
                                        void revealPath(project().path).catch((error) => {
                                          props.notify(errorMessage(error), "error");
                                        });
                                      }}
                                    />
                                    <ActionButton
                                      theme={props.theme}
                                      kind="ghost"
                                      compact
                                      label={s().btn_detach_project}
                                      onPress={() => {
                                        void handleDetachProject(project());
                                      }}
                                    />
                                  </View>
                                )}
                              </If>

                              <If when={activeAgents().length === 0}>
                                <Text
                                  style={{
                                    fontSize: 13,
                                    color: props.theme.muted,
                                    fontFamily: props.theme.fontFamily,
                                  }}
                                >
                                  {s().no_installed_agents}
                                </Text>
                              </If>

                              <If when={Boolean(selectedProjectId() && projectStatusLoading())}>
                                <Text
                                  role="summary"
            accessibilityState={{ busy: true }}
                                  style={{
                                    fontSize: 12,
                                    color: props.theme.muted,
                                    fontFamily: props.theme.fontFamily,
                                  }}
                                >
                                  {s().stack_project_checking}
                                </Text>
                              </If>

                              <If when={Boolean(selectedProjectId() && projectStatusError())}>
                                <AlertBanner
                                  theme={props.theme}
                                  tone="error"
                                  message={projectStatusError() ?? ""}
                                />
                              </If>

                              <View style={{ borderTopWidth: 1, borderColor: props.theme.border }}>
                                <Each of={activeAgents()} key={(agent) => agent.key}>
                                  {(agent) => (
                                    <View
                                      style={{
                                        minHeight: 42,
                                        flexDirection: "row",
                                        alignItems: "center",
                                        gap: 10,
                                        borderBottomWidth: 1,
                                        borderColor: props.theme.border,
                                        paddingVertical: 6,
                                      }}
                                    >
                                      <Text
                                        style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                                          flex: 1,
                                          minWidth: 0,
                                          fontSize: 13,
                                          color: props.theme.foreground,
                                          fontFamily: props.theme.fontFamily,
                                        }}
                                      >
                                        {agent().display_name}
                                      </Text>

                                      <If
                                        when={activeProject()}
                                        else={
                                          /* Global deployment switch */
                                          <View style={{ flexDirection: "row", alignItems: "center", gap: 8, flexShrink: 0 }}>
                                            <Text
                                              style={{
                                                fontSize: 12,
                                                color: isSkillDeployedTo(skill(), agent().key)
                                                  ? props.theme.foreground
                                                  : props.theme.muted,
                                                fontFamily: props.theme.fontFamily,
                                              }}
                                            >
                                              {isSkillDeployedTo(skill(), agent().key)
                                                ? s().library_deployed
                                                : s().library_not_deployed}
                                            </Text>
                                            <Switch
                                              checked={isSkillDeployedTo(skill(), agent().key)}
                                              disabled={deployingKey() !== null}
                                              accessibilityLabel={`${s().library_deployment} · ${agent().display_name} · ${s().scope_global}`}
                                              onChange={({ checked }) => {
                                                const current = isSkillDeployedTo(skill(), agent().key);
                                                if (checked === current) {
                                                  return;
                                                }
                                                void handleGlobalDeployToggle(skill(), agent(), current);
                                              }}
                                              trackStyle={{
                                                width: 38,
                                                padding: 2,
                                                borderRadius: 999,
                                                backgroundColor: isSkillDeployedTo(skill(), agent().key)
                                                  ? props.theme.primary
                                                  : props.theme.border,
                                              }}
                                              thumbStyle={{
                                                width: 16,
                                                height: 16,
                                                borderRadius: 999,
                                                backgroundColor: props.theme.background,
                                                transform: isSkillDeployedTo(skill(), agent().key)
                                                  ? [{ translateX: 18 }]
                                                  : [{ translateX: 0 }],
                                              }}
                                            />
                                          </View>
                                        }
                                      >
                                        {(project) => (
                                          /* Project-scoped deployment status & controls */
                                          <If when={projectStatusFor(skill().id, agent().key)}>
                                            {(status) => (
                                              <View
                                                style={{
                                                  flexDirection: "row",
                                                  alignItems: "center",
                                                  gap: 8,
                                                  flexShrink: 0,
                                                }}
                                              >
                                                <Text
                                                  style={{
                                                    fontSize: 12,
                                                    color:
                                                      status().placement === "conflict" ||
                                                      status().placement === "unavailable"
                                                        ? props.theme.warning
                                                        : status().placement === "managed" ||
                                                            status().placement === "local"
                                                          ? props.theme.foreground
                                                          : props.theme.muted,
                                                    fontFamily: props.theme.fontFamily,
                                                  }}
                                                >
                                                  {status().placement === "managed"
                                                    ? s().library_project_deployed
                                                    : status().placement === "local"
                                                      ? s().library_project_local
                                                      : status().placement === "conflict"
                                                        ? s().library_project_conflict
                                                        : status().placement === "unavailable"
                                                          ? s().library_project_unavailable
                                                          : status().global_available
                                                            ? s().library_global_available
                                                            : s().library_not_deployed}
                                                </Text>

                                                <If when={status().placement === "managed"}>
                                                  <Switch
                                                    checked
                                                    disabled={deployingKey() !== null}
                                                    accessibilityLabel={`${s().library_project_deployed} · ${agent().display_name}`}
                                                    tooltip={hintTooltip(
                                                      () => props.theme,
                                                      () => s().library_remove_project_deployment,
                                                    )}
                                                    onChange={({ checked }) => {
                                                      if (!checked) {
                                                        void handleProjectDeployToggle(
                                                          skill(),
                                                          project(),
                                                          agent(),
                                                          true,
                                                        );
                                                      }
                                                    }}
                                                    trackStyle={{
                                                      width: 38,
                                                      padding: 2,
                                                      borderRadius: 999,
                                                      backgroundColor: props.theme.primary,
                                                    }}
                                                    thumbStyle={{
                                                      width: 16,
                                                      height: 16,
                                                      borderRadius: 999,
                                                      backgroundColor: props.theme.background,
                                                      transform: [{ translateX: 18 }],
                                                    }}
                                                  />
                                                </If>

                                                <If
                                                  when={
                                                    status().placement === "missing" ||
                                                    status().placement === "conflict"
                                                  }
                                                >
                                                  <ActionButton
                                                    theme={props.theme}
                                                    kind="ghost"
                                                    compact
                                                    disabled={deployingKey() !== null}
                                                    label={
                                                      status().placement === "conflict"
                                                        ? s().library_resolve_project_conflict
                                                        : s().stack_deploy_project
                                                    }
                                                    accessibilityLabel={`${
                                                      status().placement === "conflict"
                                                        ? s().library_resolve_project_conflict
                                                        : s().stack_deploy_project
                                                    } · ${agent().display_name}`}
                                                    onPress={() => {
                                                      void handleProjectDeployToggle(
                                                        skill(),
                                                        project(),
                                                        agent(),
                                                        false,
                                                      );
                                                    }}
                                                  />
                                                </If>
                                              </View>
                                            )}
                                          </If>
                                        )}
                                      </If>
                                    </View>
                                  )}
                                </Each>
                              </View>
                            </View>
                          }
                        >
                          {/* SKILL.md Preview + Lint Warnings */}
                          <View style={{ maxWidth: 720, gap: 12 }}>
                            <If
                              when={Boolean(
                                selectedLint() && selectedLint()!.warnings.length > 0,
                              )}
                            >
                              <AlertBanner
                                theme={props.theme}
                                tone="warning"
                                message={(selectedLint()?.warnings ?? []).join(" · ")}
                              />
                            </If>

                            <If
                              when={Boolean(
                                skill().description && skill().description!.trim().length > 0,
                              )}
                            >
                              <View
                                style={{
                                  paddingBottom: 12,
                                  borderBottomWidth: 1,
                                  borderColor: props.theme.border,
                                }}
                              >
                                <Text
                                  style={{
                                    fontSize: 13,
                                    lineHeight: 20,
                                    color: props.theme.foreground,
                                    fontFamily: props.theme.fontFamily,
                                  }}
                                >
                                  {skill().description ?? ""}
                                </Text>
                              </View>
                            </If>

                            <TextView
                              markdown={previewMarkdown()}
                              selectable
                              onLinkPress={({ url }) => {
                                void openUrl(url);
                              }}
                              styles={{
                                root: {
                                  color: props.theme.foreground,
                                  fontFamily: props.theme.fontFamily,
                                  fontSize: 14,
                                  lineHeight: 22,
                                },
                                link: {
                                  color: props.theme.primary,
                                },
                                code: {
                                  fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                                  backgroundColor: props.theme.background,
                                },
                                codeBlock: {
                                  backgroundColor: props.theme.background,
                                  borderColor: props.theme.border,
                                },
                                codeBlockCode: {
                                  fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                                },
                              }}
                            />
                          </View>
                        </If>
                      </Scroll>
                    }
                  >
                    {/* Quick Edit Mode for SKILL.md (raw frontmatter preserved) */}
                    <View style={{ flex: 1, minHeight: 0, padding: 16 }}>
                      <Input
                        multiline
                        submitBehavior="newline"
                        autoCorrect={false}
                        value={draftMarkdown()}
                        onChange={({ value }) => draftMarkdown.set(value)}
                        placeholder={s().ph_skill_md}
                        accessibilityLabel={s().btn_edit_skill}
                        disabled={savingMarkdown()}
                        style={{
                          flex: 1,
                          minHeight: 0,
                          fontSize: 13,
                          lineHeight: 20,
                          padding: 12,
                          borderRadius: 8,
                          borderWidth: 1,
                          borderColor: props.theme.border,
                          backgroundColor: props.theme.background,
                          color: props.theme.foreground,
                          placeholderColor: props.theme.muted,
                          fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                        }}
                      />
                    </View>
                  </If>
                </View>
              )}
            </If>
          </ResizablePanel>
        </ResizablePanelGroup>
      </View>
    </View>
  );
}
