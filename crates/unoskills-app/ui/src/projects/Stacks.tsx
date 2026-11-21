import { Clipboard, cleanup, computed, effect, keymap, List, Scroll, signal, Text, View, type InputRef, type JSX } from "nsh";
import { Button, MenuSeparator, ResizablePanel, ResizablePanelGroup, Tabs } from "nsh/base";
import { Icon, TabItem, focusOutline, interactionStyles } from "../controls";
import type { IconName } from "../icons";
import type { ScreenProps, Theme } from "../shared";
import {
  Action, Choice, Feedback, Field, Footer, Header, Heading, Label, MenuAction, OverflowMenu,
  confirm, load, operation, padding, row, viewport,
} from "./controls";
import { StackDeploy } from "./deploy";
import { StackEditor, StackImport } from "./editor";
import { InstallReport } from "./reports";
import { request, stackRef, type InstallResult, type Stack, type StackSource } from "./host";
import { stackMemberRows } from "./members";
import { installCounts, stackSize, strings } from "./strings";

const sourceFilters = ["all", "Featured", "Community"] as const;
type SourceFilter = typeof sourceFilters[number];

export function Stacks(props: ScreenProps) {
  const t = () => strings(props.locale);
  const tab = signal<"Local" | "catalog">("Local");
  const source = signal<SourceFilter>("all");
  const query = signal("");
  const tag = signal("");
  const width = signal(304);
  const reloadCommunity = signal(0);
  const mode = () => props.route.tab;
  const editorOpen = () => ["create", "edit", "import"].includes(mode() ?? "");
  const catalogRoute = computed(() => props.route.source);
  effect(() => {
    const catalogSource = catalogRoute();
    if (catalogSource === "Local") {
      tab.set("Local");
    } else if (catalogSource && sourceFilters.includes(catalogSource as SourceFilter)) {
      tab.set("catalog");
      if (!props.route.stackId) {
        source.set(catalogSource as SourceFilter);
        tag.set("");
      }
    }
  });
  const catalog = load(() => {
    void props.revision;
    return request("stack.list", null);
  }, true);
  const community = load(() => {
    void reloadCommunity();
    return request("stack.community", null);
  }, true);
  const local = () => catalog().data?.local ?? [];
  const available = computed(() => {
    const builtIn = catalog().data?.featured ?? [];
    const remote = community().data ?? [];
    if (source() === "Featured") return builtIn;
    if (source() === "Community") return remote;
    const merged = new Map<string, Stack>();
    for (const stack of [...builtIn, ...remote]) {
      if (!merged.has(stack.id)) merged.set(stack.id, stack);
    }
    return [...merged.values()];
  });
  const tags = () => Array.from(new Set(available().flatMap((stack) => stack.tags))).sort();
  const filtered = computed(() => {
    if (tab() === "Local") return local();
    const term = query().trim().toLocaleLowerCase();
    const activeTag = tag().toLocaleLowerCase();
    return available().filter((stack) =>
      (!activeTag || stack.tags.some((value) => value.toLocaleLowerCase() === activeTag)) &&
      (!term || [stack.name, stack.id, stack.author, stack.description, ...stack.tags].some((field) => field.toLocaleLowerCase().includes(term)))
    );
  });
  const selected = computed(() => {
    const id = props.route.stackId;
    const origin = props.route.source;
    const matching = (stack: Stack) => stack.id === id && (!origin || origin === stack.source);
    if (mode() === "edit" || mode() === "deploy") return local().find(matching);
    return filtered().find(matching) ?? filtered()[0];
  });
  let search: InputRef | undefined;
  let pendingSearchFocus = false;

  function registerSearch(element: InputRef) {
    search = element;
    if (pendingSearchFocus) {
      pendingSearchFocus = false;
      queueMicrotask(() => { if (props.active && tab() === "catalog") search?.focus(); });
    }
  }

  function pickTab(next: "Local" | "catalog") {
    tab.set(next);
    props.navigate({ page: "stacks", source: next === "Local" ? "Local" : source() });
  }

  effect(() => {
    if (!props.active || editorOpen() || mode() === "deploy") return;
    cleanup(keymap.bind("mod+k", () => {
      if (tab() === "catalog") {
        search?.focus();
      } else {
        pendingSearchFocus = true;
        pickTab("catalog");
      }
    }, { when: "always" }));
  });

  function open(stack: Stack, nextMode?: string) {
    props.navigate({
      page: "stacks", stackId: stack.id, source: stack.source, tab: nextMode,
      projectId: props.route.projectId, scopePath: props.route.scopePath, agentKey: props.route.agentKey,
    });
  }

  function openLocal(stack: Stack) {
    open({ ...stack, source: "Local" });
    props.refresh();
  }

  function chooseFilter(value: string) {
    if (value.startsWith("tag:")) {
      tag.set(value.slice(4));
    } else if (sourceFilters.includes(value as SourceFilter)) {
      source.set(value as SourceFilter);
      if (!tags().includes(tag())) tag.set("");
    }
  }

  return <View style={{ ...viewport, backgroundColor: props.theme.background }}>
    <If when={editorOpen()} else={
      <>
        <Header theme={props.theme} title={t().stacks}>
          <Action theme={props.theme} label={t().import}
            run={() => props.navigate({ page: "stacks", tab: "import" })} />
          <Action theme={props.theme} label={t().create} glyph="plus"
            run={() => props.navigate({ page: "stacks", tab: "create" })} />
        </Header>
        <Tabs accessibilityLabel={t().stacks} indicatorStyle={{ bottom: 0, height: 2, backgroundColor: props.theme.primary }}
          style={{ ...row, minHeight: 36, paddingHorizontal: 20, borderBottomWidth: 1,
            borderColor: props.theme.border, flexShrink: 0, gap: 16 }}>
          <CatalogTab theme={props.theme} label={t().Local} glyph="layers" selected={tab() === "Local"} run={() => pickTab("Local")} />
          <CatalogTab theme={props.theme} label={t().Featured} glyph="users" selected={tab() === "catalog"} run={() => pickTab("catalog")} />
        </Tabs>
        <View style={viewport}>
          <ResizablePanelGroup axis="horizontal" handleStyle={{ backgroundColor: props.theme.border }}
            handleAccessibilityLabel={t().stacks}>
            <ResizablePanel size={width()} minSize={240} maxSize={500} onResize={({ size }) => width.set(size)}
              style={{ minHeight: 0, minWidth: 0, overflow: "hidden", backgroundColor: props.theme.background }}>
              <If when={tab() === "catalog"}>
                <View style={{ padding: 12, gap: 8, flexShrink: 0, borderBottomWidth: 1, borderColor: props.theme.border }}>
                  <View style={row}>
                    <View style={{ flex: 1, minWidth: 0 }}>
                      <Field theme={props.theme} label={t().searchStacks} search compact value={query()}
                        update={query.set} receiveRef={registerSearch}
                        submit={() => { const stack = filtered()[0]; if (stack) open(stack); }} />
                    </View>
                    <Action theme={props.theme} label={t().refreshCommunity} glyph="refresh-cw" iconOnly compact
                      pending={community().pending} run={() => reloadCommunity.update((value) => value + 1)} />
                  </View>
                  <View style={{ width: 168, maxWidth: "100%" }}>
                    <Choice theme={props.theme} label={t().filterTag} compact menuWidth={240} menuHeight={360}
                      options={[
                        { value: "tag:", title: t().allTags },
                        ...tags().map((value) => ({ value: `tag:${value}`, title: value })),
                        ...sourceFilters.map((value) => ({ value, title: t()[value], group: t().catalogSource })),
                      ]}
                      value={tag() ? `tag:${tag()}` : source() === "all" ? "tag:" : source()}
                      displayValue={tag() || (source() === "all" ? t().allTags : t()[source()])}
                      choose={chooseFilter} />
                  </View>
                  <Feedback theme={props.theme} label={t().loading} error={community().error} />
                </View>
              </If>
              <If when={catalog().pending || catalog().error}>
                <View style={{ padding: 12 }}>
                  <Feedback theme={props.theme} label={t().loading} pending={catalog().pending} error={catalog().error} />
                </View>
              </If>
              <If when={filtered().length > 0} else={
                <If when={!catalog().pending && (tab() === "Local" || !community().pending)}>
                  <CatalogEmpty theme={props.theme} title={tab() === "Local" ? t().noLocalStacks : t().noFeaturedStacks}
                    description={t().emptyStacksDescription} showIcon>
                    <Action theme={props.theme} label={t().createFirst} primary compact
                      run={() => props.navigate({ page: "stacks", tab: "create" })} />
                  </CatalogEmpty>
                </If>
              }>
                <List each={filtered()} keyExtractor={(stack) => `${stack.source}:${stack.id}`} estimatedItemSize={68} style={viewport}>
                  {(stack) => <StackCard theme={props.theme} stack={stack()} locale={props.locale}
                    selected={selected()?.id === stack().id && selected()?.source === stack().source} run={() => open(stack())} />}
                </List>
              </If>
            </ResizablePanel>
            <ResizablePanel minSize={240} style={{ minHeight: 0, minWidth: 0, overflow: "hidden", backgroundColor: props.theme.surface }}>
              <If when={selected()} else={
                <CatalogEmpty theme={props.theme} title={t().selectStack} description={t().selectStackDescription} />
              }>
                {(stack) => <StackDetail screen={props} stack={stack()} local={local()} open={open} />}
              </If>
            </ResizablePanel>
          </ResizablePanelGroup>
        </View>
      </>
    }>
      <If when={mode() === "create"}><StackEditor screen={props} saved={openLocal} /></If>
      <If when={mode() === "import"}><StackImport screen={props} saved={openLocal} /></If>
      <If when={mode() === "edit"}>
        <If when={selected()} else={
          <Header theme={props.theme} title={t().edit}>
            <Action theme={props.theme} label={t().cancel} ghost run={() => props.navigate({ page: "stacks", source: "Local" })} />
          </Header>
        }>
          {(stack) => <StackEditor screen={props} stack={stack()} saved={openLocal} />}
        </If>
      </If>
    </If>
    <If when={mode() === "deploy" ? selected() : undefined}>
      {(stack) => <StackDeploy screen={props} stack={stack()} back={() => open(stack())} />}
    </If>
  </View>;
}

