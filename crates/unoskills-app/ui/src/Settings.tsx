import { List, Scroll, View, computed, effect, signal } from "nsh";
import { Switch } from "nsh/base";
import { Action, Choice, FormText, InlineError, Label, Modal, Section } from "./controls";
import { strings } from "./i18n";
import { call, errorMessage, type ScreenProps } from "./shared";
import { paletteFor, palettes, type Preferences } from "./theme";
import { UpdatePanel, type Updates } from "./Updates";

type SettingsProps = ScreenProps & {
  preferences: Preferences;
  setPreference: (key: keyof Preferences, value: string) => Promise<void>;
  setNetwork: (proxy: string, token: string) => Promise<void>;
  updates: Updates;
};
type BackupStatus = {
  repo_url: string | null; branch: string; local_skill_count: number; local_stack_count: number;
  last_pushed_at: number | null; last_pulled_at: number | null;
};
type BackupPush = { pushed: boolean; skills_exported: number; stacks_exported: number };
type BackupPull = { skills_imported: number; skills_updated: number; stacks_imported: number };
type Agent = { key: string; display_name: string; installed: boolean; enabled: boolean; is_custom: boolean; skills_dir: string };

export function Settings(props: SettingsProps) {
  const t = () => strings[props.locale];
  const section = computed(() => props.route.tab ?? "general");
  const tabs = () => [
    ["general", t().settings_general], ["appearance", t().settings_appearance_tab],
    ["network", t().settings_network], ["agents", t().settings_agents_registry],
    ["updates", t().settings_updates_tab],
  ];
  return (
    <View style={{ flex: 1, minHeight: 0 }}>
      <View style={{ gap: 12, padding: 20 }}>
        <Label theme={props.theme} heading size={24}>{t().settings_title}</Label>
        <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 6 }}>
          {tabs().map(([id, label]) => (
            <Action theme={props.theme} compact selected={section() === id} label={label}
              onPress={() => props.navigate({ page: "settings", tab: id })} />
          ))}
        </View>
      </View>
      {section() === "agents" ? <Agents {...props} /> : (
        <Scroll style={{ flex: 1, minHeight: 0 }}>
          <View style={{ padding: 20, paddingTop: 0, gap: 16, maxWidth: 850, width: "100%" }}>
            {section() === "appearance" ? <Appearance {...props} />
              : section() === "network" ? <Network {...props} />
              : section() === "updates" ? (
                <UpdatePanel theme={props.theme} locale={props.locale} version={props.preferences.version}
                  updates={props.updates} />
              ) : <General {...props} />}
          </View>
        </Scroll>
      )}
    </View>
  );
}

function General(props: SettingsProps) {
  const t = () => strings[props.locale];
  async function reveal() {
    try {
      await call<{ path: string }, void>("dialog.reveal", { path: props.preferences.root_dir });
    } catch (error) { props.notify(errorMessage(error), "error"); }
  }
  return (
    <Section theme={props.theme} title={t().settings_general}>
      <Choice theme={props.theme} title={t().settings_language} detail={t().settings_language_desc}
        value={props.locale} items={[
          { value: "zh-CN", title: "简体中文" }, { value: "en", title: "English" }, { value: "ja", title: "日本語" },
        ]} onChange={(value) => void props.setPreference("language", value)} />
      <Choice theme={props.theme} title={t().settings_deploy_mode} detail={t().settings_deploy_mode_desc}
        value={props.preferences.deploy_mode} items={[
          { value: "symlink", title: t().mode_symlink }, { value: "copy", title: t().mode_copy },
        ]} onChange={(value) => void props.setPreference("deploy_mode", value)} />
      <View style={{ gap: 4 }}>
        <Label theme={props.theme}>{t().settings_root_dir}</Label>
        <Label theme={props.theme} muted mono>{props.preferences.root_dir}</Label>
        <Action theme={props.theme} label={t().tooltip_reveal_folder} disabled={!props.preferences.root_dir}
          onPress={() => void reveal()} />
      </View>
    </Section>
  );
}

