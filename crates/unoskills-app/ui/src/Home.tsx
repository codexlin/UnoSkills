import { Scroll, View, cleanup, computed, effect, openUrl, reduceMotion, signal, windowSize } from "nsh";
import { Bar, Grid, Plot, PlotAxis } from "nsh/base/plot";
import { Action, ActionControl, Icon, InlineError, Label, PageHeader, TabList, textSize } from "./components";
import { strings } from "./i18n";
import { call, errorMessage, type ScreenProps } from "./shared";
import { request } from "./projects/host";

type Board = "trending" | "all-time";
type RecentSkill = { id: string; name: string; date: string | null };
type Project = { id: string; name: string; path: string };
type Day = { label: string; count: number };
type Summary = {
  skill_count: number; deployed_count: number; agent_count: number; adoptable_count: number;
  recent: RecentSkill[]; projects: Project[]; activity: Day[];
};
type Ranking = { id: string; name: string; installs: number; source: string; url: string | null };

export function Home(props: ScreenProps & { onAdoptableCount: (count: number) => void }) {
  const t = () => strings[props.locale];
  const [summary, summaryError] = [signal<Summary | null>(null), signal("")];
  const board = signal<Board>("trending");
  const cache = signal<Partial<Record<Board, Ranking[]>>>({});
  const marketError = signal("");
  const marketLoading = signal(false);
  const attaching = signal(false);
  const wide = () => windowSize().width >= 900;
  const compact = () => windowSize().height < 680;
  const activeRows = computed(() => cache()[board()]);

  effect(() => {
    void props.revision;
    let current = true;
    summaryError.set("");
    call<null, Summary>("home.summary", null).then(
      (data) => { if (current) { summary.set(data); props.onAdoptableCount(data.adoptable_count); } },
      (error) => { if (current) summaryError.set(errorMessage(error)); },
    );
    return () => { current = false; };
  });

  let marketRequest = 0;
  cleanup(() => { marketRequest += 1; });
  async function loadBoard(selected: Board, force = false) {
    const request = ++marketRequest;
    marketLoading.set(false);
    marketError.set("");
    if (!force && cache()[selected]) return;
    marketLoading.set(true);
    try {
      const rows = await call<{ board: Board }, Ranking[]>("home.market", { board: selected });
      if (request === marketRequest) cache.update((entries) => ({ ...entries, [selected]: rows }));
    } catch (error) {
      if (request === marketRequest) marketError.set(errorMessage(error));
    } finally {
      if (request === marketRequest) marketLoading.set(false);
    }
  }
  void loadBoard(board());

  async function openSkill(row: Ranking) {
    try {
      if (row.url && await openUrl(row.url)) return;
      props.navigate({ page: "discover", tab: "install", source: row.source });
    } catch (error) {
      props.notify(errorMessage(error), "error");
    }
  }

  async function attachProject() {
    if (attaching()) return;
    attaching.set(true);
    try {
      const path = await call<{ title: string }, string | null>("dialog.folder", { title: t().btn_add_project });
      if (!path) return;
      const project = await request("project.attach", { path });
      props.navigate({ page: "projects", projectId: project.id, scopePath: "" });
      props.refresh();
    } catch (error) {
      props.notify(errorMessage(error), "error");
    } finally {
      attaching.set(false);
    }
  }

  const metrics = computed(() => [
    { label: t().nav_library, count: summary()?.skill_count, icon: "book-open", route: { page: "library" } },
    { label: t().home_deployed, count: summary()?.deployed_count, icon: "network", route: { page: "library" } },
    { label: t().home_agents, count: summary()?.agent_count, icon: "bot", route: { page: "settings", tab: "agents" } },
    { label: t().discover_adopt_tab, count: summary()?.adoptable_count, icon: "archive-restore", route: { page: "discover", tab: "adopt" } },
  ] as const);
  const theme = () => props.theme;
  const radius = () => theme().radius === 0 ? 0 : Math.min(16, theme().radius + 2);
  const boardItems = ["trending", "all-time"] as const;

  function countLabel(label: string, count: number | undefined) {
    if (count === undefined) return label;
    return props.locale === "en" ? `${label} (${count})` : `${label}（${count}）`;
  }

  function metric(index: number) {
    const item = () => metrics()[index];
    return <View style={{ flex: 1, minWidth: 0, padding: 4, borderLeftWidth: index % 2, borderColor: theme().border }}>
      <ActionControl theme={theme()} accessibilityLabel={countLabel(item().label, item().count)}
        onPress={() => props.navigate(item().route)}
        style={{ ...row, height: 44, minWidth: 0, paddingHorizontal: 10, gap: 8, borderRadius: theme().radius,
          borderWidth: 1, borderColor: "transparent" }}>
        <Icon name={item().icon} color={theme().muted} />
        <View style={{ flex: 1, minWidth: 0 }}>
          <Label theme={theme()} truncate>{item().label}</Label>
        </View>
        <Label theme={theme()} weight="600" size={textSize(theme(), 20)}>{String(item().count ?? "—")}</Label>
        <Icon name="chevron-right" color={theme().muted} size={12} />
      </ActionControl>
    </View>;
  }

  return (
    <View style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
      <PageHeader theme={theme()} title={t().nav_home}>
        <Action theme={theme()} variant="primary" size="regular" glyph="plus" label={t().btn_install_from_link}
          onPress={() => props.navigate({ page: "discover", tab: "install" })} />
      </PageHeader>
      <Scroll style={{ flex: 1, minHeight: 0 }}>
        <View style={{ width: "100%", maxWidth: 1380, padding: 20 }}>
          <View style={{ flexDirection: wide() ? "row" : "column", alignItems: "flex-start", gap: wide() ? 24 : 20 }}>
            <View style={{ flexGrow: 1, flexBasis: 0, width: wide() ? undefined : "100%", minWidth: 0, gap: compact() ? 16 : 20 }}>
              {summaryError() ? <InlineError theme={theme()} message={summaryError()} /> : null}
              <View style={{ gap: 8 }}>
                <View style={sectionHeading}>
                  <View style={{ flex: 1, minWidth: 0 }}>
                    <Label theme={theme()} heading size={textSize(theme(), 18)}>{t().home_workspace}</Label>
                  </View>
                  <Action theme={theme()} variant="outline" size="small" glyph="layers" label={t().tab_my_stacks}
                    onPress={() => props.navigate({ page: "stacks", source: "Local" })} />
                </View>
                <View style={{ padding: 4, borderWidth: 1, borderColor: theme().border, borderRadius: radius() }}>
                  <View style={row}>{metric(0)}{metric(1)}</View>
                  <View style={{ ...row, borderTopWidth: 1, borderColor: theme().border }}>{metric(2)}{metric(3)}</View>
                </View>
              </View>
              <View style={{ ...row, flexWrap: "wrap", gap: 8 }}>
                <Label theme={theme()} weight="600">{t().home_projects}</Label>
                <Each of={summary()?.projects ?? []} key={(project) => project.id}>{(project) =>
                  <View style={{ maxWidth: 188, minWidth: 0 }}>
                    <Action theme={theme()} variant="outline" size="small" glyph="folder-open" label={project().name} tooltip={project().path}
                      onPress={() => props.navigate({ page: "projects", projectId: project().id, scopePath: "" })} />
                  </View>
                }</Each>
                <Action theme={theme()} variant="ghost" size="small" glyph="plus" label={t().btn_add_project}
                  disabled={attaching()} onPress={() => void attachProject()} />
              </View>
              <View>
                <View style={{ paddingBottom: 8 }}>
                  <Label theme={theme()} heading size={textSize(theme(), 16)}>{t().home_recent}</Label>
                </View>
                {summary() && !summary()!.recent.length ?
                  <Label theme={theme()} muted>{t().empty_library_desc}</Label> : null}
                <Each of={summary()?.recent ?? []} key={(skill) => skill.id}>{(skill) =>
                  <View style={{ borderTopWidth: 1, borderColor: theme().border }}>
                    <ActionControl theme={theme()} accessibilityLabel={skill().name}
                      onPress={() => props.navigate({ page: "library", skillId: skill().id })}
                      style={{ ...row, gap: 8, height: 32, paddingHorizontal: 10, borderRadius: theme().radius,
                        borderColor: "transparent", borderWidth: 1 }}>
                      <Icon name="file-text" color={theme().muted} />
                      <View style={{ flex: 1, minWidth: 0 }}><Label theme={theme()} truncate>{skill().name}</Label></View>
                      {skill().date ? <Label theme={theme()} muted size={textSize(theme(), 12)}>{skill().date}</Label> : null}
                      <Icon name="chevron-right" color={theme().muted} size={12} />
                    </ActionControl>
                  </View>
                }</Each>
              </View>
              {summary() ? <Activity theme={theme()} days={summary()!.activity} locale={props.locale} compact={compact()} /> : null}
            </View>
            <View style={{ width: wide() ? "40%" : "100%", minWidth: 0, flexShrink: 0, gap: 8 }}>
              <View style={sectionHeading}>
                <View style={{ flex: 1, minWidth: 0 }}>
                  <Label theme={theme()} heading size={textSize(theme(), 18)}>{t().home_market_rank}</Label>
                </View>
                <Action theme={theme()} variant="ghost" size="small" glyph={marketLoading() ? "loader-circle" : "refresh-cw"}
                  iconOnly tooltip={t().home_refresh_market} label={t().home_refresh_market}
                  disabled={marketLoading()} onPress={() => void loadBoard(board(), true)} />
              </View>
              <View style={{ borderWidth: 1, borderColor: theme().border, borderRadius: radius(),
                backgroundColor: theme().surface, padding: 12 }}>
                <View style={{ ...row, flexWrap: "wrap", justifyContent: "space-between", gap: 8, paddingBottom: 12 }}>
                  <TabList theme={theme()} accessibilityLabel={t().home_market_rank} compact selectedId={board()}
                    tabs={boardItems.map((id) => ({ id, label: id === "trending" ? t().board_trending : t().board_all_time }))}
                    onSelect={(entry) => { board.set(entry); void loadBoard(entry); }} />
                  {activeRows()?.length ? <View style={{ paddingRight: 40 }}>
                    <Label theme={theme()} muted size={textSize(theme(), 12)}>{t().home_market_installs}</Label>
                  </View> : null}
                </View>
                {marketLoading() ? <View style={{ ...row, gap: 8, paddingVertical: 16 }}>
                  <Icon name="loader-circle" color={theme().muted} size={14} spinning />
                  <Label theme={theme()} muted>{t().home_market_loading}</Label>
                </View> : null}
                {marketError() ? <InlineError theme={theme()} message={marketError()} /> : null}
                {activeRows()?.length === 0 ? <View style={{ paddingVertical: 16 }}>
                  <Label theme={theme()} muted>{t().home_market_empty}</Label>
                </View> : null}
                <Each of={activeRows() ?? []} key={(entry) => entry.id}>{(entry, index) =>
                  <View style={{ ...row, minHeight: 48, gap: 12, paddingHorizontal: 8, borderTopWidth: 1,
                    borderColor: theme().border,
                    backgroundColor: index() === 0 ? `color-mix(in srgb, ${theme().primary} 6%, transparent)` : undefined }}>
                    <View style={{ width: 24, alignItems: "center", flexShrink: 0 }}>
                      <Label theme={theme()} muted={index() !== 0} weight={index() === 0 ? "600" : "400"}
                        size={textSize(theme(), 12)}>{String(index() + 1)}</Label>
                    </View>
                    <View style={{ flex: 1, minWidth: 0 }}>
                      <Label theme={theme()} truncate weight="500">{entry().name}</Label>
                      <Label theme={theme()} truncate muted size={textSize(theme(), 12)}>{entry().source.split("@", 1)[0]}</Label>
                    </View>
                    <Label theme={theme()} weight="500">{formatInstalls(entry().installs, props.locale)}</Label>
                    <View style={{ width: 24, flexShrink: 0 }}>
                      {entry().url ? <Action theme={theme()} variant="ghost" size="badge" glyph="external-link" iconOnly role="link"
                        label={`${t().home_market_details} · ${entry().name} · ${entry().source}`} tooltip={t().home_market_details}
                        onPress={() => void openSkill(entry())} /> : null}
                    </View>
                  </View>
                }</Each>
                <View style={{ ...row, justifyContent: "flex-end", borderTopWidth: 1, borderColor: theme().border, paddingTop: 8 }}>
                  <Action theme={theme()} variant="ghost" size="small" glyph="arrow-right" label={t().home_view_market}
                    onPress={() => props.navigate({ page: "discover", tab: board() })} />
                </View>
              </View>
            </View>
          </View>
        </View>
      </Scroll>
    </View>
  );
}

