import { Scroll, View, cleanup, computed, effect, openUrl, signal, windowSize } from "nsh";
import { Action, InlineError, Label, Section } from "./controls";
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
  const medium = () => windowSize().width >= 1120;
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
    { label: t().nav_library, total: summary()?.skill_count, route: { page: "library" } },
    { label: t().home_deployed, total: summary()?.deployed_count, route: { page: "library" } },
    { label: t().home_agents, total: summary()?.agent_count, route: { page: "settings", tab: "agents" } },
    { label: t().discover_adopt_tab, total: summary()?.adoptable_count, route: { page: "discover", tab: "adopt" } },
  ] as const);

  return (
    <View style={{ flex: 1, minHeight: 0, gap: 0 }}>
      <View style={{ flexDirection: "row", justifyContent: "space-between", alignItems: "center", padding: 20 }}>
        <Label theme={props.theme} heading size={24}>{t().nav_home}</Label>
        <Action theme={props.theme} onPrimary label={t().btn_install_from_link}
          onPress={() => props.navigate({ page: "discover", tab: "install" })} />
      </View>
      <Scroll style={{ flex: 1, minHeight: 0 }}>
        <View style={{ padding: 20, paddingTop: 0, gap: 20, flexDirection: medium() ? "row" : "column" }}>
          <View style={{ flex: 1, minWidth: 0, gap: 16 }}>
            {summaryError() ? <InlineError theme={props.theme} message={summaryError()} /> : null}
            <Section theme={props.theme} title={t().home_workspace}>
              <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 8 }}>
                <Each of={metrics()}>{(metric) =>
                  <Action theme={props.theme} label={`${metric().label} · ${metric().total ?? "—"}`}
                    onPress={() => props.navigate(metric().route)} />
                }</Each>
              </View>
              <Action theme={props.theme} label={t().tab_my_stacks}
                onPress={() => props.navigate({ page: "stacks", source: "Local" })} />
            </Section>
            <Section theme={props.theme} title={t().home_projects}>
              <Each of={summary()?.projects ?? []} key={(project) => project.id}>{(project) =>
                <View style={{ gap: 3, minWidth: 0 }}>
                  <Action theme={props.theme} compact label={project().name}
                    onPress={() => props.navigate({ page: "projects", projectId: project().id, scopePath: "" })} />
                  <Label theme={props.theme} muted mono truncate>{project().path}</Label>
                </View>
              }</Each>
              <Action theme={props.theme} label={t().btn_add_project} disabled={attaching()} onPress={() => void attachProject()} />
            </Section>
            <Section theme={props.theme} title={t().home_recent}>
              {summary() && !summary()!.recent.length ? <Label theme={props.theme} muted>{t().empty_library_desc}</Label> : null}
              <Each of={summary()?.recent ?? []} key={(skill) => skill.id}>{(skill) =>
                <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 8, minWidth: 0 }}>
                  <Action theme={props.theme} compact label={skill().name}
                    onPress={() => props.navigate({ page: "library", skillId: skill().id })} />
                  <Label theme={props.theme} muted>{skill().date ?? ""}</Label>
                </View>
              }</Each>
            </Section>
            <Section theme={props.theme} title={t().home_intake}>
              {summary() ? <Activity theme={props.theme} days={summary()!.activity} locale={props.locale} /> : null}
            </Section>
          </View>
          <View style={{ width: medium() ? 360 : "100%", flexShrink: 0, minWidth: 0 }}>
            <Section theme={props.theme} title={t().home_market_rank}>
              <View style={{ gap: 6, flexDirection: "row", flexWrap: "wrap" }}>
                <Action theme={props.theme} compact selected={board() === "trending"}
                  label={t().board_trending} onPress={() => { board.set("trending"); void loadBoard("trending"); }} />
                <Action theme={props.theme} compact selected={board() === "all-time"}
                  label={t().board_all_time} onPress={() => { board.set("all-time"); void loadBoard("all-time"); }} />
                <Action theme={props.theme} compact label={t().home_refresh_market} icon="↻"
                  disabled={marketLoading()} onPress={() => void loadBoard(board(), true)} />
              </View>
              {marketLoading() ? <Label theme={props.theme} muted>{t().busy_loading_market}</Label> : null}
              {marketError() ? <InlineError theme={props.theme} message={marketError()} /> : null}
              {activeRows()?.length === 0 ? <Label theme={props.theme} muted>{t().home_market_empty}</Label> : null}
              <Each of={activeRows() ?? []} key={(row) => row.id}>{(row) =>
                <View style={{ flexDirection: "row", alignItems: "center", gap: 8, justifyContent: "space-between" }}>
                  <View style={{ flex: 1, minWidth: 0 }}>
                    <Action theme={props.theme} compact role="link" label={row().name} onPress={() => void openSkill(row())} />
                  </View>
                  <Label theme={props.theme} muted>{row().installs.toLocaleString(props.locale)}</Label>
                </View>
              }</Each>
              <Action theme={props.theme} label={t().home_view_market}
                onPress={() => props.navigate({ page: "discover", tab: board() })} />
            </Section>
          </View>
        </View>
      </Scroll>
    </View>
  );
}

function Activity(props: { theme: ScreenProps["theme"]; days: Day[]; locale: ScreenProps["locale"] }) {
  const max = () => Math.max(1, ...props.days.map((day) => day.count));
  const total = () => props.days.reduce((sum, day) => sum + day.count, 0);
  const t = () => strings[props.locale];
  return (
    <View style={{ gap: 8 }}>
      <Label theme={props.theme} muted>{t().home_intake_scope}</Label>
      {total() === 0 ? <Label theme={props.theme} muted>{t().home_intake_empty}</Label> : (
        <View style={{ flexDirection: "row", gap: 6 }}>
          <Each of={props.days} key={(day) => day.label}>{(day) =>
            <View style={{ flex: 1, gap: 3, alignItems: "center", justifyContent: "flex-end" }}>
              <Label theme={props.theme} size={12}>{day().count}</Label>
              <View accessibilityElementsHidden style={{ width: "70%", height: Math.max(1, 58 * day().count / max()),
                borderRadius: 2, backgroundColor: props.theme.primary }} />
              <Label theme={props.theme} muted size={12}>{day().label}</Label>
            </View>
          }</Each>
        </View>
      )}
      <Label theme={props.theme} muted>{`${t().home_intake_total} · ${total()}`}</Label>
    </View>
  );
}