function Appearance(props: SettingsProps) {
  const t = () => strings[props.locale];
  const fonts = signal<string[]>([]);
  call<null, string[]>("settings.fonts", null).then((value) => fonts.set(value))
    .catch((error) => props.notify(errorMessage(error), "error"));
  const paletteItems = () => palettes.map((value, index) => ({
    value, title: [t().palette_claude, t().palette_vercel, t().palette_supabase, t().palette_catppuccin, t().palette_ocean, "UnoSkills"][index],
  }));
  const fontItems = (label: string, current: string) => [
    { title: label, value: "auto" },
    ...Array.from(new Set([...fonts(), ...(current === "auto" ? [] : [current])]))
      .map((value) => ({ title: value, value })),
  ];
  const options = (values: number[]) => values.map((value) => ({ value: String(value), title: String(value) }));
  return (
    <View style={{ gap: 16 }}>
      <Section theme={props.theme} title={t().settings_appearance}>
        <Choice theme={props.theme} title={t().settings_appearance} detail={t().settings_appearance_desc}
          value={props.preferences.theme} items={[
            { value: "system", title: t().theme_system }, { value: "light", title: t().theme_light },
            { value: "dark", title: t().theme_dark },
          ]} onChange={(value) => void props.setPreference("theme", value)} />
        <Choice theme={props.theme} title={t().settings_accent_palette} detail={t().settings_accent_palette_desc}
          value={paletteFor(props.preferences.accent_palette)} items={paletteItems()}
          onChange={(value) => void props.setPreference("accent_palette", value)} />
        <Choice theme={props.theme} title={t().settings_corner_radius} detail={t().settings_corner_radius_desc}
          value={props.preferences.corner_radius} items={options([0, 4, 6, 8, 12])}
          onChange={(value) => void props.setPreference("corner_radius", value)} />
      </Section>
      <Section theme={props.theme} title={t().settings_fonts}>
        <Choice theme={props.theme} title={t().settings_ui_font} detail={t().settings_ui_font_desc}
          value={props.preferences.ui_font} items={fontItems(t().font_auto_by_lang, props.preferences.ui_font)}
          searchable searchLabel={t().search_fonts} onChange={(value) => void props.setPreference("ui_font", value)} />
        <Choice theme={props.theme} title={t().settings_mono_font} detail={t().settings_mono_font_desc}
          value={props.preferences.mono_font} items={fontItems(t().font_auto_mono, props.preferences.mono_font)}
          searchable searchLabel={t().search_fonts} onChange={(value) => void props.setPreference("mono_font", value)} />
        <Choice theme={props.theme} title={t().settings_font_size} detail={t().settings_font_size_desc}
          value={props.preferences.font_size} items={options([13, 14, 15, 16])}
          onChange={(value) => void props.setPreference("font_size", value)} />
      </Section>
    </View>
  );
}