function CatalogTab(props: { theme: Theme; label: string; glyph: IconName; selected: boolean; run: () => void }) {
  return <TabItem theme={props.theme} accessibilityLabel={props.label} selected={props.selected} onPress={props.run}
    style={{ ...row, gap: 4, height: 35, borderRadius: props.theme.radius }}>
    <Icon name={props.glyph} color={props.selected ? props.theme.foreground : props.theme.muted} size={14} />
    <Label theme={props.theme} label={props.label} muted={!props.selected} weight={props.selected ? "600" : undefined} />
  </TabItem>;
}

function CatalogEmpty(props: { theme: Theme; title: string; description: string; showIcon?: boolean; children?: JSX.Element }) {
  return <View style={{ ...viewport, alignItems: "center", justifyContent: "center", padding: 16, gap: 8 }}>
    <If when={props.showIcon}><Icon name="layout-dashboard" color={props.theme.muted} size={24} /></If>
    <Heading theme={props.theme} label={props.title} />
    <Text style={{ fontFamily: props.theme.fontFamily, fontSize: props.theme.fontSize - 2, color: props.theme.muted,
      textAlign: "center", maxWidth: "100%" }}>{props.description}</Text>
    {props.children}
  </View>;
}

function StackBadge(props: { theme: Theme; label: string; variant: "verified" | "community" | "local" }) {
  return <View style={{ flexShrink: 0, paddingHorizontal: 6, borderWidth: 1, borderRadius: 4,
    borderColor: props.variant === "verified" ? props.theme.success : props.theme.border,
    backgroundColor: props.variant === "local" ? props.theme.surface : "transparent" }}>
    <Label theme={props.theme} label={props.label} size={11} lines={1}
      color={props.variant === "verified" ? props.theme.success : props.theme.foreground} />
  </View>;
}

