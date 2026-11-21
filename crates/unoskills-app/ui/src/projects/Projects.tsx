import { computed, List, Scroll, signal, View } from "nsh";
import type { ScreenProps } from "../shared";
import {
  Action, Check, Choice, Feedback, Heading, Label, RowButton, SidebarPane, WorkspacePane,
  actions, confirm, load, operation, padding, toggle, viewport,
} from "./controls";
import { request, type Agent, type Project } from "./host";
import { strings } from "./strings";

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
    return op.run(async () => {
      const path = await request("dialog.folder", { title: t().chooseFolder });
      return path ? request("project.attach", { path }) : null;
    }, (project) => {
      if (!project) return;
      props.navigate({ page: "projects", projectId: project.id, scopePath: "" });
      props.refresh();
      props.notify(t().attached, "success");
    });
  }

  return <WorkspacePane theme={props.theme}>
    <SidebarPane theme={props.theme}>
      <View style={{ padding: 16, gap: 12 }}>
        <Heading theme={props.theme} label={t().projects} level={1} />
        <Action theme={props.theme} label={t().attach} primary disabled={op.busy()} run={attach} />
        <Feedback theme={props.theme} label={t().loading} pending={targets().pending}
          error={targets().error ?? op.error()} />
        <If when={targets().error}>
          <Action theme={props.theme} label={t().refresh} run={() => props.refresh()} />
        </If>
      </View>
      <List each={projects()} keyExtractor={(project) => project.id} estimatedItemSize={50}
        style={viewport} contentStyle={{ paddingHorizontal: 8, paddingBottom: 12 }}>
        {(project) => <RowButton theme={props.theme} title={project().name} detail={project().path}
          selected={selected()?.id === project().id}
          run={() => props.navigate({ page: "projects", projectId: project().id, scopePath: "" })} />}
      </List>
    </SidebarPane>
    <If when={selected()} else={<View style={{ ...viewport, ...padding }}>
      <If when={!targets().pending && !targets().error}><Label theme={props.theme} label={t().noProjects} muted /></If>
    </View>}>
      {(project) => <ProjectDetail screen={props} project={project()} agents={targets().data?.agents ?? []} />}
    </If>
  </WorkspacePane>;
}

function ProjectDetail(props: { screen: ScreenProps; project: Project; agents: Agent[] }) {
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

  return <Scroll style={viewport} contentStyle={padding}>
    <View style={{ gap: 8, minWidth: 0 }}>
      <Heading theme={screen.theme} label={props.project.name} level={1} />
      <Label theme={screen.theme} label={props.project.path} muted />
      <View style={actions}>
        <Action theme={screen.theme} label={t().rescan} disabled={op.busy() || projectsScan().pending}
          run={rescanProject} />
        <Action theme={screen.theme} label={t().openLibrary} disabled={!module()}
          run={() => screen.navigate({ page: "library", ...target() })} />
        <Action theme={screen.theme} label={t().detach} danger disabled={op.busy()} run={detach} />
      </View>
    </View>
    <View style={actions}>
      <Choice fill theme={screen.theme} label={t().scope}
        options={modules().map(({ path }) => ({ value: path, title: path || t().root }))}
        value={module()?.path ?? null} disabled={op.busy()}
        choose={chooseScope} />
      <Choice fill theme={screen.theme} label={t().agent}
        options={props.agents.map((item) => ({ value: item.key, title: item.display_name }))}
        value={agent()?.key ?? null} disabled={op.busy()}
        choose={(agentKey) => screen.navigate({ page: "projects", ...target(), agentKey })} />
    </View>
    <Feedback theme={screen.theme} label={op.busy() ? t().working : t().loading}
      pending={op.busy() || projectsScan().pending} error={op.error() ?? projectsScan().error} />
    <If when={module()}>
      {(scope) => <>
        <View style={{ gap: 8 }}>
          <Heading theme={screen.theme} label={t().technologies} />
          <If when={scope().evidence.length === 0}><Label theme={screen.theme} label={t().noTechnologies} muted /></If>
          <Each of={scope().evidence} key={(item) => item.technology}>
            {(item) => <View style={{ ...actions, minWidth: 0 }}>
              <Label theme={screen.theme} label={item().technology} />
              <Label theme={screen.theme} label={item().path} muted />
              <If when={item().added}><Label theme={screen.theme} label={t().newTechnology} /></If>
            </View>}
          </Each>
        </View>
        <View style={{ gap: 8 }}>
          <Heading theme={screen.theme} label={t().recommendations} />
          <If when={scope().recommendations.length === 0}><Label theme={screen.theme} label={t().noRecommendations} muted /></If>
          <Each of={scope().recommendations} key={(item) => item.member.source}>
            {(item) => <View style={{ borderTopWidth: 1, borderColor: screen.theme.border, paddingTop: 8, gap: 4 }}>
              <Check theme={screen.theme} label={item().member.name} checked={selection().has(item().member.source)}
                disabled={op.busy()} update={(checked) => pickMembers(scope().path, toggle(selection(), item().member.source, checked))} />
              <Label theme={screen.theme} label={`${item().technology} · ${t()[item().purpose]} · ${
                inventory().data?.some((skill) => skill.name === item().installed_slug && skill.location === "project_deployment")
                  ? t().deployed : item().installed_slug ? t().inLibrary : t().missing
              }`} muted />
              <Label theme={screen.theme} label={item().member.source} muted />
            </View>}
          </Each>
          <If when={scope().recommendations.length > 0}>
            <View style={actions}>
              <Action theme={screen.theme} label={t().saveRecommendations} primary
                disabled={op.busy() || members().length === 0} run={saveStack} />
            </View>
          </If>
        </View>
        <View style={{ gap: 8 }}>
          <Heading theme={screen.theme} label={t().inventory} />
          <If when={!agent()}><Label theme={screen.theme} label={t().noAgents} muted /></If>
          <Feedback theme={screen.theme} label={t().loading} pending={inventory().pending} error={inventory().error} />
          <If when={inventory().data?.length === 0}><Label theme={screen.theme} label={t().emptyInventory} muted /></If>
          <List each={inventory().data ?? []} viewport="parent" estimatedItemSize={50}
            keyExtractor={(skill) => skill.path}>
            {(skill) => <View style={{ borderTopWidth: 1, borderColor: screen.theme.border, gap: 2, paddingVertical: 8 }}>
              <Label theme={screen.theme} label={`${skill().name} · ${t()[skill().location]}`} />
              <Label theme={screen.theme} label={skill().path} muted />
            </View>}
          </List>
        </View>
      </>}
    </If>
  </Scroll>;
}
