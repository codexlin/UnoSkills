import { expect, open, test } from "nsh/test";
import { strings } from "../../src/i18n";
import { defaults, makeTheme } from "../../src/theme";
import catppuccin from "../../themes/catppuccin.json";
import claude from "../../themes/claude.json";
import ocean from "../../themes/ocean.json";
import supabase from "../../themes/supabase.json";
import unoskills from "../../themes/unoskills.json";
import vercel from "../../themes/vercel.json";

test("startup matches system languages and loads the saved appearance and language when present", async () => {
  const cases = [
    { languages: ["zh-Hans-CN", "en-US"], language: null, theme: "system", expected: "zh-CN" },
    { languages: ["fr-FR", "ja-JP", "en-US"], language: null, theme: "system", expected: "ja" },
    { languages: ["de-DE", "fr-FR"], language: null, theme: "system", expected: "en" },
    { languages: ["ja-JP"], language: "en", theme: "light", expected: "en" },
  ] as const;

  for (const { languages, language, theme, expected } of cases) {
    const app = await open("../../App.tsx", {
      width: 1240, height: 780, colorScheme: "dark",
      localeSettings: { languages: [...languages] },
      host: { handlers: {
        "unoskills.command": (raw: unknown) => {
          switch ((raw as { command: string }).command) {
            case "settings.get": return { ok: true, value: { ...defaults, language, theme } };
            case "home.summary": return { ok: true, value: {
              skill_count: 0, deployed_count: 0, agent_count: 0, enabled_agent_count: 0,
              project_count: 0, adoptable_count: 0, projects: [], recent: [],
            } };
            case "update.check": return { ok: true, value: { available: null } };
            default: return { ok: true, value: [] };
          }
        },
      } },
    });
    await app.waitFor(`text:${strings[expected].home_agents}`);
    const shell = "id:desktop-shell";
    expect(app.viewStyle(shell)?.backgroundColor).toBe(theme === "system" ? "#1c1917ff" : "#fdfbf7ff");
    await app.setColorScheme("light");
    await app.waitUntil(() => app.viewStyle(shell)?.backgroundColor === "#fdfbf7ff");
  }
});

test("settings exposes deploy mode, appearance and text size as direct choices", async () => {
  const writes: unknown[] = [];
  const t = strings["zh-CN"];
  const app = await open("../../App.tsx", {
    width: 960, height: 680,
    host: { handlers: {
      "unoskills.command": (raw: unknown) => {
        const { command, input } = raw as { command: string; input: unknown };
        switch (command) {
          case "settings.get":
            return { ok: true, value: { ...defaults, language: "zh-CN", deploy_mode: "symlink", font_size: "14" } };
          case "settings.deploy_mode":
          case "settings.preference":
            writes.push(input);
            return { ok: true, value: null };
          case "settings.fonts":
            return { ok: true, value: [] };
          case "home.summary":
            return { ok: true, value: { skill_count: 0, deployed_count: 0, agent_count: 0, enabled_agent_count: 0, project_count: 0, adoptable_count: 0, projects: [], recent: [] } };
          case "update.check":
            return { ok: true, value: { available: null } };
          default:
            return { ok: true, value: [] };
        }
      },
    } },
  });
  await app.press(`role:button[${t.nav_settings}]`);
  await app.press(`label:${t.mode_copy}`);
  await app.press(`role:button[${t.settings_appearance_tab}]`);
  await app.press(`label:${t.theme_light}`);
  await app.press("label:16px");
  await app.press(`role:button[${t.settings_general}]`);
  await app.press(`label:${t.settings_language}`);
  await app.press("日本語");
  await app.waitFor(`role:button[${strings.ja.settings_appearance_tab}]`);
  expect(app.texts()).toContain(strings.ja.settings_deploy_mode);
  expect(writes).toEqual([
    "copy", { key: "theme", value: "light" }, { key: "font_size", value: "16" }, { key: "language", value: "ja" },
  ]);
});

test("backup restore shows which local item was preserved when a pull is partial", async () => {
  const t = strings.en;
  const conflict = "local-skill: The local skill differs from the last synchronized backup";
  const app = await open("../../App.tsx", {
    width: 1240, height: 780,
    host: { handlers: {
      "unoskills.command": (raw: unknown) => {
        switch ((raw as { command: string }).command) {
          case "settings.get": return { ok: true, value: { ...defaults, language: "en" } };
          case "settings.backup_status": return { ok: true, value: {
            repo_url: "/tmp/fixture.git", branch: "main", local_skill_count: 2, local_stack_count: 0,
          } };
          case "settings.backup_pull": return { ok: true, value: {
            skills_imported: 0, skills_updated: 1, stacks_imported: 0, conflicts: [conflict],
          } };
          case "home.summary": return { ok: true, value: {
            skill_count: 2, deployed_count: 0, agent_count: 0, enabled_agent_count: 0,
            project_count: 0, adoptable_count: 0, projects: [], recent: [],
          } };
          case "update.check": return { ok: true, value: { available: null } };
          default: return { ok: true, value: [] };
        }
      },
    } },
  });
  await app.press(`role:button[${t.nav_settings}]`);
  await app.press(`role:button[${t.settings_network}]`);
  await app.waitFor(`text:2 ${t.skills_unit} · 0 ${t.nav_stacks}`);
  await app.press(`role:button[${t.btn_backup_pull}]`);
  await app.waitUntil(() => app.texts().some((text) => text.includes(conflict)));
  expect(app.texts().some((text) => text.includes(t.backup_pull_conflicts))).toBe(true);
});

