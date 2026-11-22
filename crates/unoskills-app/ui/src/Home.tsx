import { Scroll, View, effect, signal, type JSX } from "nsh";
import { Action, ActionControl, Hint, Icon, InlineError, Label, PageHeader, SkeletonBlock, textSize } from "./components";
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
type TrendingSkill = { id: string; name: string };

const row = { flexDirection: "row", alignItems: "center" } as const;
const wrapRow = { ...row, flexWrap: "wrap", columnGap: 10, rowGap: 6 } as const;

export function Home(props: ScreenProps & { onAdoptableCount: (count: number) => void }) {
  const t = () => strings[props.locale];
  const theme = () => props.theme;
  const summary = signal<Summary | null>(null);
  const summaryError = signal("");
  const trending = signal<TrendingSkill[] | null>(null);
  const marketUnavailable = signal(false);
  const attaching = signal(false);
  const emptyLibrary = () => summary()?.skill_count === 0;
  const installSkill = () => props.navigate({ page: "discover", tab: "install" });

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
          {emptyLibrary() ? <View style={{ gap: 8, paddingBottom: 16, borderBottomWidth: 1, borderColor: theme().border }}>
            <Label theme={theme()} heading size={textSize(theme(), 20)}>{t().home_first_skill}</Label>
            <Label theme={theme()} muted>{t().home_first_skill_desc}</Label>
            <View style={wrapRow}>
              <Action theme={theme()} variant="primary" glyph="plus" label={t().btn_install_from_link} onPress={installSkill} />
              <Action theme={theme()} variant="outline" glyph="layers" label={t().home_browse_stacks}
                onPress={() => props.navigate({ page: "stacks", source: "Featured" })} />
            </View>
            {adoptLocal(data().adoptable_count)}
          </View> : null}

          <View style={{ gap: 4 }}>
            <HomeHeading theme={theme()} title={data().projects.length ? t().home_projects : t().home_projects_empty_title}>
              <Action theme={theme()} variant="ghost" size="compact" glyph="plus" label={t().btn_add_project}
                pending={attaching()} onPress={attachProject} />
            </HomeHeading>
            {data().projects.length === 0
              ? <Label theme={theme()} muted>{t().home_projects_empty_desc}</Label>
              : null}
            <Each of={data().projects} key={(project) => project.id}>{(project) =>
              <HomeRow theme={theme()} glyph="folder-open" title={project().name} detail={project().path} path
                onPress={() => props.navigate({ page: "projects", projectId: project().id, scopePath: "" })} />
            }</Each>
          </View>

          {!emptyLibrary() ? <View style={{ gap: 6, paddingTop: 14, borderTopWidth: 1, borderColor: theme().border }}>
            <HomeHeading theme={theme()} title={t().nav_library} detail={t().home_library_count
              .replace("{total}", data().skill_count.toLocaleString(props.locale))
              .replace("{deployed}", data().deployed_count.toLocaleString(props.locale))}>
              <Action theme={theme()} variant="ghost" size="compact" glyph="arrow-right" label={t().home_open_library}
                onPress={() => props.navigate({ page: "library" })} />
            </HomeHeading>
            <Label theme={theme()} muted size={textSize(theme(), 12)}>{t().home_recent}</Label>
            <View>
              <Each of={data().recent} key={(skill) => skill.id}>{(skill) =>
                <HomeRow theme={theme()} glyph="file-text" title={skill().name} detail={skill().date ?? undefined}
                  onPress={() => props.navigate({ page: "library", skillId: skill().id })} />
              }</Each>
            </View>
            {adoptLocal(data().adoptable_count)}
          </View> : null}
        </>}</If>

        <View style={{ gap: 4, paddingTop: 12, borderTopWidth: 1, borderColor: theme().border }}>
          <HomeHeading theme={theme()} title={t().home_market_trending} quiet>
            <Action theme={theme()} variant="ghost" size="compact" glyph="arrow-right" label={t().home_view_market}
              onPress={() => props.navigate({ page: "discover", tab: "trending" })} />
          </HomeHeading>
          {marketUnavailable() ? <Label theme={theme()} muted>{t().home_market_unavailable}</Label> : null}
          {trending()?.length === 0 ? <Label theme={theme()} muted>{t().home_market_empty}</Label> : null}
          {!trending() && !marketUnavailable()
            ? <View testID="home-ranking-skeleton" role="summary" accessibilityLabel={t().home_market_loading}
                accessibilityState={{ busy: true }} style={{ ...wrapRow, columnGap: 18, minHeight: 21 }}>
                <SkeletonBlock theme={theme()} width={136} height={10} />
                <SkeletonBlock theme={theme()} width={116} height={10} />
                <SkeletonBlock theme={theme()} width={144} height={10} />
              </View>
            : null}
          <View style={{ ...wrapRow, columnGap: 18 }}>
            <Each of={trending() ?? []} key={(skill) => skill.id}>{(skill) =>
              <Label theme={theme()} muted size={textSize(theme(), 13)}>{skill().name}</Label>
            }</Each>
          </View>
        </View>
      </View>
    </Scroll>
  </View>;
}

function HomeHeading(props: {
  theme: Theme; title: string; detail?: string; quiet?: boolean; children?: JSX.Element;
}) {
  return <View style={{ ...wrapRow, justifyContent: "space-between", minHeight: 30 }}>
    <View style={{ ...wrapRow, flexGrow: 1, flexShrink: 1 }}>
      <Label theme={props.theme} heading size={textSize(props.theme, props.quiet ? 14 : 18)}>{props.title}</Label>
      {props.detail
        ? <Label theme={props.theme} muted size={textSize(props.theme, 13)}>{props.detail}</Label>
        : null}
    </View>
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
      style={{ ...row, minHeight: 34, paddingHorizontal: 8, gap: 8, borderWidth: 1,
        borderColor: "transparent", borderRadius: props.theme.control.radius }}>
      <Icon name={props.glyph} color={props.theme.muted} />
      <View style={{ flex: 1, minWidth: 0 }}><Label theme={props.theme} truncate>{props.title}</Label></View>
      {props.detail
        ? <View style={{ maxWidth: "54%", flexShrink: 1, minWidth: 0 }}>
            <Label theme={props.theme} muted mono={props.path} truncate size={textSize(props.theme, 12)}>{detail()}</Label>
          </View>
        : null}
      <Icon name="chevron-right" color={props.theme.muted} size={12} />
    </ActionControl>
  </View>;
}
