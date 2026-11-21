import { expect, open, test } from "nsh/test";
import type { Stack, StackFormData } from "../../src/projects/host";
import { defaults } from "../../src/theme";

const duplicate = { name: "Duplicate member", source: "owner/repo", note: "" };
const stack: Stack = {
  id: "draft-stack", name: "Original stack", author: "", description: "",
  tags: [], recommended_agents: [], verified: false, source: "Local",
  skills: [duplicate, { ...duplicate }, { name: "Other member", source: "owner/other", note: "" }],
};

type Edit = StackFormData & { previous: Stack; remove_indices: number[] };

async function openEditor() {
  const edits: Edit[] = [];
  const app = await open("./StackShortcutHarness.tsx", {
    width: 1200,
    height: 850,
    platform: "macos",
    host: {
      handlers: {
        "unoskills.command": (payload: unknown) => {
          const { command, input } = payload as { command: string; input: Edit };
          switch (command) {
            case "settings.get":
              return { ok: true, value: { ...defaults, language: "en" } };
            case "update.check":
              return { ok: true, value: { available: null } };
            case "home.summary":
              return { ok: true, value: {
                skill_count: 0, deployed_count: 0, agent_count: 0, adoptable_count: 0,
                activity: [], projects: [], recent: [],
              } };
            case "stack.list":
              return { ok: true, value: { local: [stack], featured: [] } };
            case "stack.members":
              return { ok: true, value: stack.skills.map((member) => ({ member, installed: null, source_conflict: false })) };
            case "stack.community":
            case "stack.library":
            case "home.market":
              return { ok: true, value: [] };
            case "stack.edit":
              edits.push(input);
              return { ok: true, value: { ...stack, name: input.name } };
            default:
              return { ok: false, code: "UNKNOWN", message: command };
          }
        },
      },
    },
  });

  await app.press("label:My Stacks");
  await app.waitFor("text:Duplicate member");
  expect(app.count("text:Duplicate member")).toBe(2);
  await app.press("label:Stack actions");
  await app.press("label:Edit stack");
  await app.waitFor("label:Stack Name");
  await app.clear("label:Stack Name");
  await app.type("label:Stack Name", "Edited stack");
  await app.submit("label:Stack Name");
  await app.press("text:Duplicate member >> nth=1");
  expect(app.enabled("label:Save to My Stacks")).toBe(true);
  await app.press("label:Home");
  await app.waitFor("text:Local workspace");
  return { app, edits };
}

test("only the visible stack editor handles save and retains duplicate member indices", async () => {
  const { app, edits } = await openEditor();
  await app.keyboard.press("mod+s");
  expect(edits).toHaveLength(0);
  await app.waitFor("text:save");
  expect(app.shows("text:Local workspace")).toBe(true);

  await app.press("label:Stacks");
  expect(app.value("label:Stack Name")).toBe("Edited stack");
  await app.keyboard.press("mod+s");
  await app.waitUntil(() => edits.length === 1);
  expect(edits[0].name).toBe("Edited stack");
  expect(edits[0].previous.skills).toEqual(stack.skills);
  expect(edits[0].remove_indices).toEqual([1]);
  await app.waitFor("label:Stack Name", { state: "gone" });
});

test("only the visible stack editor handles Escape and a hidden draft survives", async () => {
  const { app, edits } = await openEditor();
  await app.keyboard.press("escape");
  expect(app.shows("text:Local workspace")).toBe(true);
  await app.waitFor("text:cancel");
  expect(edits).toHaveLength(0);

  await app.press("label:Stacks");
  expect(app.value("label:Stack Name")).toBe("Edited stack");
  await app.keyboard.press("escape");
  await app.waitFor("label:Stack Name", { state: "gone" });
  await app.waitFor("label:Stack actions");
  expect(edits).toHaveLength(0);
});
