import { Image, View, announce, cleanup, computed, effect, host, keymap, setColorScheme, signal, windowSize } from "nsh";
import { Button, Tooltip } from "nsh/base";
import { titleBar } from "nsh/desktop";
import { setLocale } from "nsh/i18n";
import { Action, Icon, InlineError, Label, textSize } from "./controls";
import { Home } from "./Home";
import { strings } from "./i18n";
import { Library, Discover } from "./library";
import { Projects, Stacks } from "./projects";
import { Settings } from "./Settings";
import { call, errorMessage, pendingCalls, type Page, type Route, type ScreenProps, type Theme, type Tone } from "./shared";
import { defaults, makeTheme, type Preferences } from "./theme";
import { createUpdates, UpdateModal } from "./Updates";

const nav = [
  { page: "home", label: "nav_home", icon: "house" },
  { page: "library", label: "nav_library", icon: "book-open" },
  { page: "stacks", label: "nav_stacks", icon: "layout-dashboard" },
  { page: "discover", label: "nav_discover", icon: "globe" },
  { page: "settings", label: "nav_settings", icon: "settings" },
] as const;
const logo = typeof host.params.brand_mark === "string" ? host.params.brand_mark : "";
type Notice = { message: string; tone: Tone };

function NavigationItem(props: {
  theme: Theme; item: typeof nav[number]; label: string; selected: boolean; collapsed: boolean;
  count: number; onPress: () => void;
}) {
  const footer = () => props.item.page === "settings";
  const selection = () => footer()
    ? `color-mix(in srgb, ${props.theme.primary} 12%, transparent)`
    : props.theme.chrome?.railSelection ?? props.theme.accent;
  const color = () => footer() && props.selected ? props.theme.primary : props.theme.foreground;
  const count = () => props.count > 0 ? `+${props.count}` : undefined;
  return <Button
    accessibilityLabel={count() ? `${props.label} ${count()}` : props.label}
    selected={props.selected}
    onPress={props.onPress}
    tooltip={props.collapsed ? <Tooltip placement="right" style={{ paddingHorizontal: 8, paddingVertical: 4,
      backgroundColor: props.theme.surface, borderColor: props.theme.border, borderWidth: 1, borderRadius: props.theme.radius }}>
      <Label theme={props.theme}>{props.label}</Label>
    </Tooltip> : undefined}
    style={{ width: "100%", minWidth: 0, height: props.collapsed ? 32 : textSize(props.theme, 28),
      flexDirection: "row", alignItems: "center", justifyContent: props.collapsed ? "center" : "flex-start",
      paddingHorizontal: props.collapsed ? 0 : 8, gap: 8, borderRadius: props.theme.radius,
      borderWidth: 1, borderColor: "transparent", backgroundColor: props.selected ? selection() : "transparent" }}
    focusVisibleStyle={{ borderColor: props.theme.primary }}
    hoverStyle={{ backgroundColor: props.selected ? selection() : props.theme.chrome?.railSelection ?? props.theme.accent }}
    activeStyle={{ backgroundColor: props.theme.accent }}
  >
    <Icon name={props.item.icon} color={color()} size={textSize(props.theme, footer() ? 16 : 14)} />
    {!props.collapsed ? <>
      <View style={{ flex: 1, minWidth: 0 }}>
        <Label theme={props.theme} truncate color={color()} weight={props.selected ? "500" : "400"}>{props.label}</Label>
      </View>
      {count() ? <View style={{ borderColor: props.theme.warning, borderWidth: 1, paddingHorizontal: 4,
        borderRadius: Math.min(4, props.theme.radius), flexShrink: 0 }}>
        <Label theme={props.theme} size={textSize(props.theme, 12)} color={props.theme.warning}>{count()}</Label>
      </View> : null}
    </> : null}
  </Button>;
}

