import { expect, open, test } from "nsh/test";

test("the dialog stays centered and fits a constrained viewport", async () => {
  const app = await open("./ModalHarness.tsx", { width: 1240, height: 780, platform: "linux" });
  const measure = async () => {
    await app.press("role:button[Measure dialog]");
    return JSON.parse(app.texts().find((text) => text.startsWith("Bounds: "))!.slice(8)) as
      { x: number; y: number; width: number; height: number };
  };
  const regular = await measure();
  expect(regular.x + regular.width / 2).toBe(620);
  expect(Math.abs(regular.y + regular.height / 2 - 390)).toBeLessThanOrEqual(1);

  await app.setViewport(340, 150);
  const compact = await measure();
  expect(Math.abs(compact.x + compact.width / 2 - 170)).toBeLessThanOrEqual(1);
  expect(Math.abs(compact.y + compact.height / 2 - 75)).toBeLessThanOrEqual(1);
  expect(compact.width).toBeLessThan(regular.width);
  expect(compact.height).toBeLessThan(regular.height);
  expect(compact.x).toBeGreaterThan(0);
  expect(compact.y).toBeGreaterThan(0);
  await app.keyboard.press("escape");
  await app.waitFor("role:dialog[Fixture dialog]", { state: "gone" });
});