test("navigation follows appearance and palette changes and a long font list remains searchable", async () => {
  const t = strings.en;
  const font = "Fixture Narrow Mono";
  const writes: unknown[] = [];
  const app = await open("../../App.tsx", {
    width: 1240, height: 780, platform: "linux", colorScheme: "light",
    host: { handlers: {
      "unoskills.command": (raw: unknown) => {
        const { command, input } = raw as { command: string; input: unknown };
        switch (command) {
          case "settings.get":
            return { ok: true, value: { ...defaults, language: "en" } };
          case "settings.fonts":
            return { ok: true, value: [...Array.from({ length: 80 }, (_, i) => `Fixture Font ${i}`), font] };
          case "settings.preference":
            writes.push(input);
            return { ok: true, value: null };
          case "home.summary":
            return { ok: true, value: { skill_count: 0, deployed_count: 0, agent_count: 0, enabled_agent_count: 0, project_count: 0, adoptable_count: 0, projects: [], recent: [] } };
          case "update.check":
            return { ok: true, value: { available: null } };
          default:
            return { ok: true, value: [] };
        }
      },
    } },
  });
  const settings = `role:button[${t.nav_settings}]`;
  const library = `role:button[${t.nav_library}]`;
  const appearance = `role:button[${t.settings_appearance_tab}]`;
  const general = `role:button[${t.settings_general}]`;
  const shell = "id:desktop-shell";
  await app.press(settings);
  await app.press(appearance);
  expect(app.viewStyle(shell)?.backgroundColor).toBe("#fdfbf7ff");
  expect(app.viewStyle(settings)?.backgroundColor).toBe("#b45309ff");
  expect(app.viewStyle(appearance)?.backgroundColor).toBe("#b45309ff");
  await app.setColorScheme("dark");
  await app.waitUntil(() => app.viewStyle(shell)?.backgroundColor === "#1c1917ff");
  expect(app.viewStyle(settings)?.backgroundColor).toBe("#f97316ff");
  expect(app.spanStyle(`${settings} >> text:${t.nav_settings}`).color).toBe("#ffffffff");
  await app.setColorScheme("light");
  await app.waitUntil(() => app.viewStyle(shell)?.backgroundColor === "#fdfbf7ff");

  await app.press(`label:${t.theme_dark}`);
  await app.waitUntil(() => app.viewStyle(shell)?.backgroundColor === "#1c1917ff");
  expect(app.viewStyle(settings)?.backgroundColor).toBe("#f97316ff");
  expect(app.viewStyle(appearance)?.backgroundColor).toBe("#f97316ff");
  expect(app.viewStyle(library)?.backgroundColor).toBe(undefined);
  await app.hover(library);
  expect(app.viewStyle(library)?.backgroundColor).toBe("#0ea5e9ff");
  expect(app.viewStyle(settings)?.backgroundColor).toBe("#f97316ff");
  await app.hover(general);
  expect(app.viewStyle(general)?.backgroundColor).toBe("#1e4252ff");
  await app.hover(appearance);
  expect(app.viewStyle(library)?.backgroundColor).toBe(undefined);
  expect(app.viewStyle(general)?.backgroundColor).toBe(undefined);
  expect(app.viewStyle(appearance)?.backgroundColor).toBe("#f97316ff");
  await app.press(`label:${t.theme_light}`);
  await app.waitUntil(() => app.viewStyle(shell)?.backgroundColor === "#fdfbf7ff");
  await app.press(`label:${t.settings_accent_palette}`);
  await app.press("text:Claude");
  await app.waitUntil(() => app.viewStyle(shell)?.backgroundColor === "#faf9f5ff");
  expect(app.spanStyle(`text:${t.settings_accent_palette}`).color).toBe("#141413ff");
  await app.hover(library);
  expect(app.viewStyle(library)?.backgroundColor).toBe("#e9e6dcff");
  await app.hover(settings);
  expect(app.viewStyle(settings)?.backgroundColor).toBe("#c96442ff");
  expect(app.spanStyle(`${settings} >> text:${t.nav_settings}`).color).toBe("#ffffffff");
  expect(app.spanStyle(`${appearance} >> text:${t.settings_appearance_tab}`).color).toBe("#ffffffff");

  const fontChoice = `role:combobox[${t.settings_ui_font}]`;
  await app.press(fontChoice);
  const automatic = `role:option[${t.font_auto_by_lang}]`;
  await app.waitFor(automatic);
  await app.hover(settings);
  expect(app.viewStyle(fontChoice)?.backgroundColor).toBe("#e9e6dcff");
  await app.keyboard.press("escape");
  await app.waitUntil(() => !app.shows(automatic));
  expect(app.viewStyle(fontChoice)?.backgroundColor).toBe("#faf9f5ff");
  await app.press(fontChoice);
  await app.waitFor(automatic);
  expect(app.viewStyle(automatic)?.backgroundColor).toBe("#e9e6dcff");
  await app.keyboard.press("down");
  expect(app.viewStyle("role:option[Fixture Font 0]")?.backgroundColor).toBe("#ede9deff");
  expect(app.viewStyle(automatic)?.backgroundColor).toBe("#e9e6dcff");
  await app.type(`label:${t.search_fonts}`, "Narrow Mono");
  await app.press(`text:${font}`);
  await app.press(`role:button[${t.settings_agents_registry}]`);
  await app.press(`role:button[${t.settings_add_custom_agent}]`);
  expect(app.spanStyle(`role:dialog[${t.settings_add_custom_agent}] >> text:${t.settings_add_custom_agent}`).color)
    .toBe("#28261bff");
  expect(writes).toEqual([
    { key: "theme", value: "dark" }, { key: "theme", value: "light" },
    { key: "accent_palette", value: "claude" }, { key: "ui_font", value: font },
  ]);
});

