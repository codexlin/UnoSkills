import { expect, open, test } from "nsh/test";
import { strings } from "../../src/i18n";
import { getStrings } from "../../src/library/strings";
import { defaults } from "../../src/theme";

const t = strings.en;
const release = {
  latest: { tag_name: "v9.0.0", html_url: "https://example.com/releases/v9", body: "" },
  recommended_asset: { name: "setup.deb", size: 4_194_304 },
  checksums_asset: { name: "SHA256SUMS.txt", size: 252 },
};
const summary = {
  skill_count: 3, deployed_count: 1, agent_count: 3, enabled_agent_count: 2, project_count: 4, adoptable_count: 0,
  recent: [{ id: "fixture", name: "Recent fixture", date: "2026-11-21" }],
  projects: [{ id: "project", name: "Project fixture", path: "/tmp/project" }],
};

function openHome(
  handle: (command: string, input: unknown) => unknown,
  options?: { data?: typeof summary; width?: number; height?: number },
) {
  return open("../../App.tsx", {
    width: options?.width ?? 1240,
    height: options?.height ?? 780,
    platform: "linux",
    clock: {},
    host: { handlers: {
      "unoskills.command": (raw: unknown) => {
        const { command, input } = raw as { command: string; input: unknown };
        if (command === "settings.get") return { ok: true, value: { ...defaults, language: "en", theme: "dark" } };
        if (command === "home.summary") return { ok: true, value: options?.data ?? summary };
        return handle(command, input);
      },
    } },
  });
}

const rankedSkills = ["Trending fixture", "Second fixture", "Third fixture", "Fourth fixture", "Fifth fixture", "Sixth fixture"]
  .map((name, index) => ({
    id: name, name, source: "owner/repo", installs: 1200 - index,
    url: "https://skills.sh/owner/repo/fixture",
  }));

test("Home keeps status shortcuts and work above the expandable list in a narrow window", async () => {
  const app = await openHome((command) => ({
    ok: true,
    value: command === "home.market" ? rankedSkills : { available: null },
  }), { width: 960, height: 620 });
  const counts = "3 in library · 1 with deployment records";

  await app.waitFor(`text:${counts}`);
  await app.waitFor("text:Trending fixture");
  const texts = app.texts();
  expect(texts.indexOf(t.home_agents)).toBeLessThan(texts.indexOf(t.home_projects));
  expect(app.shows(`label:${t.home_agents} · 3 detected · 2 enabled`)).toBe(true);
  expect(app.shows(`label:${t.home_linked_projects} · 4`)).toBe(true);
  expect(app.shows(`text:${t.home_adoptable_skills}`)).toBe(false);
  expect(texts.indexOf(t.home_projects)).toBeLessThan(texts.indexOf(counts));
  expect(texts.indexOf(counts)).toBeLessThan(texts.indexOf(t.home_market_trending));
  expect(app.shows("role:button[Project fixture]")).toBe(true);
  expect(app.shows("role:button[Recent fixture]")).toBe(true);
  expect(app.shows(`label:${t.home_open_library}`)).toBe(true);
  expect(app.shows("text:owner/repo")).toBe(true);
  expect(app.shows(`text:${t.home_market_installs.replace("{count}", "1,200")}`)).toBe(true);
  expect(app.shows(`role:link[Trending fixture · ${t.home_market_details}]`)).toBe(true);
  expect(app.shows(`label:${t.tab_my_stacks}`)).toBe(true);
  expect(app.shows(`role:tab[${t.board_all_time}]`)).toBe(false);
  expect(app.shows("text:Added in the last 7 days")).toBe(false);
  expect(app.shows("text:Third fixture")).toBe(true);
  expect(app.shows("text:Fourth fixture")).toBe(false);
  await app.press(`label:${t.home_market_show_more}`);
  await app.waitFor("text:Sixth fixture");
  await app.press(`label:${t.home_market_show_less}`);
  expect(app.shows("text:Fourth fixture")).toBe(false);
});

test("returning to Home keeps the loaded discovery preview", async () => {
  let requests = 0;
  const app = await openHome((command) => {
    if (command === "home.market") {
      requests += 1;
      return { ok: true, value: rankedSkills };
    }
    return { ok: true, value: command === "update.check" ? { available: null } : [] };
  });

  await app.waitFor("text:Sixth fixture");
  expect(app.shows(`label:${t.home_market_show_more}`)).toBe(false);
  expect(app.shows("id:home-ranking-skeleton")).toBe(false);
  await app.press(`label:${t.nav_settings}`);
  await app.press(`label:${t.nav_home}`);
  expect(app.shows("text:Sixth fixture")).toBe(true);
  expect(requests).toBe(1);
});

