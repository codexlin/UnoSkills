import { computed, Scroll, signal, View } from "nsh";
import type { ScreenProps } from "../shared";
import { Action, Check, Choice, Feedback, Heading, Label, actions, confirm, load, operation, padding, viewport } from "./controls";
import { request, stackRef, type DeployResult, type DeployStatus, type InstallResult, type ProjectScope, type Stack } from "./host";
import { Failures, InstallReport } from "./reports";
import { strings } from "./strings";

export function StackDeploy(props: { screen: ScreenProps; stack: Stack; back: () => void }) {
  const screen = props.screen;
  const t = () => strings(screen.locale);
  const op = operation(screen);
  const reload = signal(0);
  const projects = load(() => {
    void screen.revision;
    return request("project.list", null);
  }, true);
  const project = computed(() => {
    const options = projects().data?.projects ?? [];
    const id = screen.route.projectId;
    return id === undefined ? options[0] : options.find((project) => project.id === id);
  });
  const scopes = load(() => {
    const id = project()?.id;
    if (id) return request("project.scopes", { project_id: id });
  });
  const scopeOptions = () => Array.from(new Set(["", ...(scopes().data ?? [])]));
  const agent = computed(() => {
    const agents = projects().data?.agents ?? [];
    const key = screen.route.agentKey;
    return key === undefined ? agents[0] : agents.find((agent) => agent.key === key);
  });
  const selectedScope = () => {
    const path = screen.route.scopePath ?? "";
    return scopes().data && scopeOptions().includes(path) ? path : undefined;
  };
  const target = computed<ProjectScope | undefined>(() => {
    const current = project();
    const currentAgent = agent();
    const path = selectedScope();
    if (!current || !currentAgent || path === undefined) return undefined;
    return { project_id: current.id, scope_path: path, agent_key: currentAgent.key };
  });
  const includeGlobal = signal(false);
  const result = signal<DeployResult | undefined>(undefined);
  const installed = signal<InstallResult | undefined>(undefined);
  const preview = load(() => {
    void reload();
    void screen.revision;
    const scope = target();
    if (scope) return request("stack.preview_deploy", { ...stackRef(props.stack), ...scope });
  });
  const members = load(() => {
    void screen.revision;
    return request("stack.members", stackRef(props.stack));
  });
  const missing = () => members().data?.some((member) => !member.installed) ?? false;
  const ready = () => {
    const status = preview().data;
    return status ? status.pending + (includeGlobal() ? status.global_available : 0) : 0;
  };

  function destination(projectId: string, scopePath: string, agentKey?: string) {
    result.set(undefined);
    includeGlobal.set(false);
    screen.navigate({ page: "stacks", stackId: props.stack.id, source: "Local", tab: "deploy", projectId, scopePath, agentKey });
  }

  function attach() {
    return op.run(async () => {
      const path = await request("dialog.folder", { title: t().chooseFolder });
      return path ? request("project.attach", { path }) : null;
    }, (project) => {
      if (!project) return;
      destination(project.id, "", agent()?.key);
      screen.refresh();
      screen.notify(t().attached, "success");
    });
  }

  function install() {
    const ref = stackRef(props.stack);
    return op.run(() => request("stack.install", ref), (response) => {
      installed.set(response);
      screen.refresh();
    });
  }

  function deploy() {
    const scope = target();
    if (!scope || ready() === 0 || preview().pending) return;
    const args = { ...stackRef(props.stack), ...scope, include_global: includeGlobal() };
    const summary = [
      [project()?.name, scope.scope_path || t().root, agent()?.display_name].join(" · "),
      `${t().pending} ${ready()}`,
      t().conflictHelp,
    ].join("\n");
    return op.run(async () => {
      if (!await confirm(screen, t().previewDeploy, t().confirmDeploy, summary)) return;
      return request("stack.confirm_deploy", args);
    }, (deployed) => {
      if (deployed) {
        result.set(deployed);
        screen.refresh();
        if (deployed.failed.length === 0) screen.notify(`${t().deployed} ${deployed.deployed}`, "success");
      }
    });
  }

  return <Scroll style={viewport} contentStyle={padding}>
    <View style={{ gap: 4 }}>
      <Heading theme={screen.theme} label={t().previewDeploy} level={1} />
      <Label theme={screen.theme} label={props.stack.name} />
    </View>
    <View style={actions}>
      <Action theme={screen.theme} label={t().back} disabled={op.busy()} run={props.back} />
      <Action theme={screen.theme} label={t().attach} disabled={op.busy()} run={attach} />
      <If when={target()}>
        {(scope) => <Action theme={screen.theme} label={t().openProject} disabled={op.busy()} run={() =>
          screen.navigate({ page: "projects", projectId: scope().project_id, scopePath: scope().scope_path, agentKey: scope().agent_key })} />}
      </If>
    </View>
    <View style={actions}>
      <Choice fill theme={screen.theme} label={t().selectProject}
        options={(projects().data?.projects ?? []).map((project) => ({ value: project.id, title: project.name }))}
        value={project()?.id ?? null} disabled={op.busy()} choose={(id) => destination(id, "", agent()?.key)} />
      <Choice fill theme={screen.theme} label={t().scope}
        options={scopes().data ? scopeOptions().map((path) => ({ value: path, title: path || t().root })) : []}
        value={selectedScope() ?? null} disabled={op.busy() || !project()}
        choose={(scope) => { const selected = project(); if (selected) destination(selected.id, scope, agent()?.key); }} />
      <Choice fill theme={screen.theme} label={t().agent}
        options={(projects().data?.agents ?? []).map((agent) => ({ value: agent.key, title: agent.display_name }))}
        value={agent()?.key ?? null} disabled={op.busy()}
        choose={(key) => { const selected = project(); if (selected) destination(selected.id, selectedScope() ?? "", key); }} />
    </View>
    <If when={project()}>{(selected) => <Label theme={screen.theme} label={selected().path} muted />}</If>
    <If when={!projects().pending && !projects().error}>
      <If when={projects().data?.projects.length === 0} else={
        <If when={!project()}><Label theme={screen.theme} label={t().projectUnavailable} danger /></If>
      }><Label theme={screen.theme} label={t().noProjects} muted /></If>
      <If when={projects().data?.agents.length === 0} else={
        <If when={!agent()}><Label theme={screen.theme} label={t().agentUnavailable} danger /></If>
      }><Label theme={screen.theme} label={t().noAgents} muted /></If>
    </If>
    <If when={scopes().data && selectedScope() === undefined}>
      <Label theme={screen.theme} label={t().scopeUnavailable} danger />
    </If>
    <Feedback theme={screen.theme} label={op.busy() ? t().working : t().loading}
      pending={op.busy() || projects().pending || scopes().pending || preview().pending || members().pending}
      error={op.error() ?? projects().error ?? scopes().error ?? preview().error ?? members().error} />
    <If when={preview().data}>
      {(status) => <View style={{ gap: 8, padding: 12, borderWidth: 1, borderRadius: 6,
        backgroundColor: screen.theme.surface, borderColor: screen.theme.border }}>
        <DeployCounts screen={screen} status={status()} />
        <If when={status().global_available > 0}>
          <Check theme={screen.theme} label={t().includeGlobal} checked={includeGlobal()}
            disabled={op.busy()} update={includeGlobal.set} />
        </If>
        <If when={status().conflicts > 0 || status().project_local > 0}>
          <Label theme={screen.theme} label={t().conflictHelp} muted />
        </If>
      </View>}
    </If>
    <View style={actions}>
      <If when={missing()}>
        <Action theme={screen.theme} label={t().installPreview} run={install} disabled={op.busy()} />
      </If>
      <Action theme={screen.theme} label={t().confirmDeploy} primary
        disabled={op.busy() || preview().pending || ready() === 0} run={deploy} />
      <Action theme={screen.theme} label={t().refresh} disabled={op.busy() || preview().pending}
        run={() => reload.update((value) => value + 1)} />
    </View>
    <If when={installed()}>{(summary) => <InstallReport screen={screen} result={summary()} />}</If>
    <If when={result()}>
      {(summary) => <View style={{ gap: 4 }}>
        <Label theme={screen.theme} label={`${t().deployed} ${summary().deployed} · ${t().alreadyDeployed} ${summary().already_deployed} · ${t().failures} ${summary().failed.length}`} />
        <Failures screen={screen} failures={summary().failed} />
      </View>}
    </If>
  </Scroll>;
}

function DeployCounts(props: { screen: ScreenProps; status: DeployStatus }) {
  const t = () => strings(props.screen.locale);
  const rows = () => [
    [t().pending, props.status.pending],
    [t().deployed, props.status.deployed],
    [t().global, props.status.global_available],
    [t().project_local, props.status.project_local],
    [t().unavailable, props.status.unavailable],
    [t().conflicts, props.status.conflicts],
  ] as const;
  return <View style={{ ...actions, rowGap: 8, columnGap: 16 }}>
    <Each of={rows()} key={(row) => row[0]}>
      {(row) => <Label theme={props.screen.theme} label={`${row()[0]} ${row()[1]}`} />}
    </Each>
  </View>;
}
