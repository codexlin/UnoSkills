import { expect, open, test } from "nsh/test";
import { strings } from "../../src/i18n";
import { defaults } from "../../src/theme";

const t = strings.en;
const release = {
  latest: { tag_name: "v9.0.0", html_url: "https://example.com/releases/v9", body: "" },
  recommended_asset: { name: "setup.deb", size: 4_194_304 },
  checksums_asset: { name: "SHA256SUMS.txt", size: 252 },
};
const summary = {
  skill_count: 3, deployed_count: 1, agent_count: 2, adoptable_count: 0,
  recent: [{ id: "fixture", name: "Recent fixture", date: "2026-11-21" }],
  projects: [{ id: "project", name: "Project fixture", path: "/tmp/project" }],
  activity: [{ label: "11/20", count: 1 }, { label: "11/21", count: 2 }],
};

function openHome(handle: (command: string, input: unknown) => unknown) {
  return open("../../App.tsx", {
    width: 1240,
    height: 780,
    platform: "linux",
    clock: {},
    host: { handlers: {
      "unoskills.command": (raw: unknown) => {
        const { command, input } = raw as { command: string; input: unknown };
        if (command === "settings.get") return { ok: true, value: { ...defaults, language: "en", theme: "dark" } };
        if (command === "home.summary") return { ok: true, value: summary };
        return handle(command, input);
      },
    } },
  });
}

test("Home keeps active metric cards, recent rows, chart, and separate ranked boards", async () => {
  const boards: string[] = [];
  const app = await openHome((command, input) => {
    if (command !== "home.market") return { ok: true, value: { available: null } };
    const { board } = input as { board: string };
    boards.push(board);
    return { ok: true, value: [{
      id: board, name: board === "trending" ? "Trending fixture" : "All time fixture",
      installs: 30_500, source: "owner/repo", url: "https://skills.sh/owner/repo/fixture",
    }] };
  });

  await app.waitFor(`label:${t.nav_library} (3)`);
  expect(app.shows("role:button[Recent fixture]")).toBe(true);
  expect(app.shows("role:button[Project fixture]")).toBe(true);
  expect(app.shows(`text:${t.home_intake_total} 3`)).toBe(true);
  await app.waitFor("text:Trending fixture");
  expect(app.texts()).toContain("30.5k");
  expect(app.shows(`role:link[${t.home_market_details} · Trending fixture · owner/repo]`)).toBe(true);
  const trending = `role:tab[${t.board_trending}]`;
  const allTime = `role:tab[${t.board_all_time}]`;
  await app.hover(allTime);
  expect(app.viewStyle(allTime)?.backgroundColor ?? "#00000000").toBe("#00000000");
  expect(app.viewStyle(allTime)?.effectiveOpacity).toBe(1);
  await app.press(allTime);
  await app.waitFor("text:All time fixture");
  expect(app.shows("text:Trending fixture")).toBe(false);
  // Pointer still over the selected tab: its native indicator supplies the fill.
  expect(app.viewStyle(allTime)?.backgroundColor ?? "#00000000").toBe("#00000000");
  await app.press(allTime);
  expect(app.shows("text:All time fixture")).toBe(true);
  await app.press(trending);
  await app.waitFor("text:Trending fixture");
  expect(boards).toEqual(["trending", "all-time"]);
});

test("the title bar opens real update states with an accessible progress value", async () => {
  const status = (download: unknown) => ({
    ok: true,
    value: { version: "0.6.2", available: release, platform: "linux_x64", checking: false, download },
  });
  let finish!: () => void;
  const download = new Promise((resolve) => { finish = () => resolve(status({ phase: "ready", verified: true })); });
  let downloadStarted = false;
  const installCalls: string[] = [];
  const app = await openHome((command) => {
    switch (command) {
      case "update.check": return status({ phase: "idle" });
      case "update.download":
        downloadStarted = true;
        return download;
      case "update.status": return status({ phase: "downloading", percent: 37 });
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
    await app.wait(320);
    await app.waitFor(`text:${t.update_downloading} 37%`);
    expect(app.shows(`role:progressbar[${t.update_downloading} 37%]`)).toBe(true);
    expect(installCalls).toEqual([]);
    finish();
    await downloadPress;
    await app.waitFor(`text:${t.badge_sha256_verified}`);
    await app.press(`label:${t.btn_install_update_linux}`);
    expect(installCalls).toEqual(["update.install"]);
  } finally {
    finish();
  }
});
