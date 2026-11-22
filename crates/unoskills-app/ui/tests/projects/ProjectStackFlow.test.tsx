import { expect, open, test } from "nsh/test";
import type { Agent, Module, ProjectScope, Stack, StackMember } from "../../src/projects/host";
import { defaults } from "../../src/theme";

const project = { id: "880a4d16-ebec-4c91-878c-b06213b89f5d", name: "Parity fixture", path: "/fixture/repo" };
const scope = "apps/web";
const member: StackMember = { name: "Fixture skill", source: "owner/repo", note: "" };
const agents: Agent[] = [
  { key: "other", display_name: "Other agent", project_relative_skills_dir: ".other/skills" },
  { key: "codex", display_name: "Fixture Codex", project_relative_skills_dir: ".agents/skills" },
];
const savedStack: Stack = {
  id: "parity-stack", name: "Parity recommendations", author: "", description: "",
  recommended_agents: [], tags: [], verified: false, source: "Local", skills: [member],
};
const featured = { ...savedStack, id: "featured", name: "Built-in stack", source: "Featured", verified: true };
const community = { ...savedStack, id: "community", name: "Remote stack", source: "Community", tags: ["Testing"] };
const longStack: Stack = {
  ...savedStack,
  id: "library-stack",
  name: "Long local stack",
  skills: Array.from({ length: 48 }, (_, index) => ({ ...member, name: `Library skill ${index}` })),
};
const secondLocal = { ...savedStack, id: "second-local", name: "Second local stack" };
const scanned: Module[] = [
  { path: "", evidence: [{ technology: "Rust", path: "Cargo.toml", added: false }], recommendations: [] },
  { path: scope, evidence: [{ technology: "TypeScript", path: "apps/web/package.json", added: true }],
    recommendations: [{ member, technology: "TypeScript", purpose: "testing", installed_slug: "fixture-skill" }] },
];

type Call = { command: string; input: unknown };

async function openFixture(localStacks = [longStack, secondLocal], versionFixture = false, registration = false) {
  let local = localStacks;
  let deployed = false;
  let lock = "";
  const calls: Call[] = [];
  const ok = (value: unknown) => ({ ok: true, value });
  const app = await open("./StackShortcutHarness.tsx", {
    width: 1240, height: 780, platform: "linux", colorScheme: "dark", reducedMotion: true,
    host: { handlers: {
      "unoskills.command": (payload: unknown) => {
        const { command, input } = payload as Call;
        calls.push({ command, input });
        switch (command) {
          case "settings.get": return ok({ ...defaults, language: "en" });
          case "update.check": return ok({ available: null });
          case "home.summary": return ok({
            skill_count: 0, deployed_count: 0, agent_count: 0, enabled_agent_count: 0, project_count: 1, adoptable_count: 0, projects: [project], recent: [],
          });
          case "home.market":
          case "stack.library": return ok([]);
          case "stack.list": return ok({ local, featured: [featured] });
          case "stack.community": return ok([community]);
          case "stack.members": return ok((local.find(({ id }) => id === (input as { stack_id: string }).stack_id) ?? featured).skills
            .map((item) => ({ member: item, installed: { id: item.name }, source_conflict: false })));
          case "project.list": return ok({ projects: [project], agents });
          case "project.scan": return ok(scanned);
          case "project.scopes": return ok(scanned.map(({ path }) => path));
          case "project.inventory": return ok((input as ProjectScope).scope_path === scope
            ? versionFixture ? [{
              name: "reviewer", path: "/fixture/repo/apps/web/.agents/skills/reviewer",
              skill_id: "reviewer-id", locked_hash: lock || null, library_different: lock === "original",
              location: "project_deployment",
            }] : [{ name: "Globally available fixture", path: "/fixture/agents/fixture", location: "global" }]
            : []);
          case "project.lock_skill":
            lock = "original";
            return ok(null);
          case "project.prepare_version": return ok({
            id: "reviewed-plan", intent: "upgrade", skill_id: "reviewer-id", skill_name: "reviewer",
            current_hash: "original", candidate_hash: "upgraded",
            files: [{ path: "reference.md", kind: "modified", diff: "- old instructions\n+ review tests" }],
          });
          case "project.apply_version":
            lock = "upgraded";
            return ok(null);
          case "project.cancel_version": return ok(null);
          case "project.save_recommendations":
            local = [savedStack, ...local];
            return ok(savedStack);
          case "stack.preview_deploy": return ok({
            deployed: deployed ? 1 : 0, global_available: deployed || registration ? 0 : 1, project_local: 0,
            pending: registration && !deployed ? 1 : 0, unavailable: 0, conflicts: 0,
            rows: [{ index: 0, name: member.name, target: "apps/web/.agents/skills/fixture",
              outcome: deployed ? "already_present" : registration ? "register" : "global_only" }],
          });
          case "stack.confirm_deploy":
            deployed = true;
            return ok({ deployed: registration ? 0 : 1, registered: registration ? 1 : 0,
              already_deployed: 0, global_available: 0, project_local: 0, unavailable: 0, failed: [], skipped: [] });
          default: return { ok: false, message: command, code: "UNKNOWN" };
        }
      },
    } },
  });
  return { app, calls };
}

test("the empty My Stacks tab offers creation and still opens the recommended catalog", async () => {
  const { app } = await openFixture([]);
  await app.setViewport(960, 600);
  await app.press("label:Stacks");
  await app.waitFor("text:No local stacks yet");
  expect(app.shows("text:Select a Stack")).toBe(false);
  await app.press("role:tab[Featured]");
  await app.waitFor("label:Built-in stack");
  await app.waitFor("text:Included Skills (1)");
  await app.press("role:tab[My Stacks]");
  await app.press("label:Create Your First Stack");
  await app.waitFor("label:Stack Name");
});

