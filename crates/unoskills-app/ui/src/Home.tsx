import { Scroll, View, effect, openUrl, signal, windowSize, type JSX } from "nsh";
import { Action, ActionControl, Hint, Icon, InlineError, Label, PageHeader, SkeletonBlock, panelSurface, textSize } from "./components";
import { strings } from "./i18n";
import { attachProjectFromPicker } from "./projects";
import { call, errorMessage, type ScreenProps, type Theme } from "./shared";

type RecentSkill = { id: string; name: string; date: string | null };
type Project = { id: string; name: string; path: string };
type Summary = {
  skill_count: number;
  deployed_count: number;
  project_count: number;
  agent_count: number;
  enabled_agent_count: number;
  adoptable_count: number;
  recent: RecentSkill[];
  projects: Project[];
};
type TrendingSkill = { id: string; name: string; source: string; installs: number; url: string | null };

const row = { flexDirection: "row", alignItems: "center" } as const;
const wrapRow = { ...row, flexWrap: "wrap", columnGap: 10, rowGap: 6 } as const;
const pane = { minWidth: 0, gap: 6, padding: 14 } as const;
const marketRow = { ...row, minHeight: 36, paddingHorizontal: 12, gap: 12 } as const;
const compactRankLimit = 3;
const fullRankLimit = 6;

