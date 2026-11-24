import { Text, View, host, mount, signal } from "nsh";
import { Button } from "nsh/base";
import type { Locale, Route, Theme, Tone } from "../../src/shared";
import { Discover } from "../../src/library/DiscoverScreen";
import { Library } from "../../src/library/LibraryScreen";

const TEST_THEME: Theme = {
  background: "#ffffff",
  surface: "#f7f7f8",
  foreground: "#111827",
  muted: "#6b7280",
  border: "#e5e7eb",
  primary: "#2563eb",
  onPrimary: "#ffffff",
  accent: "#eef2ff",
  onAccent: "#111827",
  secondary: { background: "#eef2ff", foreground: "#111827" },
  modalBackdrop: "#00000072",
  danger: "#dc2626",
  warning: "#d97706",
  success: "#16a34a",
  fontFamily: "system-ui",
  monoFamily: "monospace",
  fontSize: 14,
  radius: 8,
  ring: "#2563eb",
  control: { radius: 6, inputFill: "#ffffff", outlineFill: "#ffffff", shadow: "none" },
  switch: { offTrack: "#e5e7eb", offThumb: "#ffffff", onThumb: "#ffffff" },
  panel: { foreground: "#111827", radius: 12, shadow: "none" },
  popup: { background: "#f7f7f8", foreground: "#111827", shadow: "none" },
};

function HarnessApp() {
  const initialRoute =
    (host.params.initialRoute as unknown as Route | undefined) ?? { page: "library" };
  const initialLocale = (host.params.locale as unknown as Locale | undefined) ?? "en";

  const route = signal<Route>(initialRoute);
  const active = signal(true);
  const revision = signal(0);
  const lastNotice = signal<string>("");
  const lastNavigated = signal<string>("");

  const navigate = (next: Route) => {
    lastNavigated.set(JSON.stringify(next));
    route.set(next);
  };

  const refresh = () => {
    revision.update((value) => value + 1);
  };

  const notify = (message: string, tone: Tone = "info") => {
    lastNotice.set(`${tone}:${message}`);
  };

  host.on("test.navigate", (next) => {
    route.set(next as unknown as Route);
  });
  host.on("test.set_active", (next) => {
    active.set(Boolean(next));
  });
  host.on("test.refresh", () => {
    refresh();
  });

  const screenProps = {
    get locale() {
      return initialLocale;
    },
    get theme() {
      return TEST_THEME;
    },
    get route() {
      return route();
    },
    get active() {
      return active();
    },
    get revision() {
      return revision();
    },
    navigate,
    refresh,
    notify,
  };

  return (
    <View style={{ flex: 1, minHeight: 0, minWidth: 0 }}>
      <If when={host.params.showRefreshControl === true}>
        <Button accessibilityLabel="Refresh test screen" onPress={refresh}>
          <Text>Refresh test screen</Text>
        </Button>
      </If>
      <If when={route().page === "discover"} else={<Library {...screenProps} />}>
        <Discover {...screenProps} />
      </If>
      <If when={lastNavigated().length > 0}>
        <Text testID="test-navigated">{lastNavigated()}</Text>
      </If>
      <If when={lastNotice().length > 0}>
        <Text testID="test-notice">{lastNotice()}</Text>
      </If>
    </View>
  );
}

mount(HarnessApp);
