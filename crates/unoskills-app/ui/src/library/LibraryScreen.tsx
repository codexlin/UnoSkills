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
  untracked,
  windowSize,
} from "nsh";
import {
  AlertDialog,
  ResizablePanel,
  ResizablePanelGroup,
} from "nsh/base";
import { ActionMenu, AlertBanner, Label, MenuAction, MenuDivider, SelectControl } from "../components";
import type { ScreenProps } from "../shared";
import { LibraryListPane } from "./LibraryListPane";
import { SkillInspector } from "./SkillInspector";
import { ActionButton, PageHeader, SearchField } from "./controls";
import { createProjectScope } from "./scope";
import { createDeployments } from "./deployments";
import {
  attachProject,
  detachProject,
  errorMessage,
  getSkillMarkdown,
  loadLibraryState,
  pickFolder,
  removeSkill,
  saveSkillMarkdown,
  updateSkill,
} from "./host";
import {
  busyUpdatingSkill,
  confirmDeleteSkillDesc,
  confirmDetachProjectDesc,
  getStrings,
  toastProjectAttached,
  toastSavedSkill,
  toastSkillUpToDate,
  toastSkillUpdated,
} from "./strings";
import type {
  AgentInfo,
  ProjectWorkspace,
  Skill,
  SkillLintReport,
} from "./types";

const ALL_AGENTS_VALUE = "__all_agents__";

export function Library(props: ScreenProps) {
  const scope = createProjectScope(() => props.locale);
  const skills = signal<Skill[]>([]);
  const agents = signal<AgentInfo[]>([]);
  const selectedSkillId = signal<string | null>(null);
  const agentFilter = signal<string | null>(null);
  const searchQuery = signal("");

  const loadingState = signal(true);
  const loadError = signal<string | null>(null);

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
  const busyLabel = signal<string | null>(null);
  const listPaneWidth = signal(408);

  let searchInputRef: InputRef | undefined;
  let stateReqSeq = 0;
  let markdownReqSeq = 0;
  let lastRouteSignature = "";
  let wasActive = false;

  const s = () => getStrings(props.locale);
  const isActive = () =>
    (props as ScreenProps & { active?: boolean }).active ?? props.route.page === "library";

  const activeAgents = computed(() =>
    agents().filter((agent) => agent.installed && agent.enabled),
  );

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
      return filterAgent ? scope.isAvailable(skill, filterAgent) === true : true;
    });
  });

  const agentSelectItems = computed(() => [
    { value: ALL_AGENTS_VALUE, title: s().library_all_agents },
    ...activeAgents().map((agent) => ({
      value: agent.key,
      title: agent.display_name,
    })),
  ]);

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

  const fetchState = async (options?: {
    preferredSkillId?: string | null;
    projectIdOverride?: string | null;
    scopePathOverride?: string;
    notifyRefreshed?: boolean;
    confirmRouteSkillSwitch?: boolean;
  }) => {
    const req = ++stateReqSeq;
    const requestedProject =
      options?.projectIdOverride !== undefined ? options.projectIdOverride : scope.projectId();
    const requestedScope =
      options?.scopePathOverride !== undefined
        ? options.scopePathOverride
        : requestedProject
          ? scope.path()
          : "";

    loadingState.set(true);
    loadError.set(null);
    const scopeRequest = scope.begin(requestedProject);

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
        scope.apply(scopeRequest, requestedProject, requestedScope, data);
        selectedSkillId.set(nextSkillId);
        agentFilter.set(validAgentFilter);
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
      scope.finish(scopeRequest);
      props.notify(message, "error");
    } finally {
      if (req === stateReqSeq) {
        loadingState.set(false);
      }
    }
  };

  const deployments = createDeployments(props, () => fetchState());

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
        scope.select(route.projectId || null, route.scopePath ?? "");
      }
      if (route.agentKey !== undefined) {
        agentFilter.set(route.agentKey || null);
      }
      if (route.focusSearch) {
        queueMicrotask(() => searchInputRef?.focus());
      }
    }

    untracked(() => void fetchState({
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
    }));
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
            <If when={Boolean(scope.project())}>
              <ActionButton
                theme={props.theme}
                kind="outline"
                icon="◫"
                label={s().project_overview}
                onPress={() => {
                  const project = scope.project();
                  if (!project) {
                    return;
                  }
                  props.navigate({
                    page: "projects",
                    projectId: project.id,
                    scopePath: scope.path(),
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

        <ActionMenu theme={props.theme} label={s().library_scope} value={scope.label()}
          minWidth={220} maxTriggerWidth={260}>
          <MenuAction theme={props.theme} label={s().scope_global} checked={scope.projectId() === null}
            onSelect={() => void scope.change(null, "")} />
          <If when={Boolean(scope.project() && scope.path())}>
            <MenuAction theme={props.theme} label={scope.label()} checked onSelect={() => {
              const project = scope.project();
              if (project) void scope.change(project.id, scope.path());
            }} />
          </If>
          <Each of={scope.projects()} key={(project) => project.id}>
            {(project) => <MenuAction theme={props.theme} label={project().name}
              checked={scope.projectId() === project().id && scope.path() === ""}
              onSelect={() => void scope.change(project().id, "")} />}
          </Each>
          <MenuDivider theme={props.theme} />
          <MenuAction theme={props.theme} label={s().btn_add_project} leading="+"
            onSelect={() => void handleAttachProject()} />
        </ActionMenu>

        <SelectControl theme={props.theme} label={s().library_agent_filter} hint={s().library_agent_filter}
          items={agentSelectItems()} value={agentFilter() ?? ALL_AGENTS_VALUE} displayValue={selectedAgentLabel()}
          toolbar maxWidth={224} popupMinWidth={200}
          onChange={(value) => agentFilter.set(!value || value === ALL_AGENTS_VALUE ? null : value)} />

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
              projectScope={scope.projectId() !== null}
              hasProjectStatuses={scope.statuses() !== null}
              projectStatusLoading={scope.loading()}
              projectStatusError={scope.error()}
              agentFilter={agentFilter()}
              isAvailableForAgent={scope.isAvailable}
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
              activeProject={scope.project()}
              scopePath={scope.path()}
              scopeLabel={scope.label()}
              projectStatusLoading={scope.loading()}
              projectStatusError={scope.error()}
              projectStatusFor={scope.statusFor}
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
              deploying={deployments.pending()}
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
                void deployments.global(skill, agent, deployed);
              }}
              onProjectDeployToggle={(skill, project, agent, deployed) => {
                void deployments.project(skill, project, scope.path(), agent, deployed);
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
