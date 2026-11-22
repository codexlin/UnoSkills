import { expect, open, test } from "nsh/test";

test("a busy version dialog survives Escape and closes after becoming idle", async () => {
  const app = await open("./VersionBusyHarness.tsx", { platform: "linux", reducedMotion: true });
  await app.waitFor("text:Review pending");
  await app.keyboard.press("escape");
  expect(app.shows("text:Review pending")).toBe(true);
  await app.press("label:Finish work");
  await app.waitFor("text:Review ready");
  await app.keyboard.press("escape");
  await app.waitFor("text:Review closed");
  expect(app.shows("text:Review ready")).toBe(false);
});
