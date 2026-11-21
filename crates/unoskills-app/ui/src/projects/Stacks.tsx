import { effect, Clipboard, keymap, List, Scroll, signal, View, type InputRef } from "nsh";
import type { ScreenProps } from "../shared";
import {
  Action, Choice, Feedback, Field, Heading, Label, RowButton, SidebarPane, WorkspacePane,
  actions, confirm, load, operation, padding, viewport,
} from "./controls";
import { StackDeploy } from "./deploy";
import { StackEditor, StackImport } from "./editor";
import { InstallReport } from "./reports";
import { request, stackRef, type InstallResult, type Stack, type StackSource } from "./host";
import { strings } from "./strings";

const sources = ["all", "Local", "Featured", "Community"] as const;
type Filter = typeof sources[number];

export function Stacks(props: ScreenProps) {
  const t = () => strings(props.locale);
  const routeSource = props.route.source as Filter | undefined;
  const filter = signal<Filter>(routeSource && sources.includes(routeSource) ? routeSource : "all");
  const query = signal("");
  const tag = signal("");
  const reloadCommunity = signal(0);
  const catalog = load(() => {
    void props.revision;
    return request("stack.list", null);
  }, true);
  const community = load(() => {
    void reloadCommunity();
    return request("stack.community", null);
  }, true);
  const local = () => catalog().data?.local ?? [];
  const featured = () => catalog().data?.featured ?? [];
  const remote = () => community().data ?? [];
  const all = () => [...featured(), ...remote().filter((stack) => !featured().some((builtIn) => builtIn.id === stack.id))];
  const stacks = () => {
    switch (filter()) {
      case "Local": return local();
      case "Featured": return featured();
      case "Community": return remote();
      case "all": return all();
    }
  };
  const tags = () => Array.from(new Set(stacks().flatMap((stack) => stack.tags))).sort();
  const filtered = () => {
    const term = query().trim().toLocaleLowerCase();
    const activeTag = tag();
    return stacks().filter((stack) =>
      (!activeTag || stack.tags.includes(activeTag)) &&
      (!term || [stack.name, stack.id, stack.author, stack.description, ...stack.tags].some((field) => field.toLocaleLowerCase().includes(term)))
    );
  };
  const selected = () => {
    const id = props.route.stackId;
    const source = props.route.source;
    if (!id) return (props.route.source === "Local" ? local() : filtered())[0];
    const candidates = [...local(), ...featured(), ...remote()];
    return candidates.find((stack) => stack.id === id && (!source || source === stack.source));
  };
  const mode = () => props.route.tab;
  const isBrowse = () => !["create", "edit", "import", "deploy"].includes(mode() ?? "");
  let search: InputRef | undefined;
  effect(() => {
    if (!props.active) return;
    return keymap.bind("mod+k", () => search?.focus());
  });

  function open(stack: Stack, tab?: string) {
    props.navigate({
      page: "stacks", stackId: stack.id, source: stack.source, tab,
      projectId: props.route.projectId, scopePath: props.route.scopePath, agentKey: props.route.agentKey,
    });
  }

  function openLocal(stack: Stack) {
    filter.set("Local");
    open({ ...stack, source: "Local" });
    props.refresh();
  }

  function changeFilter(value: string) {
    filter.set(value as Filter);
    tag.set("");
    props.navigate({ page: "stacks" });
  }

  return <WorkspacePane theme={props.theme}>
    <SidebarPane theme={props.theme}>
      <View style={{ padding: 16, gap: 8 }}>
        <Heading theme={props.theme} label={t().stacks} level={1} />
        <View style={actions}>
          <Action theme={props.theme} label={t().create} primary
            run={() => props.navigate({ page: "stacks", tab: "create" })} />
          <Action theme={props.theme} label={t().import}
            run={() => props.navigate({ page: "stacks", tab: "import" })} />
        </View>
        <Choice theme={props.theme} label={t().catalogSource}
          options={sources.map((source) => ({ value: source, title: t()[source] }))}
          value={filter()} choose={changeFilter} />
        <Field theme={props.theme} label={t().searchStacks} search value={query()}
          update={query.set} receiveRef={(element) => { search = element; }}
          submit={() => { const stack = filtered()[0]; if (stack) open(stack); }} />
        <If when={tags().length > 0}>
          <Choice theme={props.theme} label={t().filterTag}
            options={[{ value: "", title: t().allTags }, ...tags().map((value) => ({ value, title: value }))]}
            value={tag()} choose={tag.set} />
        </If>
        <Feedback theme={props.theme} label={t().loading} pending={catalog().pending}
          error={catalog().error} />
        <If when={filter() === "all" || filter() === "Community"}>
          <Feedback theme={props.theme} label={t().Community} pending={community().pending} error={community().error} />
          <Action theme={props.theme} label={t().refresh} disabled={community().pending}
            run={() => reloadCommunity.update((value) => value + 1)} />
        </If>
      </View>
      <If when={filtered().length === 0 && !catalog().pending && !community().pending}>
        <View style={{ paddingHorizontal: 16 }}><Label theme={props.theme} label={t().noStacks} muted /></View>
      </If>
      <List each={filtered()} keyExtractor={(stack) => `${stack.source}:${stack.id}`} estimatedItemSize={50}
        style={viewport} contentStyle={{ paddingHorizontal: 8, paddingBottom: 12 }}>
        {(stack) => <RowButton theme={props.theme} title={stack().name}
          detail={stack().author ? `${t()[stack().source]} · ${stack().author}` : t()[stack().source]}
          selected={isBrowse() && selected()?.id === stack().id && selected()?.source === stack().source}
          run={() => open(stack())} />}
      </List>
    </SidebarPane>
    <View style={viewport}>
      <If when={mode() === "create"}><StackEditor screen={props} saved={openLocal} /></If>
      <If when={mode() === "import"}><StackImport screen={props} saved={openLocal} /></If>
      <If when={selected()}>
        {(stack) => <>
          <If when={mode() === "edit" && stack().source === "Local"}>
            <StackEditor screen={props} stack={stack()} saved={openLocal} />
          </If>
          <If when={mode() === "deploy" && stack().source === "Local"}>
            <StackDeploy screen={props} stack={stack()} back={() => open(stack())} />
          </If>
          <If when={isBrowse()}>
            <StackDetail screen={props} stack={stack()} local={local()} open={open} />
          </If>
        </>}
      </If>
      <If when={!selected() && !["create", "import"].includes(mode() ?? "") && !catalog().pending}>
        <View style={padding}><Heading theme={props.theme} label={t().selectStack} /></View>
      </If>
    </View>
  </WorkspacePane>;
}