test("each theme supplies its own fonts, layered radii and shadows with explicit settings taking precedence", () => {
  const examples = [
    ["unoskills", "Oxanium", "Fira Code", 28, 88],
    ["claude", "PingFang SC", "monospace", 60, 120],
    ["vercel", "Geist", "Geist Mono", 60, 120],
    ["supabase", "Outfit", "monospace", 60, 120],
    ["catppuccin", "Montserrat", "Fira Code", 36, 96],
    ["ocean", "DM Sans", "IBM Plex Mono", 60, 120],
  ] as const;
  for (const [accent_palette, sans, monospace, control, panel] of examples) {
    const light = makeTheme({ ...defaults, accent_palette, language: "zh-CN", theme: "light" });
    const dark = makeTheme({ ...defaults, accent_palette, language: "zh-CN", theme: "dark" });
    expect(light.fontFamily?.split(",")[0]).toBe(sans);
    expect(light.monoFamily?.split(",")[0]).toBe(monospace);
    expect(Math.round(light.control.radius * 10)).toBe(control);
    expect(Math.round(light.panel.radius * 10)).toBe(panel);
    expect(light.control.inputFill).toBe("transparent");
    expect(dark.control.inputFill.startsWith("color-mix(")).toBe(true);
    expect(Boolean(light.control.shadow && dark.popup.shadow)).toBe(true);
    expect(dark.background === light.background).toBe(false);
  }
  const custom = makeTheme({ ...defaults, theme: "light", corner_radius: "0", ui_font: "Custom UI", mono_font: "Custom Mono" });
  expect([custom.control.radius, custom.panel.radius]).toEqual([0, 0]);
  expect([custom.fontFamily, custom.monoFamily]).toEqual(["Custom UI", "Custom Mono"]);
});

test("every preset preserves its light and dark color pairs", () => {
  const presets = { catppuccin, claude, ocean, supabase, unoskills, vercel };
  for (const [accent_palette, { cssVars }] of Object.entries(presets)) {
    for (const scheme of ["light", "dark"] as const) {
      const colors = cssVars[scheme];
      const theme = makeTheme({ ...defaults, accent_palette, theme: scheme });
      expect([theme.surface, theme.panel.foreground]).toEqual([colors.card, colors["card-foreground"]]);
      expect([theme.popup.background, theme.popup.foreground]).toEqual([colors.popover, colors["popover-foreground"]]);
      expect(theme.chrome?.hover).toBe(colors.muted);
      expect([theme.primary, theme.onPrimary, theme.accent, theme.onAccent]).toEqual([
        colors.primary, colors["primary-foreground"], colors.accent, colors["accent-foreground"],
      ]);
      expect(theme.secondary).toEqual({
        background: colors.secondary, foreground: colors["secondary-foreground"],
      });
      expect(theme.destructive).toEqual({
        background: colors.destructive, foreground: colors["destructive-foreground"],
      });
      expect(theme.sidebar?.onAccent).toBe(colors["sidebar-accent-foreground"]);
      expect(theme.switch).toEqual(scheme === "dark" ? {
        offTrack: `color-mix(in oklab, ${colors.input} 80%, transparent)`,
        offThumb: colors.foreground,
        onThumb: colors["primary-foreground"],
      } : {
        offTrack: colors.input, offThumb: colors.background, onThumb: colors.background,
      });
    }
  }
});
