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
  adoptable_count: number;
  recent: RecentSkill[];
  projects: Project[];
};
type TrendingSkill = { id: string; name: string; source: string; installs: number; url: string | null };

const row = { flexDirection: "row", alignItems: "center" } as const;
const wrapRow = { ...row, flexWrap: "wrap", columnGap: 10, rowGap: 6 } as const;
const pane = { minWidth: 0, gap: 6, padding: 14 } as const;
const marketTile = { minWidth: 0, minHeight: 80, gap: 2, padding: 12 } as const;

export function Home(props: ScreenProps & { onAdoptableCount: (count: number) => void }) {
  const t = () => strings[props.locale];
  const theme = () => props.theme;
  const summary = signal<Summary | null>(null);
  const summaryError = signal("");
  const trending = signal<TrendingSkill[] | null>(null);
  const marketUnavailable = signal(false);
  const attaching = signal(false);
  const emptyLibrary = () => summary()?.skill_count === 0;
  const splitWorkspace = () =>
    windowSize().width >= 1200 && !emptyLibrary() && (summary()?.projects.length ?? 0) > 0;
  const marketColumns = () => windowSize().width >= 900;
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

  function adoptLocal(count: number) {
    return count > 0
      ? <Action theme={theme()} variant="ghost" size="compact" glyph="archive-restore"
          label={t().home_adopt_count.replace("{count}", count.toLocaleString(props.locale))}
          onPress={() => props.navigate({ page: "discover", tab: "adopt" })} />
      : null;
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
          {emptyLibrary() ? <View style={{ ...panelSurface(theme()), gap: 8, padding: 16 }}>
            <Label theme={theme()} heading size={textSize(theme(), 20)}>{t().home_first_skill}</Label>
            <Label theme={theme()} muted>{t().home_first_skill_desc}</Label>
            <View style={wrapRow}>
              <Action theme={theme()} variant="primary" glyph="plus" label={t().btn_install_from_link} onPress={installSkill} />
              <Action theme={theme()} variant="outline" glyph="layers" label={t().home_browse_stacks}
                onPress={() => props.navigate({ page: "stacks", source: "Featured" })} />
            </View>
            {adoptLocal(data().adoptable_count)}
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
                  .replace("{total}", data().skill_count.toLocaleString(props.locale))
                  .replace("{deployed}", data().deployed_count.toLocaleString(props.locale))}</Label>
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
              {adoptLocal(data().adoptable_count)}
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
          <View style={{ flexDirection: marketColumns() ? "row" : "column", gap: 10 }}>
            {!trending() && !marketUnavailable()
              ? <View testID="home-ranking-skeleton" role="summary" accessibilityLabel={t().home_market_loading}
                  accessibilityState={{ busy: true }}
                  style={{ flexGrow: 1, flexDirection: marketColumns() ? "row" : "column", gap: 10 }}>
                  <Each of={[0, 1, 2]}>{() =>
                    <View style={{ ...panelSurface(theme()), ...marketTile, flex: marketColumns() ? 1 : undefined, gap: 8 }}>
                      <SkeletonBlock theme={theme()} width="65%" height={13} />
                      <SkeletonBlock theme={theme()} width="46%" height={10} />
                      <SkeletonBlock theme={theme()} width="30%" height={10} />
                    </View>
                  }</Each>
                </View>
              : null}
            <Each of={trending() ?? []} key={(skill) => skill.id}>{(skill) =>
              <ActionControl theme={theme()} role={skill().url ? "link" : "button"}
                accessibilityLabel={`${skill().name} · ${skill().url ? t().home_market_details : t().home_view_market}`}
                tooltip={<Hint theme={theme()} text={skill().name} />}
                style={{ ...panelSurface(theme()), ...marketTile, flex: marketColumns() ? 1 : undefined }}
                onPress={() => void openTrending(skill())}>
                <View style={{ ...row, gap: 6 }}>
                  <View style={{ flex: 1, minWidth: 0 }}>
                    <Label theme={theme()} weight="600" truncate>{skill().name}</Label>
                  </View>
                  <Icon name={skill().url ? "external-link" : "chevron-right"} color={theme().muted} size={12} />
                </View>
                {skill().source
                  ? <Label theme={theme()} muted truncate size={textSize(theme(), 12)}>{skill().source}</Label>
                  : null}
                <Label theme={theme()} muted size={textSize(theme(), 12)}>{t().home_market_installs
                  .replace("{count}", skill().installs.toLocaleString(props.locale))}</Label>
              </ActionControl>
            }</Each>
          </View>
        </View>
      </View>
    </Scroll>
  </View>;
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
