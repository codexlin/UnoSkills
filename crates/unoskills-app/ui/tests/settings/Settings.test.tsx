import { expect, open, test } from "nsh/test";
import { strings } from "../../src/i18n";
import { defaults } from "../../src/theme";

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
  await app.press(settings);
  await app.press(`role:button[${t.settings_appearance_tab}]`);
  await app.press(`label:${t.settings_accent_palette}`);
  await app.press("text:Claude");
  await app.hover(library);
  await app.waitUntil(() => app.viewStyle(library)?.backgroundColor === "#e9e6dcff");
  await app.hover(settings);
  expect(app.viewStyle(settings)?.backgroundColor).toBe("#f5e8e2ff");

  await app.press(`label:${t.settings_ui_font}`);
  const automatic = `role:option[${t.font_auto_by_lang}]`;
  await app.waitFor(automatic);
  expect(app.viewStyle(automatic)?.backgroundColor).toBe("#f5e8e2ff");
  await app.keyboard.press("down");
  expect(app.viewStyle("role:option[Fixture Font 0]")?.backgroundColor).toBe("#e9e6dcff");
  expect(app.viewStyle(automatic)?.backgroundColor).toBe("#f5e8e2ff");
  await app.type(`label:${t.search_fonts}`, "Narrow Mono");
  await app.press(`text:${font}`);
  expect(writes).toEqual([
    { key: "accent_palette", value: "claude" }, { key: "ui_font", value: font },
  ]);
});