test("the empty library offers the first install even when discovery is offline", async () => {
  const app = await openHome((command) => {
    if (command === "home.market") return { ok: false, code: "TEST_OFFLINE", message: "offline" };
    if (command === "library.state") return { ok: true, value: { skills: [], agents: [], projects: [] } };
    if (command === "discover.leaderboard") return { ok: true, value: { items: [], has_more: false } };
    if (command === "discover.scan_adoptable") return { ok: true, value: { candidates: [] } };
    return { ok: true, value: command === "update.check" ? { available: null } : [] };
  }, { data: { ...summary, skill_count: 0, deployed_count: 0, project_count: 0, adoptable_count: 2, projects: [], recent: [] } });

  await app.waitFor(`text:${t.home_first_skill}`);
  await app.waitFor(`text:${t.home_market_unavailable}`);
  expect(app.shows(`label:${t.home_browse_stacks}`)).toBe(true);
  expect(app.shows(`label:${t.home_adoptable_skills} · 2`)).toBe(true);
  expect(app.shows(`text:${t.home_recent}`)).toBe(false);
  await app.press(`label:${t.btn_install_from_link}`);
  await app.waitFor(`text:${getStrings("en").quick_install_title}`);
});

test("Library search releases focus on Escape and blank clicks without clearing the query", async () => {
  const app = await openHome((command) => {
    if (command === "library.state") {
      return { ok: true, value: { skills: [], agents: [], projects: [], scope_path: "" } };
    }
    return { ok: true, value: command === "home.market" ? [] : { available: null } };
  });
  const library = getStrings("en");
  const search = `label:${library.ph_search_skills}`;
  await app.press(`label:${t.nav_library}`);
  await app.type(search, "fixture");
  await app.keyboard.press("escape");
  await app.keyboard.press("x");
  expect(app.value(search)).toBe("fixture");

  await app.type(search, " two");
  await app.press("id:desktop-shell");
  await app.keyboard.press("y");
  expect(app.value(search)).toBe("fixture two");
});

test("the update dialog keeps progress through polling and continuing in the background", async () => {
  const status = (download: unknown) => ({
    ok: true,
    value: { version: "0.6.2", available: release, platform: "linux_x64", checking: false, download },
  });
  let finish!: () => void;
  const download = new Promise((resolve) => { finish = () => resolve(status({ phase: "ready", verified: true })); });
  let downloadStarted = false;
  let polledProgress: { phase: string; percent?: number } = { phase: "idle" };
  let polls = 0;
  const installCalls: string[] = [];
  const app = await openHome((command) => {
    switch (command) {
      case "update.check": return status({ phase: "idle" });
      case "update.download":
        downloadStarted = true;
        return download;
      case "update.status":
        polls++;
        return status(polledProgress);
      case "update.install":
        installCalls.push(command);
        return { ok: true, value: { launched: true } };
      default: return { ok: true, value: [] };
    }
  });
  await app.press(`label:${t.badge_update_available} ${release.latest.tag_name}`);
  const downloadPress = app.press(`label:${t.btn_download_update}`);
  try {
    await app.waitUntil(() => downloadStarted);
    await app.waitUntil(() => polls > 0);
    expect(app.shows(`role:progressbar[${t.update_preparing}]`)).toBe(true);
    polledProgress = { phase: "downloading", percent: 37 };
    await app.waitFor(`text:${t.update_downloading} 37%`);
    expect(app.shows(`role:progressbar[${t.update_downloading} 37%]`)).toBe(true);
    await app.press(`label:${t.update_background}`);
    await app.waitUntil(() => !app.shows(`role:dialog[${t.settings_updates}]`));
    polledProgress = { phase: "verifying" };
    await app.waitFor(`role:button[${t.update_verifying}]`);
    await app.press(`role:button[${t.update_verifying}]`);
    expect(app.shows(`role:progressbar[${t.update_verifying}]`)).toBe(true);
    expect(installCalls).toEqual([]);
    finish();
    await downloadPress;
    await app.waitFor(`text:${t.badge_sha256_verified}`);
    await app.press(`label:${t.btn_install_update_linux}`);
    expect(installCalls).toEqual(["update.install"]);
  } finally {
    finish();
    await downloadPress;
  }
});
