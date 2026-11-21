import { computed, List, Scroll, signal, View } from "nsh";
import type { ScreenProps } from "../shared";
import { ActionMenu, MenuAction } from "../components";
import { Action, Check, Choice, Feedback, Footer, Header, Heading, Label, actions, confirm, row, viewport } from "./controls";
import { load, operation, toggle } from "./state";
import { attachProjectFromPicker, request, type Agent, type Project } from "./host";
import { projectRecommendationReason, projectSelectionCount, strings } from "./strings";

export function Projects(props: ScreenProps) {
  const t = () => strings(props.locale);
  const targets = load(() => {
    void props.revision;
    return request("project.list", null);
  }, true);
  const op = operation(props);
  const projects = () => targets().data?.projects ?? [];
  const selected = () => projects().find((project) => project.id === props.route.projectId) ?? projects()[0];

  function attach() {
    return op.run(() => attachProjectFromPicker(t().chooseFolder), (project) => {
      if (!project) return;
      props.navigate({ page: "projects", projectId: project.id, scopePath: "" });
      props.refresh();
      props.notify(t().attached, "success");
    });
  }

  return <View style={{ ...viewport, backgroundColor: props.theme.background }}>
    <If when={props.route.tab !== "choose" ? selected() : undefined} else={
      <>
        <Header theme={props.theme} title={t().projects}>
          <Action theme={props.theme} label={t().attach} glyph="plus" pending={op.busy()} run={attach} />
        </Header>
        <View style={{ padding: 20, gap: 12 }}>
          <Feedback theme={props.theme} label={t().loading} pending={targets().pending}
            error={targets().error ?? op.error()} />
          <If when={targets().error}>
            <Action theme={props.theme} label={t().refresh} run={() => props.refresh()} />
          </If>
          <If when={projects().length > 0} else={
            <If when={!targets().pending && !targets().error}><Label theme={props.theme} label={t().noProjects} muted /></If>
          }>
            <View style={{ width: 304, maxWidth: "100%" }}>
              <Choice theme={props.theme} label={t().selectProject} searchable value={null}
                options={projects().map((project) => ({ value: project.id, title: project.name, detail: project.path }))}
                choose={(projectId) => props.navigate({ page: "projects", projectId, scopePath: "" })} />
            </View>
          </If>
        </View>
      </>
    }>
      {(project) => <ProjectDetail screen={props} project={project()} agents={targets().data?.agents ?? []}
        attach={attach} attaching={op.busy()} canSwitch={projects().length > 1}
        listError={targets().error ?? op.error()} />}
    </If>
  </View>;
}

