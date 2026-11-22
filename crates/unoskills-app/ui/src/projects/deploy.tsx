import { computed, Scroll, signal, View, windowSize } from "nsh";
import { Dialog } from "nsh/base";
import { dialogAnimation, popupSurface } from "../components";
import type { ScreenProps } from "../shared";
import { Action, Banner, Check, Choice, Feedback, Heading, Label, actions, row } from "./controls";
import { load, operation } from "./state";
import { attachProjectFromPicker, request, stackRef, type DeployResult, type InstallResult, type ProjectScope, type Stack } from "./host";
import { Failures, InstallReport } from "./reports";
import { deploymentProgress, deploymentResult, deployOutcome, strings } from "./strings";

export function StackDeploy(props: { screen: ScreenProps; stack: Stack; back: () => void }) {
  const screen = props.screen;
  const t = () => strings(screen.locale);
  const op = operation(screen);
  const reload = signal(0);
  const initialScope = screen.route.projectId === undefined ? undefined : {
    projectId: screen.route.projectId, path: screen.route.scopePath,
  };
  const projects = load(() => {
    void screen.revision;
    return request("project.list", null);
  }, true);
  const project = computed(() => {
    const options = projects().data?.projects ?? [];
    const id = screen.route.projectId;
    return options.find((project) => project.id === id);
  });
  const scopes = load(() => {
    const id = project()?.id;
    if (id) return request("project.scopes", { project_id: id });
  });
  const scopeOptions = () => Array.from(new Set(["", ...(scopes().data ?? [])]));
  const agent = computed(() => {
    const agents = projects().data?.agents ?? [];
    const key = screen.route.agentKey;
    return key === undefined && agents.length === 1 ? agents[0] : agents.find((agent) => agent.key === key);
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
    return op.run(() => attachProjectFromPicker(t().chooseFolder), (project) => {
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
    const reviewed = preview().data;
    if (!scope || !reviewed || ready() === 0 || preview().pending) return;
    const args = { ...stackRef(props.stack), ...scope, reviewed, include_global: includeGlobal() };
    return op.run(() => request("stack.confirm_deploy", args), (deployed) => {
      if (deployed) {
        result.set(deployed);
        screen.refresh();
        if (deployed.failed.length === 0 && deployed.skipped.length === 0) {
          screen.notify(deploymentResult(screen.locale, deployed), "success");
        }
      }
    });
  }

  const directory = () => {
    const current = project();
    const scope = selectedScope();
    if (!current || scope === undefined) return undefined;
    const separator = current.path.includes("\\") ? "\\" : "/";
    return scope ? `${current.path.replace(/[\\/]$/, "")}${separator}${scope}` : current.path;
  };
  const status = () => preview().data;
  const progress = () => {
    const checked = status();
    if (checked) return deploymentProgress(screen.locale, checked, props.stack.skills.length, ready());
    return preview().pending ? t().checking : "";
  };

  return <Dialog open={screen.active} animation={dialogAnimation()}
    accessibilityLabel={t().deploy} closeOnBackdropPress={false} closeOnEscape={!op.busy()}
    onOpenChange={({ open }) => { if (!open && !op.busy()) props.back(); }}
    backdropStyle={{ backgroundColor: screen.theme.modalBackdrop }}
    style={{ alignItems: "center", justifyContent: "flex-start", paddingTop: 76, paddingHorizontal: 16, paddingBottom: 16 }}>
    <View style={{ width: 532, maxWidth: "100%", minHeight: 0, gap: 8, padding: 16,
      ...popupSurface(screen.theme), borderRadius: screen.theme.panel.radius }}>
      <View style={row}>
        <View style={{ flex: 1, minWidth: 0 }}>
          <Heading theme={screen.theme} label={t().deploy} size={16} lines={1} color={screen.theme.popup.foreground} />
        </View>
        <If when={!op.busy()}>
          <Action theme={screen.theme} label={t().close} iconOnly compact ghost glyph="x" run={props.back} />
        </If>
      </View>
      <Scroll style={{ minWidth: 0, minHeight: 0, maxHeight: windowSize().height - 210 }} contentStyle={{ gap: 12 }}>
        <Label theme={screen.theme} label={props.stack.name} color={screen.theme.popup.foreground} weight="600" lines={1} />
        <View style={row}>
          <View style={{ flex: 1, minWidth: 0 }}>
            <Choice theme={screen.theme} label={t().selectProject} searchable
              options={(projects().data?.projects ?? []).map((project) => ({ value: project.id, title: project.name, detail: project.path }))}
              value={project()?.id ?? null} disabled={op.busy()} choose={(id) => destination(id, "", agent()?.key)} />
          </View>
          <Action theme={screen.theme} label={t().attach} glyph="folder-open" disabled={op.busy()} run={attach} />
        </View>
        <If when={initialScope?.projectId === project()?.id && initialScope?.path !== undefined}>
          <View style={row}>
            <Label theme={screen.theme} label={t().scope} muted />
            <Label theme={screen.theme} label={initialScope?.path || t().root} muted lines={1} />
          </View>
        </If>
        <If when={directory()}>
          {(path) => <View style={{ gap: 4 }}>
            <View style={actions}>
              <View style={{ flex: 1, minWidth: 0 }}>
                <Label theme={screen.theme} label={t().deploymentFolder} weight="500" />
              </View>
              <If when={scopeOptions().length > 1}>
                <View style={{ width: 200, maxWidth: "100%", minWidth: 0 }}>
                  <Choice theme={screen.theme} label={t().deploymentFolder} compact menuHeight={240}
                    options={scopeOptions().map((value) => ({ value, title: value || t().root }))}
                    value={selectedScope() ?? null} disabled={op.busy()}
                    choose={(scope) => { const selected = project(); if (selected) destination(selected.id, scope, agent()?.key); }} />
                </View>
              </If>
            </View>
            <View style={row}>
              <View style={{ flex: 1, minWidth: 0 }}>
                <Label theme={screen.theme} label={path()} title={path()} size={12} muted lines={1} />
              </View>
              <If when={target()}>
                {(scope) => <Action theme={screen.theme} label={t().openProject} ghost compact iconOnly glyph="external-link" disabled={op.busy()}
                  run={() => screen.navigate({ page: "projects", projectId: scope().project_id,
                    scopePath: scope().scope_path, agentKey: scope().agent_key })} />}
              </If>
            </View>
          </View>}
        </If>
        <Choice theme={screen.theme} label={t().selectAgent}
          options={(projects().data?.agents ?? []).map((agent) => ({ value: agent.key, title: agent.display_name }))}
          value={agent()?.key ?? null} disabled={op.busy()}
          choose={(key) => {
            const selected = project();
            if (selected) destination(selected.id, selectedScope() ?? "", key);
            else screen.navigate({ ...screen.route, agentKey: key });
          }} />
        <If when={!projects().pending && !projects().error}>
          <If when={projects().data?.projects.length === 0}><Label theme={screen.theme} label={t().noProjects} muted /></If>
          <If when={projects().data?.agents.length === 0}><Label theme={screen.theme} label={t().noAgents} muted /></If>
        </If>
        <If when={scopes().data && selectedScope() === undefined}>
          <Banner theme={screen.theme} label={t().scopeUnavailable} tone="warning" />
        </If>
        <Feedback theme={screen.theme} label={t().loading}
          pending={projects().pending || scopes().pending || members().pending}
          error={op.error() ?? projects().error ?? scopes().error ?? preview().error ?? members().error} />
        <If when={target()}>
          <View style={row}>
            <View style={{ flex: 1, minWidth: 0 }}>
              <Label theme={screen.theme} label={progress()} weight="500" />
            </View>
            <Action theme={screen.theme} label={t().refreshDeployment} ghost iconOnly glyph="refresh-cw"
              pending={preview().pending} disabled={op.busy()}
              run={() => { result.set(undefined); reload.update((value) => value + 1); }} />
          </View>
        </If>
        <If when={installed()}>{(summary) => <InstallReport screen={screen} result={summary()} />}</If>
        <If when={result()}>
          {(summary) => <View style={{ gap: 4 }}>
            <Banner theme={screen.theme} label={deploymentResult(screen.locale, summary())}
              tone={summary().failed.length > 0 ? "error" : summary().skipped.length > 0 ? "warning" : "success"} />
            <Failures screen={screen} failures={summary().failed.map(([name, error]) =>
              [name, error.includes("TARGET_CONFLICT") ? t().targetConflict : error])} />
            <Each of={summary().skipped}>
              {(item) => <Label theme={screen.theme} label={`${item()[0]} · ${t().deployChanged}`} />}
            </Each>
          </View>}
        </If>
        <If when={status()}>{(checked) => <>
          <If when={checked().global_available > 0}>
            <Check theme={screen.theme} label={t().includeGlobal} checked={includeGlobal()}
              disabled={op.busy()} update={includeGlobal.set} />
          </If>
          <View>
            <Each of={checked().rows} key={(item) => String(item.index)}>
              {(item) => <View style={{ gap: 2, paddingVertical: 8, borderTopWidth: 1, borderColor: screen.theme.border }}>
                <Label theme={screen.theme} label={item().name} weight="600" />
                <If when={item().target}>
                  {(path) => <Label theme={screen.theme} label={path()} title={path()} muted lines={1} />}
                </If>
                <Label theme={screen.theme} label={deployOutcome(screen.locale, item().outcome, includeGlobal())} />
              </View>}
            </Each>
          </View>
        </>}</If>
      </Scroll>
      <View style={{ ...actions, justifyContent: "flex-end" }}>
        <If when={missing()}>
          <Action theme={screen.theme} label={t().installPreview} run={install} disabled={op.busy()} />
        </If>
        <Action theme={screen.theme} label={result() ? t().close : t().cancel} disabled={op.busy()} run={props.back} />
        <Action theme={screen.theme} label={t().confirmDeploy} primary
          pending={op.busy()} disabled={preview().pending || ready() === 0} run={deploy} />
      </View>
    </View>
  </Dialog>;
}
