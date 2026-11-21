import { openUrl, signal, View, type Accessor } from "nsh";
import { Progress } from "nsh/base";
import { Action, InlineError, Label, Modal, Section } from "./controls";
import { strings } from "./i18n";
import { call, errorMessage, type Locale, type Tone, type Theme } from "./shared";

type Asset = { name: string; size: number };
type Release = { tag_name: string; html_url: string; body: string };
type Available = { latest: Release; recommended_asset: Asset | null; checksums_asset: Asset | null };
type Download =
  | { phase: "idle" | "preparing" | "verifying" }
  | { phase: "downloading"; percent: number | null }
  | { phase: "ready"; verified: boolean }
  | { phase: "failed"; error: string };
type Status = {
  version: string;
  platform: "windows_x64" | "macos_arm64" | "linux_x64" | "other";
  checking: boolean;
  available: Available | null;
  download: Download;
};

export function createUpdates(locale: Accessor<Locale>, notify: (message: string, tone?: Tone) => void) {
  const status = signal<Status | null>(null);
  const checking = signal(false);
  const downloading = signal(false);
  const open = signal(false);
  const installBusy = signal(false);
  const t = () => strings[locale()];
  const available = () => status()?.available;
  const downloadState = (): Download => status()?.download ?? { phase: "idle" };

  async function check(silent = false) {
    if (checking() || downloading()) return;
    checking.set(true);
    try {
      const outcome = await call<null, Status>("update.check", null);
      status.set(outcome);
      if (!silent) {
        notify(outcome.available
          ? `${t().badge_update_available} ${outcome.available.latest.tag_name}`
          : `${t().update_current} · v${outcome.version}`, "success");
      }
    } catch (error) {
      if (!silent) notify(errorMessage(error), "error");
    } finally {
      checking.set(false);
    }
  }

  async function download() {
    if (downloading() || checking()) return;
    downloading.set(true);
    status.update((state) => state ? { ...state, download: { phase: "preparing" } } : state);
    let current = true;
    let polling = false;
    const timer = setInterval(() => {
      if (polling) return;
      polling = true;
      call<null, Status>("update.status", null)
        .then((value) => { if (current) status.set(value); })
        .catch((error) => { if (current) console.warn(errorMessage(error)); })
        .finally(() => { polling = false; });
    }, 300);
    try {
      const value = await call<null, Status>("update.download", null);
      current = false;
      status.set(value);
    } catch (error) {
      const message = errorMessage(error);
      status.update((state) => state ? { ...state, download: { phase: "failed", error: message } } : state);
      notify(message, "error");
    } finally {
      current = false;
      clearInterval(timer);
      downloading.set(false);
    }
  }

  async function openRelease(url: string) {
    try {
      if (!await openUrl(url)) notify(t().update_could_not_open, "error");
    } catch (error) {
      notify(errorMessage(error), "error");
    }
  }

  async function install() {
    if (installBusy()) return;
    installBusy.set(true);
    try {
      const result = await call<null, { launched: true } | { launched: false; path: string; error: string }>("update.install", null);
      if (!result.launched) {
        notify(result.error, "error");
        await call<{ path: string }, void>("dialog.reveal", { path: result.path });
      }
      open.set(false);
    } catch (error) {
      notify(errorMessage(error), "error");
    } finally {
      installBusy.set(false);
    }
  }

  const installLabel = () => {
    switch (status()?.platform) {
      case "windows_x64": return t().btn_install_update_win;
      case "macos_arm64": return t().btn_install_update_mac;
      default: return t().btn_install_update_linux;
    }
  };
  const phaseLabel = () => {
    const state = downloadState();
    switch (state.phase) {
      case "idle": return `${t().badge_update_available} ${available()?.latest.tag_name ?? ""}`.trim();
      case "preparing": return t().update_preparing;
      case "downloading": return state.percent === null ? t().update_downloading : `${t().update_downloading} ${state.percent}%`;
      case "verifying": return t().update_verifying;
      case "ready": return t().update_ready;
      case "failed": return t().update_failed;
    }
  };
  return { status, checking, downloading, open, installBusy, available, downloadState,
    installLabel, phaseLabel, check, download, openRelease, install };
}

export type Updates = ReturnType<typeof createUpdates>;

type Props = { theme: Theme; updates: Updates; locale: Locale };