function ProjectDetail(props: {
  screen: ScreenProps; project: Project; agents: Agent[];
  attach: () => Promise<void>; attaching: boolean; canSwitch: boolean; listError?: string;
}) {
  const screen = props.screen;
  const t = () => strings(screen.locale);
  const rescan = signal(0);
  const projectId = computed(() => props.project.id);
  const projectsScan = load(() => {
    void rescan();
    return request("project.scan", { project_id: projectId() });
  });
  const modules = () => projectsScan().data ?? [];
  const module = computed(() => modules().find((item) => item.path === screen.route.scopePath) ?? modules()[0]);
  const agent = computed(() => props.agents.find((item) => item.key === screen.route.agentKey) ?? props.agents[0]);
  const inventory = load(() => {
    void screen.revision;
    const target = agent();
    const scope = module();
    if (target && scope) return request("project.inventory", {
      project_id: props.project.id, scope_path: scope.path, agent_key: target.key,
    });
  });
  const op = operation(screen);
  const overrides = signal<{ projectId: string; path: string; selected: ReadonlySet<string> } | undefined>(undefined);
  const selection = () => {
    const current = overrides();
    const scope = module();
    if (current?.projectId === projectId() && current.path === scope?.path) return current.selected;
    return new Set(scope?.recommendations.map(({ member }) => member.source) ?? []);
  };
  const members = () => (module()?.recommendations ?? [])
    .filter(({ member }) => selection().has(member.source))
    .map(({ member }) => member);
  function pickMembers(path: string, selected: ReadonlySet<string>) {
    overrides.set({ projectId: projectId(), path, selected });
  }
  function rescanProject() {
    const scope = module();
    if (scope) pickMembers(scope.path, selection());
    rescan.update((value) => value + 1);
  }
  function chooseScope(scopePath: string) {
    overrides.set(undefined);
    screen.navigate({ page: "projects", ...target(), scopePath });
  }
  const target = () => ({
    projectId: props.project.id, scopePath: module()?.path ?? "", agentKey: agent()?.key,
  });

  function saveStack() {
    const { projectId, scopePath, agentKey } = target();
    return op.run(() => request("project.save_recommendations", {
      project_id: projectId, scope_path: scopePath, members: members(),
    }), (stack) => {
      screen.navigate({ page: "stacks", stackId: stack.id, source: "Local", projectId, scopePath, agentKey, tab: "deploy" });
      screen.refresh();
      screen.notify(t().saved, "success");
    });
  }

  function detach() {
    const { id, name } = props.project;
    return op.run(async () => {
      if (!await confirm(screen, name, t().detach, t().detachDescription, true)) return false;
      await request("project.detach", { project_id: id });
      return true;
    }, (removed) => {
      if (!removed) return;
      screen.navigate({ page: "projects" });
      screen.refresh();
      screen.notify(t().detached, "success");
    });
  }

  const statusLabel = (installedSlug: string | null) => {
    if (inventory().data?.some((skill) => skill.name === installedSlug && skill.location === "project_deployment")) {
      return t().deployed;
    }
    return installedSlug ? t().inLibrary : t().installFirst;
  };

  return <>
    <Header theme={screen.theme} title={props.project.name}>
      <Action theme={screen.theme} label={t().openLibrary} accessibilityLabel={t().openInLibrary} glyph="arrow-left" ghost
        run={() => screen.navigate({ page: "library", ...target(), scopePath: module()?.path ?? screen.route.scopePath ?? "" })} />
      <Action theme={screen.theme} label={t().rescan} glyph="refresh-cw"
        disabled={op.busy() || projectsScan().pending} run={rescanProject} />
      <ActionMenu theme={screen.theme} label={t().projectActions}>
        <MenuAction theme={screen.theme} label={t().attach} disabled={props.attaching || op.busy()} onSelect={props.attach} />
        <If when={props.canSwitch}>
          <MenuAction theme={screen.theme} label={t().switchProject} disabled={op.busy()}
            onSelect={() => screen.navigate({ page: "projects", tab: "choose" })} />
        </If>
        <MenuAction theme={screen.theme} label={t().detach} danger disabled={props.attaching || op.busy()} onSelect={detach} />
      </ActionMenu>
    </Header>
    <View style={{ ...viewport, flexDirection: "row" }}>
      <Scroll style={viewport} contentStyle={{ padding: 16, gap: 12 }}>
        <View style={{ gap: 8 }}>
          <If when={module()}>{(scope) => <View style={actions}>
            <Label theme={screen.theme} label={t().scope} muted size={screen.theme.fontSize} />
            <If when={modules().length > 1} else={<Label theme={screen.theme} label={scope().path || t().root} lines={1} />}>
              <View style={{ width: 176, maxWidth: "100%", minWidth: 0 }}>
                <Choice theme={screen.theme} label={t().scope} compact searchable menuHeight={280}
                  options={modules().map(({ path }) => ({ value: path, title: path || t().root }))}
                  value={scope().path} disabled={op.busy()} choose={chooseScope} />
              </View>
            </If>
          </View>}</If>
          <View style={actions}>
            <Label theme={screen.theme} label={t().defaultFolder} muted size={screen.theme.fontSize} />
            <View style={{ flex: 1, minWidth: 0, flexBasis: 120 }}>
              <Label theme={screen.theme} label={props.project.path} title={props.project.path} lines={1} />
            </View>
          </View>
        </View>
        <Heading theme={screen.theme} label={t().technologies} size={16} />
        <Feedback theme={screen.theme} label={t().loading} pending={projectsScan().pending}
          error={op.error() ?? projectsScan().error ?? props.listError} />
        <If when={module()}>{(scope) => <>
          <View style={{ gap: 4 }}>
            <If when={scope().evidence.length === 0}><Label theme={screen.theme} label={t().noTechnologies} muted /></If>
            <Each of={scope().evidence} key={(item) => item.technology}>
              {(item) => <View style={{ ...row, minWidth: 0 }}>
                <Label theme={screen.theme} label={item().technology} weight="500" />
                <View style={{ flex: 1, minWidth: 0 }}>
                  <Label theme={screen.theme} label={item().path} title={item().path} muted lines={1} />
                </View>
                <If when={item().added}><Label theme={screen.theme} label={t().newTechnology} color={screen.theme.primary} size={12} /></If>
              </View>}
            </Each>
          </View>
          <View style={{ gap: 8, paddingTop: 12, borderTopWidth: 1, borderColor: screen.theme.border }}>
            <Heading theme={screen.theme} label={t().recommendations} size={16} />
            <If when={scope().recommendations.length === 0}><Label theme={screen.theme} label={t().noRecommendations} muted /></If>
            <Each of={scope().recommendations} key={(item) => item.member.source}>
              {(item) => <View style={{ borderWidth: 1, borderRadius: screen.theme.radius, borderColor: screen.theme.border,
                paddingHorizontal: 12, paddingVertical: 8, gap: 4 }}>
                <View style={row}>
                  <View style={{ flex: 1, minWidth: 0 }}>
                    <Check theme={screen.theme} label={item().member.name} compact checked={selection().has(item().member.source)}
                      disabled={op.busy()} update={(checked) => pickMembers(scope().path, toggle(selection(), item().member.source, checked))} />
                  </View>
                  <Label theme={screen.theme} label={statusLabel(item().installed_slug)} muted size={12} lines={1} />
                </View>
                <Label theme={screen.theme} label={projectRecommendationReason(screen.locale, item().technology, item().purpose)} muted size={12} />
                <Label theme={screen.theme} label={item().member.source} title={item().member.source} muted size={12} lines={1} />
              </View>}
            </Each>
          </View>
        </>}</If>
      </Scroll>

      <View style={{ width: "40%", flexShrink: 0, minWidth: 0, minHeight: 0, overflow: "hidden",
        borderLeftWidth: 1, borderColor: screen.theme.border, backgroundColor: screen.theme.surface }}>
        <View style={{ ...actions, padding: 12, borderBottomWidth: 1, borderColor: screen.theme.border }}>
          <View style={{ flex: 1, minWidth: 0 }}>
            <Label theme={screen.theme} label={t().inventory} weight="600" />
          </View>
          <View style={{ width: 168, maxWidth: "100%", minWidth: 0 }}>
            <Choice theme={screen.theme} label={t().selectAgent} compact
              options={props.agents.map((item) => ({ value: item.key, title: item.display_name }))}
              value={agent()?.key ?? null} disabled={op.busy()}
              choose={(agentKey) => screen.navigate({ page: "projects", ...target(), agentKey })} />
          </View>
        </View>
        <If when={inventory().pending || inventory().error || !agent() || inventory().data?.length === 0}>
          <View style={{ padding: 12, gap: 8 }}>
            <If when={!agent()}><Label theme={screen.theme} label={t().noAgents} muted /></If>
            <Feedback theme={screen.theme} label={t().loading} pending={inventory().pending} error={inventory().error} />
            <If when={inventory().data?.length === 0}><Label theme={screen.theme} label={t().emptyInventory} muted /></If>
          </View>
        </If>
        <List each={inventory().data ?? []} estimatedItemSize={48} style={viewport} keyExtractor={(skill) => skill.path}>
          {(skill) => <View style={{ height: 48, paddingHorizontal: 12, justifyContent: "center", gap: 2,
            borderBottomWidth: 1, borderColor: screen.theme.border }}>
            <View style={row}>
              <View style={{ flex: 1, minWidth: 0 }}>
                <Label theme={screen.theme} label={skill().name} weight="500" lines={1} />
              </View>
              <Label theme={screen.theme} label={t()[skill().location]} muted size={12} lines={1} />
            </View>
            <Label theme={screen.theme} label={skill().path} title={skill().path} muted size={12} lines={1} />
          </View>}
        </List>
      </View>
    </View>
    <Footer theme={screen.theme}>
      <View style={{ flex: 1, minWidth: 0 }}>
        <Label theme={screen.theme} label={projectSelectionCount(screen.locale, members().length)} />
      </View>
      <Action theme={screen.theme} label={t().saveRecommendations} glyph="plus" primary pending={op.busy()}
        disabled={projectsScan().pending || members().length === 0} run={saveStack} />
    </Footer>
  </>;
}