function StackDetail(props: {
  screen: ScreenProps; stack: Stack; local: Stack[];
  open: (stack: Stack, tab?: string) => void;
}) {
  const screen = props.screen;
  const t = () => strings(screen.locale);
  const op = operation(screen);
  const members = load(() => {
    void screen.revision;
    return request("stack.members", stackRef(props.stack));
  });
  const outcome = signal<{ id: string; source: StackSource; result: InstallResult } | undefined>(undefined);
  const installedResult = () => {
    const last = outcome();
    return last?.id === props.stack.id && last.source === props.stack.source ? last.result : undefined;
  };
  const isLocal = () => props.stack.source === "Local";
  const hasLocal = () => props.local.some((stack) => stack.id === props.stack.id);
  const missing = () => members().data?.filter((status) => !status.installed).length ?? 0;

  function install() {
    const stack = props.stack;
    return op.run(() => request("stack.install", stackRef(stack)), (result) => {
      outcome.set({ id: stack.id, source: stack.source, result });
      screen.refresh();
      if (result.failed.length === 0) screen.notify(result.installed.length > 0 ? t().installed : t().saved, "success");
    });
  }

  function exportToml() {
    const stack = props.stack;
    return op.run(async () => {
      const path = await request("dialog.save", { title: t().export, name: `${stack.id.replace(/[^\p{L}\p{N}_.-]/gu, "-")}.unoskills-stack.toml`, extensions: ["toml"] });
      if (!path) return false;
      await request("stack.export", { ...stackRef(stack), path });
      return true;
    }, (saved) => { if (saved) screen.notify(t().exported, "success"); });
  }

  function copyToml() {
    const ref = stackRef(props.stack);
    return op.run(async () => Clipboard.writeText(await request("stack.export", ref)),
      () => screen.notify(t().copied, "success"));
  }

  function share() {
    const ref = stackRef(props.stack);
    return op.run(async () => {
      if (!await confirm(screen, t().share, t().share, t().shareDescription)) return;
      await Clipboard.writeText(await request("stack.share_gist", ref));
      return true;
    }, (shared) => { if (shared) screen.notify(t().copied, "success"); });
  }

  function remove() {
    const { id, name } = props.stack;
    return op.run(async () => {
      if (!await confirm(screen, name, t().delete, undefined, true)) return false;
      await request("stack.delete", { stack_id: id });
      return true;
    }, (removed) => {
      if (!removed) return;
      screen.navigate({ page: "stacks", source: "Local" });
      screen.refresh();
      screen.notify(t().deleted, "success");
    });
  }

  return <Scroll style={viewport} contentStyle={padding}>
    <View style={{ gap: 8, minWidth: 0 }}>
      <Heading theme={screen.theme} label={props.stack.name} level={1} />
      <Label theme={screen.theme} label={[t()[props.stack.source], props.stack.author, props.stack.verified ? t().verified : ""].filter(Boolean).join(" · ")} muted />
      <If when={props.stack.description}><Label theme={screen.theme} label={props.stack.description} /></If>
      <If when={props.stack.tags.length > 0}><Label theme={screen.theme} label={props.stack.tags.join(", ")} muted /></If>
      <If when={props.stack.recommended_agents.length > 0}>
        <Label theme={screen.theme} muted label={`${t().recommendedAgents} · ${props.stack.recommended_agents.join(", ")}`} />
      </If>
    </View>
    <View style={actions}>
      <If when={!members().pending && !members().error && (missing() > 0 || !hasLocal())}>
        <Action theme={screen.theme} label={missing() > 0 ? t().installMissing : t().saveStack}
          primary disabled={op.busy()} run={install} />
      </If>
      <If when={isLocal()}>
        <Action theme={screen.theme} label={t().deploy} primary={missing() === 0}
          disabled={op.busy() || props.stack.skills.length === 0} run={() => props.open(props.stack, "deploy")} />
        <Action theme={screen.theme} label={t().edit} disabled={op.busy()} run={() => props.open(props.stack, "edit")} />
        <Action theme={screen.theme} label={t().delete} danger disabled={op.busy()} run={remove} />
      </If>
      <If when={!isLocal() && hasLocal()}>
        <Action theme={screen.theme} label={t().Local} disabled={op.busy()} run={() => {
          const stack = props.local.find(({ id }) => id === props.stack.id);
          if (stack) props.open(stack);
        }} />
      </If>
      <Action theme={screen.theme} label={t().export} disabled={op.busy()} run={exportToml} />
      <Action theme={screen.theme} label={t().copyToml} disabled={op.busy()} run={copyToml} />
      <Action theme={screen.theme} label={t().share} disabled={op.busy()} run={share} />
    </View>
    <Feedback theme={screen.theme} label={op.busy() ? t().working : t().loading}
      pending={members().pending || op.busy()} error={op.error() ?? members().error} />
    <If when={installedResult()}>{(result) => <InstallReport screen={screen} result={result()} />}</If>
    <Heading theme={screen.theme} label={`${t().includedSkills} (${props.stack.skills.length})`} />
    <List each={members().data ?? []} viewport="parent" estimatedItemSize={58}>
      {(status) => <View style={{ ...actions, flexWrap: "nowrap", borderTopWidth: 1, borderColor: screen.theme.border, paddingVertical: 8 }}>
        <View style={{ flex: 1, minWidth: 0, gap: 2 }}>
          <Label theme={screen.theme} label={status().member.name} />
          <Label theme={screen.theme} label={status().member.source} muted />
          <If when={status().member.note}><Label theme={screen.theme} label={status().member.note} muted /></If>
        </View>
        <If when={status().installed} else={<Action theme={screen.theme} label={t().install}
          disabled={op.busy()} run={() => op.run(() => request("stack.install_member", {
            ...stackRef(props.stack), member_name: status().member.name, member_source: status().member.source,
          }), () => {
            outcome.set(undefined);
            screen.refresh();
            screen.notify(t().installed, "success");
          })} />}>
          {(skill) => <Action theme={screen.theme} label={status().source_conflict ? t().sourceConflict : t().inLibrary}
            danger={status().source_conflict} run={() => screen.navigate({ page: "library", skillId: skill().id })} />}
        </If>
      </View>}
    </List>
  </Scroll>;
}
