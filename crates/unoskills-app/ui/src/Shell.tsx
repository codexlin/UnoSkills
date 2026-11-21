import { Image, View, announce, computed, effect, host, keymap, setColorScheme, signal, windowSize } from "nsh";
import { setLocale } from "nsh/i18n";
import { Action, InlineError, Label } from "./controls";
import { Home } from "./Home";
import { strings } from "./i18n";
import { Library, Discover } from "./library";
import { Projects, Stacks } from "./projects";
import { Settings } from "./Settings";
import { call, errorMessage, pendingCalls, type Page, type Route, type ScreenProps, type Tone } from "./shared";
import { defaults, makeTheme, type Preferences } from "./theme";
import { createUpdates, UpdateModal } from "./Updates";

const nav = [
  { page: "home", label: "nav_home", icon: "⌂" },
  { page: "library", label: "nav_library", icon: "▤" },
  { page: "projects", label: "nav_projects", icon: "▣" },
  { page: "stacks", label: "nav_stacks", icon: "▱" },
  { page: "discover", label: "nav_discover", icon: "◇" },
  { page: "settings", label: "nav_settings", icon: "⚙" },
] as const;
const logo = typeof host.params.brand_mark === "string" ? host.params.brand_mark : "";
type Notice = { message: string; tone: Tone };

export function App() {
  const route = signal<Route>({ page: "home" });
  const page = computed(() => route().page);
  const revision = signal(0);
  const preferences = signal<Preferences>(defaults);
  const notice = signal<Notice | null>(null);
  const initializationError = signal("");
  const initialized = signal(false);
  const locale = () => preferences().language;
  const t = () => strings[locale()];
  const theme = computed(() => makeTheme(preferences()));
  const collapsed = () => windowSize().width < 1080 || preferences().sidebar_collapsed === "true";
  const lastRoutes = new Map<Page, Route>();
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;

  function notify(message: string, tone: Tone = "info") {
    if (noticeTimer !== undefined) clearTimeout(noticeTimer);
    announce(message);
    notice.set({ message, tone });
    noticeTimer = setTimeout(() => notice.set(null), tone === "error" ? 10000 : 4700);
  }

  function navigate(next: Route) {
    const remembered = { ...next };
    delete remembered.focusSearch;
    lastRoutes.set(next.page, remembered);
    route.set(next);
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

  const screenProps: ScreenProps = {
    get theme() { return theme(); },
    get locale() { return locale(); },
    get route() { return route(); },
    get revision() { return revision(); },
    notify,
    navigate,
    refresh,
  };
  const messageColor = () => notice()?.tone === "error" ? theme().danger
    : notice()?.tone === "success" ? theme().success : theme().foreground;

  return (
    <View style={{ flex: 1, minHeight: 0, backgroundColor: theme().background }}>
      <View style={{ minHeight: 52, paddingHorizontal: 12, gap: 8,
        flexDirection: "row", alignItems: "center", backgroundColor: theme().surface,
        borderBottomWidth: 1, borderColor: theme().border }}>
        {logo ? (
          <View accessibilityElementsHidden style={{ width: 29, height: 29, alignItems: "center",
            justifyContent: "center", borderRadius: theme().radius, backgroundColor: theme().primary }}>
            <Image src={logo} style={{ width: 23, height: 23, tintColor: theme().onPrimary }} />
          </View>
        ) : null}
        <Label theme={theme()} heading>{"UnoSkills"}</Label>
        {windowSize().width >= 1080 ? (
          <Action theme={theme()} compact label={collapsed() ? t().expand_sidebar : t().collapse_sidebar}
            icon="◫" onPress={() => void setPreference("sidebar_collapsed", String(!collapsed()))} />
        ) : null}
        <View style={{ flex: 1, minWidth: 0 }}>
          {initialized() && pendingCalls() > 0 ? <Label theme={theme()} muted size={12}>{t().working}</Label> : null}
        </View>
        {updates.available() ? (
          <Action theme={theme()} compact onPrimary label={updates.phaseLabel()} onPress={() => updates.open.set(true)} />
        ) : null}
        <Action theme={theme()} compact label={t().tooltip_refresh} icon="↻" onPress={refresh} />
      </View>
      <View style={{ flex: 1, minHeight: 0, minWidth: 0, flexDirection: "row" }}>
        <View style={{ width: collapsed() ? 52 : 160, flexShrink: 0, padding: 8, gap: 6,
          backgroundColor: theme().surface, borderRightWidth: 1, borderColor: theme().border }}>
          {nav.map(({ page, label, icon }) => (
            <View style={page === "settings" ? { flexGrow: 1, justifyContent: "flex-end" } : {}}>
              <Action theme={theme()} selected={route().page === page} label={t()[label]} compact
                icon={collapsed() ? icon : undefined}
                onPress={() => navigate(lastRoutes.get(page) ?? { page })} />
            </View>
          ))}
        </View>
        <View style={{ flex: 1, minWidth: 0, minHeight: 0 }}>
          {initializationError() ? (
            <View style={{ gap: 12, padding: 20 }}>
              <InlineError theme={theme()} message={initializationError()} />
              <Action theme={theme()} label={t().retry} onPress={() => void initialize()} />
            </View>
          ) : !initialized() ? (
            <View style={{ padding: 20 }}><Label theme={theme()} muted>{t().loading}</Label></View>
          ) : page() === "library" ? <Library {...screenProps} />
            : page() === "discover" ? <Discover {...screenProps} />
            : page() === "projects" ? <Projects {...screenProps} />
            : page() === "stacks" ? <Stacks {...screenProps} />
            : page() === "settings" ? (
              <Settings {...screenProps} preferences={preferences()} setPreference={setPreference} setNetwork={setNetwork} updates={updates} />
            ) : <Home {...screenProps} />}
        </View>
      </View>
      {notice() ? (
        <View role={notice()!.tone === "error" ? "alert" : undefined}
          style={{ flexDirection: "row", alignItems: "center", minHeight: 38, gap: 8, paddingHorizontal: 12,
            paddingVertical: 6, backgroundColor: theme().surface, borderTopWidth: 1, borderColor: theme().border }}>
          <View style={{ flex: 1, minWidth: 0 }}>
            <Label theme={theme()} color={messageColor()}>{notice()!.message}</Label>
          </View>
          <Action theme={theme()} compact label={t().btn_close} icon="×" onPress={() => notice.set(null)} />
        </View>
      ) : null}
      {updates.available() ? <UpdateModal theme={theme()} locale={locale()} updates={updates} /> : null}
    </View>
  );
}
