import { expect, open, test } from "nsh/test";
import { strings } from "../../src/i18n";
import { defaults, makeTheme } from "../../src/theme";

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
            return { ok: true, value: { skill_count: 0, deployed_count: 0, agent_count: 0, adoptable_count: 0, activity: [], projects: [], recent: [] } };
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

test("navigation uses the current palette and a long font list remains searchable", async () => {
  const t = strings.en;
  const font = "Fixture Narrow Mono";
  const writes: unknown[] = [];
  const app = await open("../../App.tsx", {
    width: 1240, height: 780, platform: "linux",
    host: { handlers: {
      "unoskills.command": (raw: unknown) => {
        const { command, input } = raw as { command: string; input: unknown };
        switch (command) {
          case "settings.get":
            return { ok: true, value: { ...defaults, language: "en", theme: "light" } };
          case "settings.fonts":
            return { ok: true, value: [...Array.from({ length: 80 }, (_, i) => `Fixture Font ${i}`), font] };
          case "settings.preference":
            writes.push(input);
            return { ok: true, value: null };
          case "home.summary":
            return { ok: true, value: { skill_count: 0, deployed_count: 0, agent_count: 0, adoptable_count: 0, activity: [], projects: [], recent: [] } };
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
  await app.hover(library);
  expect(app.viewStyle(library)?.backgroundColor).toBe("#e9e6dcff");
  await app.hover(settings);
  expect(app.viewStyle(settings)?.backgroundColor).toBe("#c96442ff");

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
    const light = makeTheme({ ...defaults, accent_palette, theme: "light" });
    const dark = makeTheme({ ...defaults, accent_palette, theme: "dark" });
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