test("Stacks keeps the detail footer available at desktop minimum size and opens the catalog search with mod+k", async () => {
  const { app } = await openFixture();
  await app.press("label:Stacks");
  await app.waitFor("text:Included Skills (48)");
  expect(app.shows("text:48 already in the library")).toBe(true);
  expect(app.shows("label:Stack actions")).toBe(true);

  await app.setViewport(960, 600);
  await app.press("label:Deploy to Project");
  await app.waitFor("label:Select a project");
  expect(app.enabled("label:Deploy Skills")).toBe(false);
  await app.press("label:Cancel");

  await app.keyboard.press("mod+k");
  await app.waitFor("label:Search stacks by name, author, or tag...");
  await app.keyboard.press("r");
  expect(app.value("label:Search stacks by name, author, or tag...")).toBe("r");
  await app.waitFor("label:Remote stack");
  await app.press("role:tab[My Stacks]");
  await app.press("label:Second local stack");
  await app.waitFor("text:Included Skills (1)");
  await app.press("role:tab[My Stacks]");
  expect(app.shows("text:Included Skills (1)")).toBe(true);
});

test("Project version actions lock the selected scope and require review before upgrading", async () => {
  const { app, calls } = await openFixture(undefined, true);
  await app.press("label:Parity fixture");
  await app.waitFor("text:Detected Stack");
  await app.press("label:Scan Scope");
  await app.press("text:apps/web");
  await app.press("label:Select an agent");
  await app.press("text:Fixture Codex");
  await app.press("label:Version · reviewer");
  await app.waitFor("text:Following the library");
  await app.press("label:Lock Project Version");
  await app.waitFor("text:Locked · Library version differs");
  const versionTarget = { project_id: project.id, scope_path: scope, agent_key: "codex", skill_id: "reviewer-id" };
  expect(calls.find(({ command }) => command === "project.lock_skill")?.input).toEqual(versionTarget);
  await app.press("label:Version · reviewer");
  await app.press("label:Review Library Version");
  await app.waitFor("text:reference.md");
  expect(app.texts()).toContain("- old instructions\n+ review tests");
  expect(calls.find(({ command }) => command === "project.prepare_version")?.input)
    .toEqual({ ...versionTarget, intent: "upgrade" });
  expect(calls.some(({ command }) => command === "project.apply_version")).toBe(false);
  await app.press("label:Upgrade Project and Keep Locked");
  await app.waitFor("text:Project version updated");
  await app.waitFor("text:Version locked");
  expect(calls.find(({ command }) => command === "project.apply_version")?.input).toEqual({ preview_id: "reviewed-plan" });
});

test("Project Overview sends the exact nested scope to deployment and keeps global deployment opt-in", async () => {
  const { app, calls } = await openFixture();
  await app.press("label:Parity fixture");
  await app.waitFor("text:Detected Stack");
  await app.press("label:Scan Scope");
  await app.press("text:apps/web");
  await app.press("label:Select an agent");
  await app.press("text:Fixture Codex");
  await app.waitFor("text:Globally available fixture");
  await app.waitFor("text:1 selected");
  await app.press("label:Save as Stack");

  await app.waitFor("text:1/1 available · 1 global · 0 to deploy");
  expect(app.texts()).toContain("/fixture/repo/apps/web");
  expect(app.enabled("label:Deploy Skills")).toBe(false);
  expect(app.texts()).toContain("apps/web/.agents/skills/fixture");
  expect(app.texts()).toContain("Only available globally");
  await app.press("text:Also deploy globally available skills");
  await app.waitFor("text:1/1 available · 1 global · 1 to deploy");
  await app.press("label:Deploy Skills");
  await app.waitFor("text:1 deployed · 0 failed");

  expect(calls.find(({ command }) => command === "project.save_recommendations")?.input).toEqual({
    project_id: project.id, scope_path: scope, members: [member],
  });
  expect(calls.find(({ command }) => command === "stack.confirm_deploy")?.input).toEqual({
    stack_id: savedStack.id, source: "Local", project_id: project.id,
    scope_path: scope, agent_key: "codex", include_global: true,
    reviewed: {
      deployed: 0, global_available: 1, project_local: 0, pending: 0, unavailable: 0, conflicts: 0,
      rows: [{ index: 0, name: member.name, target: "apps/web/.agents/skills/fixture", outcome: "global_only" }],
    },
  });
});

test("a shared stack deployment can link the selected Agent without adding another copy", async () => {
  const { app, calls } = await openFixture([secondLocal], false, true);
  await app.press("label:Stacks");
  await app.waitFor("label:Deploy to Project");
  await app.press("label:Deploy to Project");
  await app.press("label:Select a project");
  await app.press("text:Parity fixture");
  await app.press("label:Select an agent");
  await app.press("text:Fixture Codex");
  await app.waitFor("text:Link this Agent to the existing deployment");
  expect(app.enabled("label:Deploy Skills")).toBe(true);
  await app.press("label:Deploy Skills");
  await app.waitFor("text:0 deployed · 1 linked to Agent · 0 failed");
  expect(calls.find(({ command }) => command === "stack.confirm_deploy")?.input)
    .toMatchObject({ agent_key: "codex", reviewed: { rows: [{ outcome: "register" }] } });
});