export function Home(props: ScreenProps & { onAdoptableCount: (count: number) => void }) {
  const t = () => strings[props.locale];
  const theme = () => props.theme;
  const summary = signal<Summary | null>(null);
  const summaryError = signal("");
  const trending = signal<TrendingSkill[] | null>(null);
  const marketUnavailable = signal(false);
  const expandedRanking = signal(false);
  const attaching = signal(false);
  const emptyLibrary = () => summary()?.skill_count === 0;
  const splitWorkspace = () =>
    windowSize().width >= 1200 && !emptyLibrary() && (summary()?.projects.length ?? 0) > 0;
  const compactRanking = () => windowSize().height < 720;
  const rankLimit = () => compactRanking() && !expandedRanking() ? compactRankLimit : fullRankLimit;
  const formatCount = (count: number) => count.toLocaleString(props.locale);
  const installSkill = () => props.navigate({ page: "discover", tab: "install" });
  const openLeaderboard = () => props.navigate({ page: "discover", tab: "trending" });

  effect(() => {
    void props.revision;
    let current = true;
    summaryError.set("");
    void call<null, Summary>("home.summary", null).then(
      (data) => {
        if (current) {
          summary.set(data);
          props.onAdoptableCount(data.adoptable_count);
        }
      },
      (error) => { if (current) summaryError.set(errorMessage(error)); },
    );
    return () => { current = false; };
  });

  effect(() => {
    let current = true;
    void call<null, TrendingSkill[]>("home.market", null).then(
      (items) => { if (current) trending.set(items); },
      () => { if (current) marketUnavailable.set(true); },
    );
    return () => { current = false; };
  });

  async function attachProject() {
    if (attaching()) return;
    attaching.set(true);
    try {
      const project = await attachProjectFromPicker(t().btn_add_project);
      if (!project) return;
      props.navigate({ page: "projects", projectId: project.id, scopePath: "" });
      props.refresh();
    } catch (error) {
      props.notify(errorMessage(error), "error");
    } finally {
      attaching.set(false);
    }
  }

  async function openTrending(skill: TrendingSkill) {
    try {
      if (skill.url && await openUrl(skill.url)) return;
    } catch (error) {
      props.notify(errorMessage(error), "error");
    }
    openLeaderboard();
  }

  return <View style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
    <PageHeader theme={theme()} title={t().nav_home}>
      {(summary() && !emptyLibrary()) || summaryError()
        ? <Action theme={theme()} variant="primary" glyph="plus" label={t().btn_install_from_link} onPress={installSkill} />
        : null}
    </PageHeader>
    <Scroll style={{ flex: 1, minHeight: 0 }}>
      <View style={{ width: "100%", maxWidth: 1020, alignSelf: "center", padding: 20, gap: 18 }}>
        {summaryError() ? <View style={{ gap: 6 }}>
          <InlineError theme={theme()} message={summaryError()} />
          <Action theme={theme()} variant="outline" size="compact" label={t().retry} onPress={props.refresh} />
        </View> : null}
        {!summary() && !summaryError()
          ? <View role="summary" accessibilityLabel={t().loading} accessibilityState={{ busy: true }}
              style={{ gap: 10, paddingVertical: 10 }}>
              <SkeletonBlock theme={theme()} width={148} height={16} />
              <SkeletonBlock theme={theme()} width="48%" height={12} />
            </View>
          : null}
        <If when={summary()}>{(data) => <>
          <View style={{ ...panelSurface(theme()), padding: 4, gap: 4,
            flexDirection: windowSize().width < 680 ? "column" : "row" }}>
            <StatusShortcut theme={theme()} glyph="bot" label={t().home_agents}
              value={t().home_agent_counts.replace("{detected}", formatCount(data().agent_count))
                .replace("{enabled}", formatCount(data().enabled_agent_count))}
              onPress={() => props.navigate({ page: "settings", tab: "agents" })} />
            <StatusShortcut theme={theme()} glyph="folder-open" label={t().home_linked_projects}
              value={formatCount(data().project_count)}
              onPress={() => props.navigate({ page: "projects", tab: "choose" })} />
            {data().adoptable_count > 0
              ? <StatusShortcut theme={theme()} glyph="archive-restore" label={t().home_adoptable_skills}
                  value={formatCount(data().adoptable_count)}
                  onPress={() => props.navigate({ page: "discover", tab: "adopt" })} />
              : null}
          </View>
          {emptyLibrary() ? <View style={{ ...panelSurface(theme()), gap: 8, padding: 16 }}>
            <Label theme={theme()} heading size={textSize(theme(), 20)}>{t().home_first_skill}</Label>
            <Label theme={theme()} muted>{t().home_first_skill_desc}</Label>
            <View style={wrapRow}>
              <Action theme={theme()} variant="primary" glyph="plus" label={t().btn_install_from_link} onPress={installSkill} />
              <Action theme={theme()} variant="outline" glyph="layers" label={t().home_browse_stacks}
                onPress={() => props.navigate({ page: "stacks", source: "Featured" })} />
            </View>
          </View> : null}

          <View style={panelSurface(theme())}>
            <View style={{ flexDirection: splitWorkspace() ? "row" : "column" }}>
              <View style={{ ...pane, flex: splitWorkspace() ? 1 : undefined }}>
                <HomeHeading theme={theme()} title={data().projects.length ? t().home_projects : t().home_projects_empty_title}>
                  <Action theme={theme()} variant="ghost" size="compact" glyph="plus" label={t().btn_add_project}
                    pending={attaching()} onPress={attachProject} />
                </HomeHeading>
                {data().projects.length === 0
                  ? <Label theme={theme()} muted>{t().home_projects_empty_desc}</Label>
                  : null}
                <View>
                  <Each of={data().projects} key={(project) => project.id}>{(project) =>
                    <HomeRow theme={theme()} glyph="folder-open" title={project().name} detail={project().path} path
                      onPress={() => props.navigate({ page: "projects", projectId: project().id, scopePath: "" })} />
                  }</Each>
                </View>
              </View>

              {!emptyLibrary() ? <View style={{ ...pane, flex: splitWorkspace() ? 1 : undefined, borderColor: theme().border,
                borderLeftWidth: splitWorkspace() ? 1 : 0, borderTopWidth: splitWorkspace() ? 0 : 1 }}>
                <HomeHeading theme={theme()} title={t().nav_library}>
                  <Action theme={theme()} variant="ghost" size="compact" glyph="arrow-right" label={t().home_open_library}
                    onPress={() => props.navigate({ page: "library" })} />
                </HomeHeading>
                <Label theme={theme()} muted size={textSize(theme(), 13)}>{t().home_library_count
                  .replace("{total}", formatCount(data().skill_count))
                  .replace("{deployed}", formatCount(data().deployed_count))}</Label>
                <Label theme={theme()} weight="600" size={textSize(theme(), 12)}>{t().home_recent}</Label>
                <View>
                  <Each of={data().recent} key={(skill) => skill.id}>{(skill) =>
                    <HomeRow theme={theme()} glyph="file-text" title={skill().name} detail={skill().date ?? undefined}
                      onPress={() => props.navigate({ page: "library", skillId: skill().id })} />
                  }</Each>
                </View>
              </View> : null}
            </View>
            {!emptyLibrary() ? <View style={{ ...wrapRow, padding: 8, borderTopWidth: 1, borderColor: theme().border }}>
              <Action theme={theme()} variant="ghost" size="compact" glyph="layers" label={t().tab_my_stacks}
                onPress={() => props.navigate({ page: "stacks", source: "Local" })} />
            </View> : null}
          </View>
        </>}</If>

        <View style={{ gap: 8 }}>
          <HomeHeading theme={theme()} title={t().home_market_trending}>
            <Action theme={theme()} variant="ghost" size="compact" glyph="arrow-right" label={t().home_view_market}
              onPress={openLeaderboard} />
          </HomeHeading>
          {marketUnavailable() ? <Label theme={theme()} muted>{t().home_market_unavailable}</Label> : null}
          {trending()?.length === 0 ? <Label theme={theme()} muted>{t().home_market_empty}</Label> : null}
          {!trending() && !marketUnavailable()
            ? <View testID="home-ranking-skeleton" role="summary" accessibilityLabel={t().home_market_loading}
                accessibilityState={{ busy: true }} style={panelSurface(theme())}>
                <Each of={Array.from({ length: rankLimit() }, (_, index) => index)}>{() =>
                  <View style={marketRow}>
                    <SkeletonBlock theme={theme()} width="30%" height={12} />
                    <View style={{ flex: 1 }} />
                    <SkeletonBlock theme={theme()} width={64} height={10} />
                  </View>
                }</Each>
              </View>
            : null}
          <If when={(trending()?.length ?? 0) > 0}>
            <View style={panelSurface(theme())}>
              <Each of={trending()?.slice(0, rankLimit()) ?? []} key={(skill) => skill.id}>{(skill, index) =>
                <View style={{ borderTopWidth: index() > 0 ? 1 : 0, borderColor: theme().border }}>
                  <ActionControl theme={theme()} role={skill().url ? "link" : "button"}
                    accessibilityLabel={`${skill().name} · ${skill().url ? t().home_market_details : t().home_view_market}`}
                    tooltip={<Hint theme={theme()} text={skill().source ? `${skill().name} · ${skill().source}` : skill().name} />}
                    style={{ ...marketRow, borderRadius: theme().control.radius }}
                    onPress={() => void openTrending(skill())}>
                    <View style={{ ...row, flex: 1, minWidth: 0, gap: 10 }}>
                      <View style={{ maxWidth: skill().source ? "62%" : "100%", flexShrink: 1, minWidth: 0 }}>
                        <Label theme={theme()} weight="500" truncate>{skill().name}</Label>
                      </View>
                      {skill().source
                        ? <View style={{ flex: 1, minWidth: 0 }}>
                            <Label theme={theme()} muted truncate size={textSize(theme(), 12)}>{skill().source}</Label>
                          </View>
                        : null}
                    </View>
                    <Label theme={theme()} muted size={textSize(theme(), 12)}>{t().home_market_installs
                      .replace("{count}", formatCount(skill().installs))}</Label>
                    <Icon name={skill().url ? "external-link" : "chevron-right"} color={theme().muted} size={12} />
                  </ActionControl>
                </View>
              }</Each>
              {compactRanking() && (trending()?.length ?? 0) > compactRankLimit
                ? <View style={{ ...row, padding: 4, justifyContent: "center", borderTopWidth: 1, borderColor: theme().border }}>
                    <Action theme={theme()} variant="ghost" size="compact"
                      label={expandedRanking() ? t().home_market_show_less : t().home_market_show_more}
                      onPress={() => expandedRanking.update((current) => !current)} />
                  </View>
                : null}
            </View>
          </If>
        </View>
      </View>
    </Scroll>
  </View>;
}

