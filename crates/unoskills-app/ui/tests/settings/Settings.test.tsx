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