function Network(props: SettingsProps) {
  const t = () => strings[props.locale];
  const proxy = signal(props.preferences.proxy);
  const token = signal(props.preferences.token);
  const networkBusy = signal(false);
  const backupBusy = signal<"save" | "push" | "pull" | null>(null);
  const status = signal<BackupStatus | null>(null);
  const repo = signal("");
  const branch = signal("");
  const statusError = signal("");
  async function backupStatus(seed = false) {
    try {
      const value = await call<null, BackupStatus>("settings.backup_status", null);
      status.set(value);
      statusError.set("");
      if (seed) {
        repo.set(value.repo_url ?? "");
        branch.set(value.branch);
      }
    } catch (error) {
      statusError.set(errorMessage(error));
    }
  }
  void backupStatus(true);

  async function network() {
    if (networkBusy()) return;
    networkBusy.set(true);
    try {
      await props.setNetwork(proxy(), token());
      props.notify(t().toast_network_saved, "success");
    } catch (error) {
      props.notify(errorMessage(error), "error");
    } finally {
      networkBusy.set(false);
    }
  }

  async function backup(action: "save" | "push" | "pull") {
    if (backupBusy()) return;
    backupBusy.set(action);
    const config = { repo_url: repo(), branch: branch() };
    try {
      if (action === "push") {
        const result = await call<typeof config, BackupPush>("settings.backup_push", config);
        props.notify(result.pushed
          ? `${t().btn_backup_push} · ${result.skills_exported} ${t().skills_unit} · ${result.stacks_exported} ${t().nav_stacks}`
          : t().toast_backup_noop, "success");
      } else if (action === "pull") {
        const result = await call<typeof config, BackupPull>("settings.backup_pull", config);
        props.notify(`${t().btn_backup_pull} · ${result.skills_imported + result.skills_updated} ${t().skills_unit} · ${result.stacks_imported} ${t().nav_stacks}`, "success");
        props.refresh();
      } else {
        await call("settings.backup_save", config);
        props.notify(t().toast_backup_saved, "success");
      }
      await backupStatus(true);
    } catch (error) {
      props.notify(errorMessage(error), "error");
    } finally {
      backupBusy.set(null);
    }
  }

  return (
    <View style={{ gap: 16 }}>
      <Section theme={props.theme} title={t().settings_network}>
        <FormText theme={props.theme} title={t().settings_http_proxy} detail={t().settings_http_proxy_desc}
          placeholder={t().ph_http_proxy} value={proxy()} onChange={(value) => proxy.set(value)} />
        <FormText theme={props.theme} title={t().settings_github_token} detail={t().settings_github_token_desc}
          placeholder={t().ph_github_token} masked value={token()} onChange={(value) => token.set(value)} />
        <Action theme={props.theme} label={networkBusy() ? t().saving : t().btn_save}
          disabled={networkBusy()} onPress={() => void network()} />
      </Section>
      <Section theme={props.theme} title={t().settings_git_backup}>
        <FormText theme={props.theme} title={t().settings_git_backup_repo} detail={t().settings_git_backup_repo_desc}
          placeholder={t().ph_git_backup_repo} value={repo()} onChange={(value) => repo.set(value)} />
        <FormText theme={props.theme} title={t().settings_git_backup_branch} detail={t().settings_git_backup_branch_desc}
          placeholder="main" value={branch()} onChange={(value) => branch.set(value)} />
        {statusError() ? <InlineError theme={props.theme} message={statusError()} /> : null}
        {status() ? (
          <Label theme={props.theme} muted>
            {`${status()!.local_skill_count} ${t().skills_unit} · ${status()!.local_stack_count} ${t().nav_stacks}`}
          </Label>
        ) : null}
        <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 8 }}>
          <Action theme={props.theme} label={backupBusy() === "save" ? t().saving : t().btn_save}
            disabled={backupBusy() !== null} onPress={() => void backup("save")} />
          <Action theme={props.theme} label={backupBusy() === "pull" ? t().busy_backup_pulling : t().btn_backup_pull}
            disabled={backupBusy() !== null || !repo().trim()} onPress={() => void backup("pull")} />
          <Action theme={props.theme} label={backupBusy() === "push" ? t().busy_backup_pushing : t().btn_backup_push}
            disabled={backupBusy() !== null || !repo().trim()} onPress={() => void backup("push")} />
        </View>
      </Section>
    </View>
  );
}