export function UpdatePanel(props: Props & { version: string }) {
  const t = () => strings[props.locale];
  const update = props.updates;
  return (
    <Section theme={props.theme} title={t().settings_updates}>
      <Label theme={props.theme} mono>{`${t().settings_current_version} · v${props.version}`}</Label>
      <Label theme={props.theme} muted>{t().settings_updates_desc}</Label>
      <Action theme={props.theme} label={update.checking() ? t().busy_checking_updates : t().btn_check_updates}
        disabled={update.checking() || update.downloading()} onPress={() => void update.check()} />
      {update.available() ? (
        <View style={{ gap: 8 }}>
          <Label theme={props.theme} heading>{update.phaseLabel()}</Label>
          <Label theme={props.theme} muted>{update.available()!.recommended_asset?.name ?? t().update_release_only}</Label>
          <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 8 }}>
            <Action theme={props.theme} role="link" label={t().btn_open_release}
              onPress={() => void update.openRelease(update.available()!.latest.html_url)} />
            {update.downloadState().phase === "ready" ? (
              <Action theme={props.theme} onPrimary disabled={update.installBusy()}
                label={update.installLabel()} onPress={() => void update.install()} />
            ) : update.available()!.recommended_asset ? (
              <Action theme={props.theme} onPrimary
                label={update.downloading() ? t().update_view_progress : t().update_action}
                onPress={() => update.open.set(true)} />
            ) : null}
          </View>
        </View>
      ) : null}
    </Section>
  );
}

export function UpdateModal(props: Props) {
  const t = () => strings[props.locale];
  const update = props.updates;
  const asset = () => update.available()?.recommended_asset;
  const state = update.downloadState;
  const progress = () => { const current = state(); return current.phase === "downloading" ? current.percent : null; };
  const checkFile = () => update.available()?.checksums_asset;
  const readyVerified = () => { const current = state(); return current.phase === "ready" && current.verified; };
  const failure = () => { const current = state(); return current.phase === "failed" ? current.error : ""; };
  return (
    <Modal theme={props.theme} label={t().settings_updates} open={update.open()} onClose={() => update.open.set(false)}>
      <Label theme={props.theme} heading>{update.available()?.latest.tag_name ?? ""}</Label>
      {asset() ? <Label theme={props.theme} muted mono>{`${asset()!.name} · ${(asset()!.size / 1048576).toFixed(1)} MiB`}</Label> : null}
      {update.downloading() ? (
        <Progress value={progress() ?? 0} indeterminate={progress() === null} accessibilityLabel={update.phaseLabel()}
          trackStyle={{ height: 6, borderRadius: 3, backgroundColor: props.theme.border }}
          indicatorStyle={{ height: 6, borderRadius: 3, backgroundColor: props.theme.primary }}>
          <Label theme={props.theme} muted>{update.phaseLabel()}</Label>
        </Progress>
      ) : state().phase === "failed" ? (
        <InlineError theme={props.theme} message={`${t().update_download_failed}\n${failure()}`} />
      ) : (
        <Label theme={props.theme} muted>{state().phase === "ready"
          ? readyVerified() ? t().badge_sha256_verified : t().update_unverified_hint
          : !asset() ? t().update_release_only : checkFile() ? t().update_confirm_hint : t().update_unverified_hint}
        </Label>
      )}
      <View style={{ flexDirection: "row", flexWrap: "wrap", justifyContent: "flex-end", gap: 8 }}>
        <Action theme={props.theme} label={update.downloading() ? t().update_background : t().btn_close}
          onPress={() => update.open.set(false)} />
        {!update.downloading() && update.available() ? (
          <Action theme={props.theme} role="link" label={t().btn_open_release}
            onPress={() => void update.openRelease(update.available()!.latest.html_url)} />
        ) : null}
        {state().phase === "ready" ? (
          <Action theme={props.theme} onPrimary label={update.installLabel()}
            disabled={update.installBusy()} onPress={() => void update.install()} />
        ) : !update.downloading() && asset() ? (
          <Action theme={props.theme} onPrimary disabled={update.checking()}
            label={state().phase === "failed" ? t().update_retry : checkFile() ? t().btn_download_update : t().update_start_download}
            onPress={() => void update.download()} />
        ) : null}
      </View>
    </Modal>
  );
}