function StatusShortcut(props: {
  theme: Theme; glyph: "bot" | "folder-open" | "archive-restore"; label: string; value: string; onPress: () => void;
}) {
  return <ActionControl theme={props.theme} accessibilityLabel={`${props.label} · ${props.value}`} onPress={props.onPress}
    style={{ ...row, flexGrow: 1, minWidth: 0, minHeight: 56, gap: 10, paddingHorizontal: 12, paddingVertical: 6,
      borderRadius: props.theme.control.radius }}>
    <Icon name={props.glyph} color={props.theme.muted} />
    <View style={{ flex: 1, minWidth: 0 }}>
      <Label theme={props.theme} weight="500">{props.label}</Label>
      <Label theme={props.theme} muted size={textSize(props.theme, 12)}>{props.value}</Label>
    </View>
    <Icon name="chevron-right" color={props.theme.muted} size={12} />
  </ActionControl>;
}

function HomeHeading(props: { theme: Theme; title: string; children?: JSX.Element }) {
  return <View style={{ ...wrapRow, justifyContent: "space-between", minHeight: 30 }}>
    <Label theme={props.theme} heading size={textSize(props.theme, 18)}>{props.title}</Label>
    {props.children}
  </View>;
}

function HomeRow(props: {
  theme: Theme; title: string; detail?: string; path?: boolean; glyph: "folder-open" | "file-text"; onPress: () => void;
}) {
  const detail = () => {
    const full = props.detail ?? "";
    if (!props.path || full.length <= 58) return full;
    const separator = full.includes("\\") ? "\\" : "/";
    return `…${separator}${full.split(/[\\/]/).slice(-2).join(separator)}`;
  };
  return <View style={{ borderTopWidth: 1, borderColor: props.theme.border }}>
    <ActionControl theme={props.theme} accessibilityLabel={props.title} onPress={props.onPress}
      tooltip={props.path && props.detail ? <Hint theme={props.theme} text={props.detail} /> : undefined}
      style={{ ...row, minHeight: props.path ? 50 : 34, paddingHorizontal: 8, gap: 8, borderWidth: 1,
        borderColor: "transparent", borderRadius: props.theme.control.radius }}>
      <Icon name={props.glyph} color={props.theme.muted} />
      <View style={{ flex: 1, minWidth: 0 }}>
        <Label theme={props.theme} weight={props.path ? "500" : "400"} truncate>{props.title}</Label>
        {props.path && props.detail
          ? <Label theme={props.theme} muted mono truncate size={textSize(props.theme, 12)}>{detail()}</Label>
          : null}
      </View>
      {!props.path && props.detail
        ? <Label theme={props.theme} muted size={textSize(props.theme, 12)}>{props.detail}</Label>
        : null}
      <Icon name="chevron-right" color={props.theme.muted} size={12} />
    </ActionControl>
  </View>;
}