export function App() {
  const page = signal<Page>("home");
  const visited = signal<Page[]>(["home"]);
  const routePages: Page[] = [...nav.map(({ page }) => page), "projects"];
  const routes = new Map(routePages.map((target) => [target, signal<Route>({ page: target })]));
  const selectedNav = () => page() === "projects" ? "library" : page();
  const revision = signal(0);
  const preferences = signal<Preferences>(defaults);
  const notice = signal<Notice | null>(null);
  const initializationError = signal("");
  const initialized = signal(false);
  const locale = () => preferences().language;
  const t = () => strings[locale()];
  const theme = computed(() => makeTheme(preferences()));
  const narrow = () => windowSize().width < 1080;
  const collapsed = () => narrow() || preferences().sidebar_collapsed === "true";
  const adoptable = signal(0);
  let disposed = false;
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;

  function clearNoticeTimer() {
    if (noticeTimer !== undefined) clearTimeout(noticeTimer);
    noticeTimer = undefined;
  }

  function dismissNotice() {
    clearNoticeTimer();
    notice.set(null);
  }

  cleanup(() => {
    disposed = true;
    clearNoticeTimer();
  });

  function notify(message: string, tone: Tone = "info") {
    if (disposed) return;
    clearNoticeTimer();
    announce(message);
    notice.set({ message, tone });
    noticeTimer = setTimeout(dismissNotice, tone === "error" ? 10000 : 4700);
  }

  function navigate(next: Route) {
    routes.get(next.page)!.set(next);
    page.set(next.page);
    if (!visited().includes(next.page)) visited.update((pages) => [...pages, next.page]);
  }

  function openFromNavigation(target: Page) {
    const current = routes.get(target)!();
    if (current.page === "discover" && (current.tab === "install" || current.source)) {
      navigate({ page: "discover" });
      return;
    }
    navigate({ ...current, focusSearch: undefined });
  }

  function refresh() {
    revision.update((value) => value + 1);
  }

  const updates = createUpdates(locale, notify);

  async function initialize() {
    initializationError.set("");
    try {
      const saved = await call<null, Preferences>("settings.get", null);
      if (!Object.hasOwn(strings, saved.language)) saved.language = defaults.language;
      if (!["light", "dark", "system"].includes(saved.theme)) saved.theme = defaults.theme;
      preferences.set(saved);
      initialized.set(true);
      void updates.check(true);
    } catch (error) {
      initializationError.set(errorMessage(error));
    }
  }
  void initialize();

  const themeMode = computed(() => preferences().theme);
  effect(() => setColorScheme(themeMode()));
  effect(() => setLocale(locale()));

  const settingWrites = new Map<keyof Preferences, Promise<void>>();
  function setPreference(key: keyof Preferences, value: string) {
    const operation = (settingWrites.get(key) ?? Promise.resolve()).then(async () => {
      if (key === "deploy_mode") await call("settings.deploy_mode", value);
      else await call("settings.preference", { key, value });
      preferences.update((current) => ({ ...current, [key]: value }));
      if (key === "deploy_mode") refresh();
    }).catch((error) => notify(errorMessage(error), "error"));
    settingWrites.set(key, operation);
    return operation;
  }

  async function setNetwork(proxy: string, token: string) {
    await call("settings.network", { proxy, token });
    preferences.update((current) => ({ ...current, proxy, token }));
  }

  keymap.bind("mod+k", () => {
    if (page() === "home" || page() === "settings") {
      navigate({ page: "library", focusSearch: true });
    }
  }, { when: "always" });

  const propsFor = (target: Page): ScreenProps => ({
    get theme() { return theme(); },
    get locale() { return locale(); },
    get route() { return routes.get(target)!(); },
    get active() { return page() === target; },
    get revision() { return revision(); },
    notify,
    navigate,
    refresh,
  });

  function renderPage(target: Page) {
    const screen = propsFor(target);
    const active = () => page() === target;
    return (
      <View accessibilityElementsHidden={!active()}
        style={{ display: active() ? "flex" : "none", flex: 1, minHeight: 0, minWidth: 0 }}>
        {target === "library" ? <Library {...screen} />
          : target === "discover" ? <Discover {...screen} />
          : target === "projects" ? <Projects {...screen} />
          : target === "stacks" ? <Stacks {...screen} />
          : target === "settings" ? (
            <Settings {...screen} preferences={preferences()} setPreference={setPreference} setNetwork={setNetwork} updates={updates} />
          ) : <Home {...screen} onAdoptableCount={(count) => adoptable.set(count)} />}
      </View>
    );
  }
  const messageColor = () => notice()?.tone === "error" ? theme().danger
    : notice()?.tone === "success" ? theme().success : theme().foreground;
  const rail = () => theme().chrome?.rail ?? theme().surface;
  const collapseLabel = () => collapsed() ? t().expand_sidebar : t().collapse_sidebar;
  const navItem = (item: typeof nav[number]) =>
    <NavigationItem theme={theme()} item={item} label={t()[item.label]} selected={selectedNav() === item.page}
      collapsed={collapsed()} count={item.page === "discover" ? adoptable() : 0}
      onPress={() => openFromNavigation(item.page)} />;

  return (
    <View style={{ flex: 1, minHeight: 0, backgroundColor: theme().background }}>
      <View desktop={{ windowDrag: true }} style={{ height: 36, flexShrink: 0,
        paddingLeft: titleBar().left + 12, paddingRight: titleBar().right + 8,
        flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 8,
        backgroundColor: rail(), borderBottomWidth: 1, borderColor: theme().border,
        backgroundImage: `linear-gradient(180deg, color-mix(in srgb, ${rail()} 55%, ${theme().background}), ${rail()})` }}>
        <View style={{ flexDirection: "row", alignItems: "center", gap: 8, minWidth: 0 }}>
          {logo ? <Image accessibilityElementsHidden src={logo} style={{ width: 16, height: 16,
            tintColor: theme().foreground, flexShrink: 0 }} /> : null}
          <Label theme={theme()} weight="600">{"UnoSkills"}</Label>
          {!narrow() ? <Action theme={theme()} variant="ghost" height={24} glyph="panel-left" iconOnly
            label={collapseLabel()} tooltip={collapseLabel()}
            onPress={() => void setPreference("sidebar_collapsed", String(!collapsed()))} /> : null}
          {updates.available() ? <Action theme={theme()} variant="primary" height={20} glyph={updates.phaseIcon()}
            label={updates.phaseLabel()} onPress={() => updates.open.set(true)} /> : null}
        </View>
        {initialized() && pendingCalls() > 0 ?
          <View style={{ flexDirection: "row", alignItems: "center", gap: 8, minWidth: 0 }}>
            <Icon name="loader-circle" color={theme().muted} size={12} spinning />
            <Label theme={theme()} muted truncate size={textSize(theme(), 12)}>{t().working}</Label>
          </View> : null}
      </View>
      <View style={{ flex: 1, minHeight: 0, minWidth: 0, flexDirection: "row" }}>
        <View style={{ width: collapsed() ? 48 : 184, flexShrink: 0, minHeight: 0,
          paddingHorizontal: collapsed() ? 8 : 12, paddingTop: collapsed() ? 20 : 12 + textSize(theme(), 32),
          paddingBottom: 12, justifyContent: "space-between", overflow: "hidden",
          backgroundColor: rail(), borderRightWidth: 1, borderColor: theme().border }}>
          <View style={{ gap: 8 }}>
            {nav.slice(0, 4).map(navItem)}
          </View>
          {navItem(nav[4])}
        </View>
        <View style={{ flex: 1, minWidth: 0, minHeight: 0 }}>
          {initializationError() ? (
            <View style={{ gap: 12, padding: 20 }}>
              <InlineError theme={theme()} message={initializationError()} />
              <Action theme={theme()} label={t().retry} onPress={() => void initialize()} />
            </View>
          ) : !initialized() ? (
            <View style={{ padding: 20 }}><Label theme={theme()} muted>{t().loading}</Label></View>
          ) : <Each of={visited()}>{(target) => renderPage(target())}</Each>}
        </View>
      </View>
      {notice() ? (
        <View role={notice()!.tone === "error" ? "alert" : undefined}
          style={{ position: "absolute", right: 16, bottom: 16, maxWidth: "60%", flexDirection: "row",
            alignItems: "center", minHeight: 38, gap: 8, paddingHorizontal: 12, paddingVertical: 6,
            backgroundColor: theme().surface, borderWidth: 1, borderColor: theme().border, borderRadius: theme().radius }}>
          <View style={{ flex: 1, minWidth: 0 }}>
            <Label theme={theme()} color={messageColor()}>{notice()!.message}</Label>
          </View>
          <Action theme={theme()} variant="ghost" height={24} label={t().btn_close} glyph="x" iconOnly onPress={dismissNotice} />
        </View>
      ) : null}
      {updates.available() ? <UpdateModal theme={theme()} locale={locale()} updates={updates} /> : null}
    </View>
  );
}
