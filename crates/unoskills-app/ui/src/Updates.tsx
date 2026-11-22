import { openUrl, signal, View, type Accessor } from "nsh";
import { Action, InlineError, Label, Modal, ProgressBar, panelSurface, textSize } from "./components";
import type { IconName } from "./icons";
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

function sameDownload(previous: Download, next: Download) {
  if (previous.phase !== next.phase) return false;
  if (previous.phase === "downloading" && next.phase === "downloading") return previous.percent === next.percent;
  if (previous.phase === "ready" && next.phase === "ready") return previous.verified === next.verified;
  if (previous.phase === "failed" && next.phase === "failed") return previous.error === next.error;
  return true;
}

export function createUpdates(locale: Accessor<Locale>, notify: (message: string, tone?: Tone) => void) {
  // A progress poll should only update the existing release's progress.
  const releaseStatus = signal<Pick<Status, "available" | "platform"> | null>(null);
  const downloadState = signal<Download>({ phase: "idle" }, { equals: sameDownload });
  const checking = signal(false);
  const downloading = signal(false);
  const open = signal(false);
  const installBusy = signal(false);
  const t = () => strings[locale()];
  const available = () => releaseStatus()?.available;

  async function check(silent = false) {
    if (checking() || downloading()) return;
    checking.set(true);
    try {
      const outcome = await call<null, Status>("update.check", null);
      releaseStatus.set({ available: outcome.available, platform: outcome.platform });
      downloadState.set(outcome.download);
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
    downloadState.set({ phase: "preparing" });
    let current = true;
    let polling = false;
    const timer = setInterval(() => {
      if (polling) return;
      polling = true;
      call<null, Status>("update.status", null)
        .then(({ download }) => {
          if (current && (download.phase === "preparing" || download.phase === "downloading" || download.phase === "verifying")) {
            downloadState.set(download);
          }
        })
        .catch((error) => { if (current) console.warn(errorMessage(error)); })
        .finally(() => { polling = false; });
    }, 300);
    try {
      const value = await call<null, Status>("update.download", null);
      current = false;
      downloadState.set(value.download);
    } catch (error) {
      const message = errorMessage(error);
      downloadState.set({ phase: "failed", error: message });
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
    switch (releaseStatus()?.platform) {
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
  const phaseIcon = (): IconName => {
    switch (downloadState().phase) {
      case "idle": return "arrow-up";
      case "ready": return "check";
      case "failed": return "refresh-cw";
      default: return "arrow-down";
    }
  };
  return { checking, downloading, open, installBusy, available, downloadState,
    installLabel, phaseLabel, phaseIcon, check, download, openRelease, install };
}

export type Updates = ReturnType<typeof createUpdates>;

type Props = { theme: Theme; updates: Updates; locale: Locale };

export function UpdatePanel(props: Props & { version: string }) {
  const t = () => strings[props.locale];
  const update = props.updates;
  const release = () => update.available()?.latest;
  return (
    <View style={{ gap: 8 }}>
      <Label theme={props.theme} muted heading>{t().settings_updates}</Label>
      <View style={{ ...panelSurface(props.theme), gap: 12, padding: 16 }}>
        <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 16, flexWrap: "wrap" }}>
          <View style={{ flexGrow: 1, flexBasis: 220, minWidth: 0, gap: 4 }}>
            <View style={{ flexDirection: "row", flexWrap: "wrap", alignItems: "center", gap: 8 }}>
              <Label theme={props.theme} color={props.theme.panel.foreground} weight="500">{t().settings_current_version}</Label>
              <View style={{ paddingHorizontal: 6, paddingVertical: 2, borderRadius: Math.min(props.theme.radius, 4),
                backgroundColor: props.theme.chrome?.hover ?? props.theme.accent }}>
                <Label theme={props.theme} size={textSize(props.theme, 12)}>{`v${props.version}`}</Label>
              </View>
            </View>
            <Label theme={props.theme} muted size={textSize(props.theme, 12)}>{t().settings_updates_desc}</Label>
          </View>
          <Action theme={props.theme} variant="outline" size="regular" glyph="refresh-cw"
            label={update.checking() ? t().busy_checking_updates : t().btn_check_updates}
            disabled={update.checking() || update.downloading()} onPress={() => void update.check()} />
        </View>
        {release() ? <>
          <View style={{ height: 1, backgroundColor: props.theme.border }} />
          <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap",
            gap: 12, padding: 12, borderWidth: 1, borderColor: props.theme.border, borderRadius: props.theme.radius,
            backgroundColor: `color-mix(in srgb, ${props.theme.chrome?.hover ?? props.theme.accent} 40%, transparent)` }}>
            <View style={{ flex: 1, minWidth: 0, gap: 4 }}>
              <Label theme={props.theme} color={props.theme.primary} weight="600">{update.phaseLabel()}</Label>
              <Label theme={props.theme} muted truncate size={textSize(props.theme, 12)}>
                {update.available()?.recommended_asset?.name ?? release()!.tag_name}
              </Label>
            </View>
            <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 8 }}>
              {release()!.html_url ? <Action theme={props.theme} variant="outline" role="link" size="regular" glyph="external-link"
                label={t().btn_open_release} onPress={() => void update.openRelease(release()!.html_url)} /> : null}
              {update.downloadState().phase === "ready" ? <Action theme={props.theme} variant="primary" size="regular" glyph="check"
                label={update.installLabel()} disabled={update.installBusy()} onPress={() => void update.install()} />
                : update.available()?.recommended_asset ? <Action theme={props.theme} variant="primary" size="regular" glyph="arrow-down"
                  label={update.downloading() ? t().update_view_progress : t().update_action} onPress={() => update.open.set(true)} /> : null}
            </View>
          </View>
        </> : null}
      </View>
    </View>
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
  const release = () => update.available()?.latest;
  const dismiss = () => update.open.set(false);
  return (
    <Modal theme={props.theme} label={t().settings_updates} width={420} open={update.open()} onClose={dismiss}>
      <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
        <Label theme={props.theme} weight="600" size={textSize(props.theme, 18)}>{release()?.tag_name ?? ""}</Label>
        {asset() && asset()!.size > 0 ? <Label theme={props.theme} muted>
          {`${(asset()!.size / 1048576).toLocaleString(props.locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} MiB`}
        </Label> : null}
      </View>
      {update.downloading() ? (
        <View style={{ gap: 8 }}>
          <Label theme={props.theme}>{update.phaseLabel()}</Label>
          <ProgressBar theme={props.theme} value={progress()} label={update.phaseLabel()} animateIndeterminate={false} />
        </View>
      ) : state().phase === "failed" ? (
        <InlineError theme={props.theme} message={`${t().update_download_failed}\n${failure()}`} />
      ) : (
        <Label theme={props.theme} muted>{state().phase === "ready"
          ? readyVerified() ? t().badge_sha256_verified : t().update_unverified_hint
          : !asset() ? t().update_release_only : checkFile() ? t().update_confirm_hint : t().update_unverified_hint}
        </Label>
      )}
      <View style={{ flexDirection: "row", flexWrap: "wrap", justifyContent: "flex-end", gap: 8 }}>
        {update.downloading() || state().phase === "ready" || (state().phase === "idle" && asset()) ? (
          <Action theme={props.theme} variant="outline" size="regular"
            label={update.downloading() ? t().update_background : state().phase === "ready" ? t().btn_close : t().btn_cancel}
            onPress={dismiss} />
        ) : null}
        {!update.downloading() && release() && (!asset() || state().phase === "failed") ? (
          <Action theme={props.theme} variant="ghost" role="link" size="regular" label={t().btn_open_release}
            disabled={!release()!.html_url} onPress={() => void update.openRelease(release()!.html_url)} />
        ) : null}
        {state().phase === "ready" ? (
          <Action theme={props.theme} variant="primary" size="regular" label={update.installLabel()}
            disabled={update.installBusy()} onPress={() => void update.install()} />
        ) : !update.downloading() && asset() ? (
          <Action theme={props.theme} variant="primary" size="regular" glyph="arrow-down" disabled={update.checking()}
            label={state().phase === "failed" ? t().update_retry : checkFile() ? t().btn_download_update : t().update_start_download}
            onPress={() => void update.download()} />
        ) : null}
      </View>
    </Modal>
  );
}