const row = { flexDirection: "row", alignItems: "center" } as const;
const sectionHeading = { ...row, minHeight: 32, gap: 8 } as const;

function formatInstalls(count: number, locale: ScreenProps["locale"]) {
  if (count < 1000) return count.toLocaleString(locale);
  const [divisor, suffix] = count >= 1_000_000 ? [1_000_000, "M"] : [1000, "k"];
  const number = (count / divisor).toLocaleString(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  return `${number}${suffix}`;
}

function Activity(props: { theme: ScreenProps["theme"]; days: Day[]; locale: ScreenProps["locale"]; compact: boolean }) {
  const maximum = () => Math.max(0, ...props.days.map((day) => day.count));
  const total = () => props.days.reduce((sum, day) => sum + day.count, 0);
  const t = () => strings[props.locale];
  const ticks = computed(() => {
    const max = maximum();
    const roughStep = max / 4;
    const magnitude = 10 ** Math.max(0, Math.floor(Math.log10(roughStep)));
    const step = ([1, 2, 5, 10].find((unit) => unit * magnitude >= roughStep) ?? 10) * magnitude;
    return Array.from({ length: Math.floor(max / step) + 1 }, (_, index) => index * step);
  });
  const hovered = signal<number | null>(null);
  const selected = () => hovered() === null ? undefined : props.days[hovered()!];
  const axisStyle = () => ({ fontFamily: props.theme.fontFamily, fontSize: textSize(props.theme, 12), color: props.theme.muted });
  return (
    <View style={{ gap: props.compact ? 4 : 8 }}>
      <View style={{ ...row, justifyContent: "space-between", gap: 8 }}>
        <Label theme={props.theme} heading size={textSize(props.theme, 16)}>{t().home_intake}</Label>
        {total() > 0 ? <Label theme={props.theme} weight="600">
          {`${t().home_intake_total} ${total().toLocaleString(props.locale)}`}
        </Label> : null}
      </View>
      <If when={maximum() > 0} else={<Label theme={props.theme} muted>{t().home_intake_empty}</Label>}>
        <Plot data={props.days} y="count" xScale={{ type: "band", paddingInner: 0.2, paddingOuter: 0.1, maxBandWidth: 24 }}
          yScale={{ type: "linear", domain: [0, maximum()] }}
          accessibilityLabel={t().home_intake}
          accessibilityValue={selected() ? { text: `${selected()!.label} · ${selected()!.count.toLocaleString(props.locale)}` } : undefined}
          onHoverChange={({ index }) => hovered.set(index)}
          style={{ height: props.compact ? 84 : 108, marginLeft: 36, marginBottom: 20 }}
          focusVisibleStyle={{ outlineColor: props.theme.primary, outlineWidth: 1 }}
          crossLineStyle={{ width: 24, height: "100%", backgroundColor: `color-mix(in srgb, ${props.theme.primary} 6%, transparent)` }}
          motion={reduceMotion() ? undefined : { enter: { duration: 100 }, exit: { duration: 100 } }}
          tooltipContent={selected() ? <View style={{ minWidth: 132, gap: 4, padding: 8, borderRadius: props.theme.radius,
            backgroundColor: props.theme.surface, borderWidth: 1, borderColor: props.theme.border }}>
            <Label theme={props.theme} weight="600" size={textSize(props.theme, 12)}>{selected()!.label}</Label>
            <View style={{ ...row, justifyContent: "space-between", gap: 8 }}>
              <Label theme={props.theme} muted size={textSize(props.theme, 12)}>{t().nav_library}</Label>
              <Label theme={props.theme} size={textSize(props.theme, 12)}>{selected()!.count.toLocaleString(props.locale)}</Label>
            </View>
          </View> : undefined}>
          <Grid y={ticks().filter((value) => value > 0)} style={{ stroke: props.theme.border, strokeWidth: 1, strokeDasharray: "2 2" }} />
          <Bar style={{ fill: props.theme.primary }} />
          <PlotAxis placement="left" tickValues={ticks()} labelStyle={{ ...axisStyle(), paddingRight: 6 }} />
          <PlotAxis labelStyle={axisStyle()} format={(value) => props.days[value]?.label ?? ""}
            lineStyle={{ height: 1, backgroundColor: props.theme.border }} />
        </Plot>
        <Label theme={props.theme} muted size={textSize(props.theme, 12)}>{t().home_intake_scope}</Label>
      </If>
    </View>
  );
}