function StackCard(props: { theme: Theme; stack: Stack; locale: ScreenProps["locale"]; selected: boolean; run: () => void }) {
  const t = () => strings(props.locale);
  return <View style={{ paddingHorizontal: 12, paddingVertical: 4 }}>
    <Button accessibilityLabel={props.stack.name} selected={props.selected} onPress={props.run}
      style={{ height: 60, minWidth: 0, overflow: "hidden", justifyContent: "center", gap: 4,
        paddingHorizontal: 12, borderWidth: 1, borderColor: props.theme.border, borderRadius: props.theme.radius,
        backgroundColor: props.theme.background }}
      hoverStyle={interactionStyles(props.theme, { selectedFill: props.selected ? props.theme.accent : undefined }).hoverStyle}
      activeStyle={interactionStyles(props.theme, { selectedFill: props.selected ? props.theme.accent : undefined }).activeStyle}
      focusVisibleStyle={focusOutline(props.theme)}
      selectedStyle={{ backgroundColor: props.theme.accent, borderColor: props.theme.primary }}>
      <View style={row}>
        <View style={{ flex: 1, minWidth: 0 }}>
          <Label theme={props.theme} label={props.stack.name} weight="600" lines={1} />
        </View>
        <StackBadge theme={props.theme}
          variant={props.stack.verified ? "verified" : props.stack.source === "Community" ? "community" : "local"}
          label={props.stack.verified ? t().verified : props.stack.source === "Community" ? t().Community : t().localBadge} />
      </View>
      <Label theme={props.theme} label={[stackSize(props.locale, props.stack.skills.length), props.stack.description].filter(Boolean).join(" · ")}
        muted size={12} lines={1} />
    </Button>
  </View>;
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
  const memberRows = computed(() => stackMemberRows(members().data ?? [], (status) => status.member));
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

  const allInstalled = () => !members().pending && !members().error && missing() === 0;
  const pendingMember = signal<string | undefined>(undefined);

  function installMember(status: ReturnType<typeof memberRows>[number]) {
    pendingMember.set(status.key);
    return op.run(() => request("stack.install_member", {
      ...stackRef(props.stack), member_name: status.item.member.name, member_source: status.item.member.source,
    }), () => {
      outcome.set(undefined);
      screen.refresh();
      screen.notify(t().installed, "success");
    }).finally(() => pendingMember.set(undefined));
  }

  return <View style={viewport}>
    <Scroll style={viewport} contentStyle={{ ...padding, gap: 16 }}>
      <View style={row}>
        <View style={{ flex: 1, minWidth: 0, gap: 8 }}>
          <View style={row}>
            <View style={{ flex: 1, minWidth: 0 }}>
              <Heading theme={screen.theme} label={props.stack.name} lines={1} />
            </View>
            <If when={props.stack.verified}><StackBadge theme={screen.theme} variant="verified" label={t().verified} /></If>
          </View>
          <If when={props.stack.description}><Label theme={screen.theme} label={props.stack.description} /></If>
        </View>
        <View style={{ alignSelf: "flex-start", flexShrink: 0 }}>
          <OverflowMenu theme={screen.theme} label={t().stackActions}>
            <MenuAction theme={screen.theme} label={t().copyToml} disabled={op.busy()} run={copyToml} />
            <MenuAction theme={screen.theme} label={t().export} disabled={op.busy()} run={exportToml} />
            <MenuAction theme={screen.theme} label={t().share} disabled={op.busy()} run={share} />
            <If when={isLocal()} else={
              <If when={hasLocal()}>
                <MenuAction theme={screen.theme} label={t().Local} disabled={op.busy()} run={() => {
                  const stack = props.local.find(({ id }) => id === props.stack.id);
                  if (stack) props.open(stack);
                }} />
              </If>
            }>
              <MenuSeparator style={{ height: 1, backgroundColor: screen.theme.border, marginVertical: 4 }} />
              <MenuAction theme={screen.theme} label={t().edit} disabled={op.busy()} run={() => props.open(props.stack, "edit")} />
              <MenuAction theme={screen.theme} label={t().delete} danger disabled={op.busy()} run={remove} />
            </If>
          </OverflowMenu>
        </View>
      </View>
      <If when={props.stack.recommended_agents.length > 0}>
        <Label theme={screen.theme} muted label={`${t().recommendedAgents}  ${props.stack.recommended_agents.join(", ")}`} />
      </If>
      <Label theme={screen.theme} label={`${t().includedSkills} (${props.stack.skills.length})`} weight="600" />
      <Feedback theme={screen.theme} label={t().loading} pending={members().pending} error={op.error() ?? members().error} />
      <If when={installedResult()}>{(result) => <InstallReport screen={screen} result={result()} />}</If>
      <View style={{ borderTopWidth: 1, borderColor: screen.theme.border }}>
        <List each={memberRows()} keyExtractor={(row) => row.key} viewport="parent" estimatedItemSize={56}>
          {(item) => {
            const status = () => item().item;
            return <View style={{ ...row, minHeight: 40, borderBottomWidth: 1, borderColor: screen.theme.border, paddingVertical: 8 }}>
              <View style={{ flex: 1, minWidth: 0, gap: 2 }}>
                <Label theme={screen.theme} label={status().member.name} weight="600" lines={1} />
                <Label theme={screen.theme} label={status().member.source}
                  title={[status().member.source, status().member.note].filter(Boolean).join("\n")} muted size={12} lines={1} />
              </View>
              <If when={status().installed} else={
                <Action theme={screen.theme} label={t().install} compact glyph="arrow-down"
                  disabled={op.busy()} pending={op.busy() && pendingMember() === item().key} run={() => installMember(item())} />
              }>
                {(skill) => <Button
                  accessibilityLabel={status().source_conflict ? t().sourceConflict : t().inLibrary}
                  onPress={() => screen.navigate({ page: "library", skillId: skill().id })}
                  focusVisibleStyle={{ borderColor: screen.theme.primary }}
                  style={{ flexShrink: 0, minHeight: 24, maxWidth: "46%", justifyContent: "center", borderWidth: 1,
                    borderColor: "transparent", borderRadius: screen.theme.radius }}>
                  <Label theme={screen.theme} label={status().source_conflict ? t().sourceConflict : t().inLibrary}
                    color={status().source_conflict ? screen.theme.warning : screen.theme.success} size={12} lines={1} />
                </Button>}
              </If>
            </View>;
          }}
        </List>
      </View>
    </Scroll>
    <Footer theme={screen.theme}>
      <View style={{ flex: 1, minWidth: 0 }}>
        <If when={members().data}>
          <Label theme={screen.theme} label={installCounts(screen.locale, missing(), props.stack.skills.length - missing())} muted size={12} />
        </If>
      </View>
      <If when={isLocal() && props.stack.skills.length > 0}>
        <Action theme={screen.theme} label={t().deploy} glyph="folder-open" primary={allInstalled()}
          disabled={op.busy()} run={() => props.open(props.stack, "deploy")} />
      </If>
      <If when={!members().pending && !members().error && (missing() > 0 || !hasLocal())}>
        <Action theme={screen.theme} label={missing() > 0 ? t().installMissing : t().saveStack}
          glyph={missing() > 0 ? "arrow-down" : "check"} primary pending={op.busy() && pendingMember() === undefined}
          disabled={op.busy()} run={install} />
      </If>
    </Footer>
  </View>;
}