function Agents(props: SettingsProps) {
  const t = () => strings[props.locale];
  const agents = signal<Agent[]>([]);
  const showUndetected = signal(false);
  const open = signal(false);
  const name = signal("");
  const directory = signal("");
  const busy = signal(false);
  const loading = signal(true);
  const formError = signal("");
  const listError = signal("");
  const detected = () => agents().filter((agent) => agent.installed);
  const undetectedCount = () => agents().length - detected().length;
  const visible = () => showUndetected() ? [...detected(), ...agents().filter((agent) => !agent.installed)] : detected();

  effect(() => {
    void props.revision;
    let current = true;
    loading.set(true);
    call<null, Agent[]>("settings.agents", null)
      .then((value) => { if (current) { agents.set(value); listError.set(""); } })
      .catch((error) => { if (current) listError.set(errorMessage(error)); })
      .finally(() => { if (current) loading.set(false); });
    return () => { current = false; };
  });

  async function chooseDirectory() {
    try {
      const path = await call<{ title: string }, string | null>("dialog.folder", { title: t().prompt_pick_agent_dir });
      if (path) directory.set(path);
    } catch (error) { formError.set(errorMessage(error)); }
  }

  async function mutate(command: string, input: object) {
    if (busy()) return;
    busy.set(true);
    formError.set("");
    try {
      await call(command, input);
      if (command === "settings.agent_add") open.set(false);
      props.refresh();
    } catch (error) {
      if (open()) formError.set(errorMessage(error));
      else props.notify(errorMessage(error), "error");
    } finally {
      busy.set(false);
    }
  }

  function add() {
    if (!name().trim() || !directory().trim()) {
      formError.set(t().toast_custom_agent_required);
      return;
    }
    void mutate("settings.agent_add", { name: name(), skills_dir: directory() });
  }

  return (
    <View style={{ flex: 1, minHeight: 0, paddingHorizontal: 20, paddingBottom: 20, gap: 12 }}>
      <View style={{ flexDirection: "row", flexWrap: "wrap", alignItems: "center", gap: 8 }}>
        <Label theme={props.theme} heading>{`${t().badge_detected} (${detected().length})`}</Label>
        <Action theme={props.theme} label={t().settings_add_custom_agent}
          onPress={() => { name.set(""); directory.set(""); formError.set(""); open.set(true); }} />
        {undetectedCount() > 0 ? (
          <Action theme={props.theme} selected={showUndetected()}
            label={`${t().badge_not_detected} (${undetectedCount()})`}
            onPress={() => showUndetected.update((shown) => !shown)} />
        ) : null}
      </View>
      {listError() ? <InlineError theme={props.theme} message={listError()} /> : null}
      {loading() ? <Label theme={props.theme} muted>{t().loading}</Label> : null}
      <List each={visible()} style={{ flex: 1, minHeight: 0 }} estimatedItemSize={58} keyExtractor={(agent) => agent.key}>
        {(item) => (
          <View style={{ flexDirection: "row", gap: 8, alignItems: "center", minHeight: 58,
            paddingHorizontal: 12, paddingVertical: 6, borderBottomWidth: 1, borderColor: props.theme.border,
            backgroundColor: props.theme.surface }}>
            <View style={{ flex: 1, minWidth: 0, gap: 3 }}>
              <Label theme={props.theme} truncate>{item().display_name}</Label>
              <Label theme={props.theme} muted mono truncate>{item().skills_dir}</Label>
            </View>
            {item().is_custom ? <Action theme={props.theme} compact danger label={t().btn_delete}
              disabled={busy()} onPress={() => void mutate("settings.agent_remove", { key: item().key })} /> : null}
            <Switch accessibilityLabel={item().display_name} checked={item().enabled} disabled={busy()}
              onChange={({ checked }) => void mutate("settings.agent_enable", { key: item().key, enabled: checked })}
              trackClass="w-11 rounded-full p-0.5"
              thumbClass="size-5 rounded-full checked:translate-x-5"
              trackStyle={{ backgroundColor: item().enabled ? props.theme.primary : props.theme.border }}
              thumbStyle={{ backgroundColor: item().enabled ? props.theme.onPrimary : props.theme.background }}
            />
          </View>
        )}
      </List>
      <Modal theme={props.theme} label={t().settings_add_custom_agent} open={open()}
        onClose={() => { if (!busy()) open.set(false); }}>
        <FormText theme={props.theme} title={t().custom_agent_name} placeholder={t().ph_custom_agent_name}
          value={name()} onChange={(value) => name.set(value)} autoFocus />
        <FormText theme={props.theme} title={t().custom_agent_directory} placeholder={t().ph_custom_agent_dir}
          value={directory()} onChange={(value) => directory.set(value)} onSubmit={add} />
        <Action theme={props.theme} label={t().btn_browse_folder} onPress={() => void chooseDirectory()} />
        {formError() ? <InlineError theme={props.theme} message={formError()} /> : null}
        <View style={{ flexDirection: "row", justifyContent: "flex-end", gap: 8 }}>
          <Action theme={props.theme} label={t().btn_cancel} disabled={busy()} onPress={() => open.set(false)} />
          <Action theme={props.theme} label={busy() ? t().saving : t().btn_add_agent}
            disabled={busy()} onPrimary onPress={add} />
        </View>
      </Modal>
    </View>
  );
}
