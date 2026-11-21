import { List, Scroll, View, computed, effect, signal, type JSX } from "nsh";
import { Collapsible, Switch, Toggle, ToggleGroup } from "nsh/base";
import { Action, ActionControl, FormText, Icon, InlineError, Label, Modal, PageHeader, focusOutline, interactionStyles, textSize } from "./controls";
import { Choice } from "./popups";
import { strings } from "./i18n";
import { call, errorMessage, type ScreenProps, type Theme } from "./shared";
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
    { id: "general", label: t().settings_general, icon: "sliders-horizontal" },
    { id: "appearance", label: t().settings_appearance_tab, icon: "palette" },
    { id: "network", label: t().settings_network, icon: "cloud" },
    { id: "agents", label: t().settings_agents_registry, icon: "bot" },
    { id: "updates", label: t().settings_updates_tab, icon: "info" },
  ] as const;

  return (
    <View class="flex-1 min-h-0">
      <PageHeader theme={props.theme} title={t().settings_title} />
      <View class="flex-1 min-h-0 flex-row">
        <View class="shrink-0 gap-1 p-3" style={{ width: 184, borderRightWidth: 1, borderColor: props.theme.border }}>
          <Each of={tabs()} key={(tab) => tab.id}>
            {(tab) => (
              <ActionControl theme={props.theme} accessibilityLabel={tab().label} selected={section() === tab().id}
                class="w-full min-h-8 flex-row items-center gap-2 px-2"
                style={{ borderRadius: props.theme.radius, borderWidth: 1,
                  borderColor: section() === tab().id ? props.theme.primary : "transparent",
                  backgroundColor: section() === tab().id ? props.theme.accent : undefined }}
                onPress={() => props.navigate({ page: "settings", tab: tab().id })}>
                <Icon name={tab().icon} size={16} color={section() === tab().id ? props.theme.primary : props.theme.muted} />
                <Label theme={props.theme} truncate weight={section() === tab().id ? "600" : "400"}>{tab().label}</Label>
              </ActionControl>
            )}
          </Each>
        </View>
        {section() === "agents" ? <Agents {...props} /> : (
          <Scroll class="flex-1 min-h-0 min-w-0">
            <View class="w-full gap-4 p-5" style={{ maxWidth: 720 }}>
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
    </View>
  );
}

type SettingOption = { value: string; title: string; glyph?: "monitor" | "sun" | "moon" };
type SettingField = { theme: Theme; title: string; detail: string; value: string; items: SettingOption[]; onChange(value: string): void };

function SettingsGroup(props: { theme: Theme; title: string; children?: JSX.Element }) {
  return <View class="min-w-0 gap-2">
    <Label theme={props.theme} muted size={textSize(props.theme, 16)}>{props.title}</Label>
    <View class="min-w-0 gap-4 p-4" style={{ backgroundColor: props.theme.surface, borderRadius: props.theme.radius, borderWidth: 1, borderColor: props.theme.border }}>
      {props.children}
    </View>
  </View>;
}

function SettingRow(props: { theme: Theme; title: string; detail: string; mono?: boolean; children?: JSX.Element }) {
  return <View class="flex-row items-center justify-between flex-wrap gap-4">
    <View class="min-w-0 gap-1" style={{ flexGrow: 1, flexBasis: 140 }}>
      <Label theme={props.theme}>{props.title}</Label>
      <Label theme={props.theme} mono={props.mono} truncate={props.mono} size={textSize(props.theme, 12)} muted>{props.detail}</Label>
    </View>
    {props.children}
  </View>;
}

function Divider(props: { theme: Theme }) {
  return <View class="h-px w-full" style={{ backgroundColor: props.theme.border }} />;
}

function SelectSetting(props: SettingField & { searchable?: boolean; searchLabel?: string }) {
  return <SettingRow theme={props.theme} title={props.title} detail={props.detail}>
    <View style={{ width: 200, maxWidth: "100%" }}>
      <Choice {...props} inline />
    </View>
  </SettingRow>;
}

function SegmentedSetting(props: SettingField) {
  return <SettingRow theme={props.theme} title={props.title} detail={props.detail}>
    <ToggleGroup class="flex-row flex-wrap items-center gap-0.5 p-0.5"
      style={{ borderWidth: 1, borderColor: props.theme.border, borderRadius: props.theme.radius, backgroundColor: props.theme.background }}>
      <Each of={props.items} key={(option) => option.value}>
        {(option) => <Toggle pressed={props.value === option().value} accessibilityLabel={option().title}
          onChange={({ pressed }) => { if (pressed) props.onChange(option().value); }}
          class="min-h-7 flex-row items-center justify-center gap-1 px-2"
          style={{ borderRadius: props.theme.radius }}
          pressedStyle={{ backgroundColor: props.theme.accent }}
          hoverStyle={interactionStyles(props.theme, { selectedFill: props.value === option().value ? props.theme.accent : undefined }).hoverStyle}
          activeStyle={interactionStyles(props.theme, { selectedFill: props.value === option().value ? props.theme.accent : undefined }).activeStyle}
          focusVisibleStyle={focusOutline(props.theme)}>
          {option().glyph ? <Icon name={option().glyph!} size={14} color={props.theme.foreground} /> : null}
          <Label theme={props.theme} weight={props.value === option().value ? "600" : "400"} size={textSize(props.theme, 13)}>{option().title}</Label>
        </Toggle>}
      </Each>
    </ToggleGroup>
  </SettingRow>;
}

function General(props: SettingsProps) {
  const t = () => strings[props.locale];
  async function reveal() {
    try {
      await call<{ path: string }, void>("dialog.reveal", { path: props.preferences.root_dir });
    } catch (error) { props.notify(errorMessage(error), "error"); }
  }
  return (
    <SettingsGroup theme={props.theme} title={t().settings_general}>
      <SelectSetting theme={props.theme} title={t().settings_language} detail={t().settings_language_desc}
        value={props.locale} items={[
          { value: "zh-CN", title: "简体中文" }, { value: "en", title: "English" }, { value: "ja", title: "日本語" },
        ]} onChange={(value) => void props.setPreference("language", value)} />
      <Divider theme={props.theme} />
      <SegmentedSetting theme={props.theme} title={t().settings_deploy_mode} detail={t().settings_deploy_mode_desc}
        value={props.preferences.deploy_mode} items={[
          { value: "symlink", title: t().mode_symlink }, { value: "copy", title: t().mode_copy },
        ]} onChange={(value) => void props.setPreference("deploy_mode", value)} />
      <Divider theme={props.theme} />
      <SettingRow theme={props.theme} title={t().settings_root_dir} detail={props.preferences.root_dir} mono>
        <Action theme={props.theme} label={t().tooltip_reveal_folder} disabled={!props.preferences.root_dir}
          glyph="folder-open" iconOnly tooltip={t().tooltip_reveal_folder} onPress={() => void reveal()} />
      </SettingRow>
    </SettingsGroup>
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
  const options = (values: number[]) => values.map((value) => ({ value: String(value), title: `${value}px` }));
  return (
    <View style={{ gap: 16 }}>
      <SettingsGroup theme={props.theme} title={t().settings_appearance}>
        <SegmentedSetting theme={props.theme} title={t().settings_appearance} detail={t().settings_appearance_desc}
          value={props.preferences.theme} items={[
            { value: "system", title: t().theme_system, glyph: "monitor" }, { value: "light", title: t().theme_light, glyph: "sun" },
            { value: "dark", title: t().theme_dark, glyph: "moon" },
          ]} onChange={(value) => void props.setPreference("theme", value)} />
        <Divider theme={props.theme} />
        <SelectSetting theme={props.theme} title={t().settings_accent_palette} detail={t().settings_accent_palette_desc}
          value={paletteFor(props.preferences.accent_palette)} items={paletteItems()}
          onChange={(value) => void props.setPreference("accent_palette", value)} />
        <Divider theme={props.theme} />
        <SegmentedSetting theme={props.theme} title={t().settings_corner_radius} detail={t().settings_corner_radius_desc}
          value={props.preferences.corner_radius} items={options([0, 4, 6, 8, 12])}
          onChange={(value) => void props.setPreference("corner_radius", value)} />
      </SettingsGroup>
      <SettingsGroup theme={props.theme} title={t().settings_fonts}>
        <SelectSetting theme={props.theme} title={t().settings_ui_font} detail={t().settings_ui_font_desc}
          value={props.preferences.ui_font} items={fontItems(t().font_auto_by_lang, props.preferences.ui_font)}
          searchable searchLabel={t().search_fonts} onChange={(value) => void props.setPreference("ui_font", value)} />
        <Divider theme={props.theme} />
        <SelectSetting theme={props.theme} title={t().settings_mono_font} detail={t().settings_mono_font_desc}
          value={props.preferences.mono_font} items={fontItems(t().font_auto_mono, props.preferences.mono_font)}
          searchable searchLabel={t().search_fonts} onChange={(value) => void props.setPreference("mono_font", value)} />
        <Divider theme={props.theme} />
        <SegmentedSetting theme={props.theme} title={t().settings_font_size} detail={t().settings_font_size_desc}
          value={props.preferences.font_size} items={options([13, 14, 15, 16])}
          onChange={(value) => void props.setPreference("font_size", value)} />
      </SettingsGroup>
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
      <SettingsGroup theme={props.theme} title={t().settings_network}>
        <FormText theme={props.theme} title={t().settings_http_proxy} detail={t().settings_http_proxy_desc}
          placeholder={t().ph_http_proxy} value={proxy()} onChange={(value) => proxy.set(value)} />
        <Divider theme={props.theme} />
        <FormText theme={props.theme} title={t().settings_github_token} detail={t().settings_github_token_desc}
          placeholder={t().ph_github_token} masked value={token()} onChange={(value) => token.set(value)} />
        <View class="flex-row justify-end"><Action theme={props.theme} glyph="check" onPrimary label={networkBusy() ? t().saving : t().btn_save}
          disabled={networkBusy()} onPress={() => void network()} /></View>
      </SettingsGroup>
      <SettingsGroup theme={props.theme} title={t().settings_git_backup}>
        <FormText theme={props.theme} title={t().settings_git_backup_repo} detail={t().settings_git_backup_repo_desc}
          placeholder={t().ph_git_backup_repo} value={repo()} onChange={(value) => repo.set(value)} />
        <Divider theme={props.theme} />
        <SettingRow theme={props.theme} title={t().settings_git_backup_branch} detail={t().settings_git_backup_branch_desc}>
          <View style={{ width: 200, maxWidth: "100%" }}>
            <FormText inline theme={props.theme} title={t().settings_git_backup_branch} placeholder="main"
              value={branch()} onChange={(value) => branch.set(value)} />
          </View>
        </SettingRow>
        {statusError() ? <InlineError theme={props.theme} message={statusError()} /> : null}
        <View class="flex-row flex-wrap items-center gap-2">
          <View class="flex-1 min-w-0">
            {status() ? <Label theme={props.theme} muted size={textSize(props.theme, 12)}>
              {`${status()!.local_skill_count} ${t().skills_unit} · ${status()!.local_stack_count} ${t().nav_stacks}`}
            </Label> : null}
          </View>
          <Action theme={props.theme} glyph="check" label={backupBusy() === "save" ? t().saving : t().btn_save}
            disabled={backupBusy() !== null} onPress={() => void backup("save")} />
          <Action theme={props.theme} glyph="arrow-down" label={backupBusy() === "pull" ? t().busy_backup_pulling : t().btn_backup_pull}
            disabled={backupBusy() !== null || !repo().trim()} onPress={() => void backup("pull")} />
          <Action theme={props.theme} glyph="arrow-up" onPrimary label={backupBusy() === "push" ? t().busy_backup_pushing : t().btn_backup_push}
            disabled={backupBusy() !== null || !repo().trim()} onPress={() => void backup("push")} />
        </View>
      </SettingsGroup>
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
  const undetected = () => agents().filter((agent) => !agent.installed);

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

  function renderRows(items: Agent[]) {
    return <View class="p-4" style={{ borderWidth: 1, borderRadius: props.theme.radius, borderColor: props.theme.border }}>
      <List each={items} viewport="parent" estimatedItemSize={40} keyExtractor={(agent) => agent.key}>
        {(item) => (
          <View style={{ flexDirection: "row", gap: 8, alignItems: "center", minHeight: 40,
            paddingHorizontal: 12, paddingVertical: 4, borderBottomWidth: 1, borderColor: props.theme.border,
            backgroundColor: props.theme.background }}>
            <View style={{ width: "32%", minWidth: 0 }}><Label theme={props.theme} truncate>{item().display_name}</Label></View>
            <View class="flex-1 min-w-0"><Label theme={props.theme} muted mono truncate size={textSize(props.theme, 12)}>{item().skills_dir}</Label></View>
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
    </View>;
  }

  return (
    <View class="flex-1 min-h-0 min-w-0">
      <Scroll class="flex-1 min-w-0 min-h-0">
        <View class="w-full gap-3 p-5" style={{ maxWidth: 720 }}>
        <View class="flex-row items-center justify-between gap-2">
          <Label theme={props.theme} muted weight="600">{`${t().badge_detected} (${detected().length})`}</Label>
          <Action theme={props.theme} label={t().settings_add_custom_agent} glyph="plus"
            onPress={() => { name.set(""); directory.set(""); formError.set(""); open.set(true); }} />
        </View>
        {listError() ? <InlineError theme={props.theme} message={listError()} /> : null}
        {loading() ? <Label theme={props.theme} muted>{t().loading}</Label> : null}
        {detected().length ? renderRows(detected()) : null}
        {undetected().length ? (
          <Collapsible open={showUndetected()} content={renderRows(undetected())}>
            <ActionControl theme={props.theme} accessibilityLabel={t().badge_not_detected} selected={showUndetected()}
              class="min-h-8 flex-row items-center gap-2 px-3"
              onPress={() => showUndetected.update((shown) => !shown)}>
              <View class={showUndetected() ? "rotate-90" : undefined}><Icon name="chevron-right" color={props.theme.muted} size={14} /></View>
              <Label theme={props.theme} weight="600">{`${t().badge_not_detected} (${undetected().length})`}</Label>
            </ActionControl>
          </Collapsible>
        ) : null}
        </View>
      </Scroll>
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
