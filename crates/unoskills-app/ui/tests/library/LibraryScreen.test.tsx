import { describe, expect, open, test } from "nsh/test";
import type {
  AdoptBatchResponse,
  AdoptCandidate,
  AgentInfo,
  LeaderboardResponse,
  LibraryStateResponse,
  ProjectDeployment,
  ProjectStatusesResponse,
  ProjectWorkspace,
  SaveMarkdownResponse,
  ScanAdoptableResponse,
  Skill,
  SkillMarkdownResponse,
} from "../../src/library/types";

interface CommandPayload {
  command: string;
  input: Record<string, unknown> | null;
}

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason?: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const DEMO_AGENTS: AgentInfo[] = [
  {
    key: "claude-code",
    display_name: "Claude Code",
    category: "coding",
    skills_dir: "/tmp/home/.claude/skills",
    detect_dir: "/tmp/home/.claude",
    additional_scan_dirs: [],
    project_relative_skills_dir: ".claude/skills",
    installed: true,
    enabled: true,
    is_custom: false,
    has_path_override: false,
    recursive_scan: false,
  },
];

const DEMO_PROJECTS: ProjectWorkspace[] = [
  {
    id: "proj-1",
    name: "Demo Repo",
    path: "/tmp/demo-repo",
    created_at: 1743465600,
  },
];

function makeSkills(): Skill[] {
  return [
    {
      id: "skill-a",
      name: "Alpha Skill",
      slug: "alpha",
      description: "Alpha description",
      source_type: "local",
      source_ref: "/tmp/skills/alpha",
      source_subpath: null,
      source_branch: null,
      central_path: "/tmp/unoskills/skills/alpha",
      content_hash: "hash-a",
      estimated_tokens: 42,
      tags: ["rust"],
      deployments: [],
      project_deployments: [],
      created_at: 1743465600,
      updated_at: 1743465600,
    },
    {
      id: "skill-b",
      name: "Beta Skill",
      slug: "beta",
      description: "Beta description",
      source_type: "local",
      source_ref: "/tmp/skills/beta",
      source_subpath: null,
      source_branch: null,
      central_path: "/tmp/unoskills/skills/beta",
      content_hash: "hash-b",
      estimated_tokens: 55,
      tags: ["ts"],
      deployments: [],
      project_deployments: [],
      created_at: 1743465600,
      updated_at: 1743465600,
    },
  ];
}

function makeLibraryState(
  skills: Skill[],
  scopePath = "",
  projectStatuses: LibraryStateResponse["project_statuses"] = null,
): { ok: true; value: LibraryStateResponse } {
  return {
    ok: true,
    value: {
      skills,
      agents: DEMO_AGENTS,
      projects: DEMO_PROJECTS,
      deploy_mode: "symlink",
      scope_path: scopePath,
      project_statuses: projectStatuses,
      project_status_error: null,
    },
  };
}

function makeMarkdownReply(
  skillId: string,
  raw: string,
  preview: string,
): { ok: true; value: SkillMarkdownResponse } {
  return {
    ok: true,
    value: {
      skill_id: skillId,
      entry_file: "SKILL.md",
      raw_markdown: raw,
      preview_markdown: preview,
      lint: {
        valid: true,
        entry_file: "SKILL.md",
        estimated_tokens: 42,
        file_count: 1,
        warnings: [],
      },
    },
  };
}

describe("LibraryScreen markdown safety, scoped targets, and Discover flows", () => {
  test("failing read on skill B after viewing A clears A content and disables Edit", async () => {
    const skills = makeSkills();
    const app = await open("./LibraryTestHarness.tsx", {
      platform: "macos",
      width: 1200,
      height: 800,
      host: {
        params: { initialRoute: { page: "library" } },
        handlers: {
          "unoskills.command": (raw: unknown) => {
            const { command, input } = raw as CommandPayload;
            if (command === "library.state") {
              return makeLibraryState(skills);
            }
            if (command === "library.get_markdown") {
              const id = String(input?.skill_id ?? "");
              if (id === "skill-a") {
                return makeMarkdownReply(
                  "skill-a",
                  "---\nname: Alpha\n---\n# Alpha Body",
                  "# Alpha Body",
                );
              }
              return {
                ok: false,
                code: "READ_FAILED",
                message: "Permission denied reading Beta SKILL.md",
              };
            }
            return { ok: false, code: "UNKNOWN", message: command };
          },
        },
      },
    });

    await app.waitFor("text:Alpha Skill");
    await app.press("Preview");
    await app.waitFor("text:Alpha Body");
    expect(app.enabled("Edit SKILL.md")).toBe(true);

    await app.press("Beta Skill");
    await app.waitFor("text:Permission denied reading Beta SKILL.md");
    expect(app.texts().join("\n")).not.toContain("Alpha Body");
    expect(app.enabled("Edit SKILL.md")).toBe(false);
    await expect(app.press("Edit SKILL.md")).rejects.toMatchObject({
      code: "ERR_NSH_TEST_DISABLED",
    });
  });

  test("delayed read resolving after switching skills does not overwrite active selection", async () => {
    const skills = makeSkills();
    const betaRead = deferred<unknown>();
    let betaReadStarted = false;

    const app = await open("./LibraryTestHarness.tsx", {
      platform: "macos",
      width: 1200,
      height: 800,
      host: {
        params: { initialRoute: { page: "library" } },
        handlers: {
          "unoskills.command": (raw: unknown) => {
            const { command, input } = raw as CommandPayload;
            if (command === "library.state") {
              return makeLibraryState(skills);
            }
            if (command === "library.get_markdown") {
              const id = String(input?.skill_id ?? "");
              if (id === "skill-b") {
                betaReadStarted = true;
                return betaRead.promise;
              }
              return makeMarkdownReply(
                "skill-a",
                "---\nname: Alpha\n---\n# Alpha Fresh",
                "# Alpha Fresh",
              );
            }
            return { ok: false, code: "UNKNOWN", message: command };
          },
        },
      },
    });

    await app.waitFor("text:Alpha Skill");
    await app.press("Preview");
    await app.waitFor("text:Alpha Fresh");

    // Start selecting Beta (its markdown read stays pending), then switch back to Alpha.
    const betaPress = app.press("Beta Skill");
    await app.waitUntil(() => betaReadStarted);
    expect(app.enabled("Edit SKILL.md")).toBe(false);

    await app.press("Alpha Skill");
    await app.waitFor("text:Alpha Fresh");

    // Resolve Beta's delayed read late; Alpha must remain intact.
    betaRead.resolve(
      makeMarkdownReply("skill-b", "---\nname: Beta\n---\n# Stale Beta", "# Stale Beta"),
    );
    await betaPress;
    await app.wait(50);

    const screenText = app.texts().join("\n");
    expect(screenText).toContain("Alpha Fresh");
    expect(screenText).not.toContain("Stale Beta");

    await app.press("Edit SKILL.md");
    expect(app.value("label:Edit SKILL.md")).toContain("# Alpha Fresh");
  });

  test("delayed async save for A completing after switching to B does not overwrite B", async () => {
    const skills = makeSkills();
    const alphaSave = deferred<{ ok: true; value: SaveMarkdownResponse }>();
    let alphaSaveStarted = false;
    let alphaMarkdown = "---\nname: Alpha\n---\n# Alpha Initial";

    const app = await open("./LibraryTestHarness.tsx", {
      platform: "macos",
      width: 1200,
      height: 800,
      clock: {},
      host: {
        params: { initialRoute: { page: "library" } },
        handlers: {
          "unoskills.command": (raw: unknown) => {
            const { command, input } = raw as CommandPayload;
            if (command === "library.state") {
              return makeLibraryState(skills);
            }
            if (command === "library.get_markdown") {
              const id = String(input?.skill_id ?? "");
              if (id === "skill-a") {
                return makeMarkdownReply("skill-a", alphaMarkdown, "# Alpha Initial");
              }
              return makeMarkdownReply(
                "skill-b",
                "---\nname: Beta\n---\n# Beta Original",
                "# Beta Original",
              );
            }
            if (command === "library.save_markdown") {
              alphaSaveStarted = true;
              alphaMarkdown = String(input?.content ?? alphaMarkdown);
              return alphaSave.promise;
            }
            return { ok: false, code: "UNKNOWN", message: command };
          },
        },
      },
    });

    await app.waitFor("text:Alpha Skill");
    await app.press("Preview");
    await app.waitFor("text:Alpha Initial");
    await app.press("Edit SKILL.md");
    await app.type("label:Edit SKILL.md", "\nExtra line");

    app.onDialog(() => "Discard changes");
    const savePress = app.press("Save");
    try {
      await app.waitUntil(() => alphaSaveStarted);
      await app.press("Beta Skill");
      await app.waitFor("text:Beta Original");
    } finally {
      // Explicitly complete the outstanding host call even if a test step fails.
      alphaSave.resolve({
        ok: true,
        value: {
          skill: { ...skills[0], estimated_tokens: 99 },
          entry_file: "SKILL.md",
          raw_markdown: alphaMarkdown,
          preview_markdown: "# Alpha Saved Late",
          lint: {
            valid: true,
            entry_file: "SKILL.md",
            estimated_tokens: 99,
            file_count: 1,
            warnings: [],
          },
        },
      });
    }
    await savePress;
    await app.wait(50);

    const screenText = app.texts().join("\n");
    expect(screenText).toContain("Beta Original");
    expect(screenText).not.toContain("Alpha Saved Late");
  });

  test("same-skill refresh preserves unsaved draft and route skill switch prompts discard", async () => {
    const skills = makeSkills();
    const app = await open("./LibraryTestHarness.tsx", {
      platform: "macos",
      width: 1200,
      height: 800,
      host: {
        params: { initialRoute: { page: "library" } },
        handlers: {
          "unoskills.command": (raw: unknown) => {
            const { command, input } = raw as CommandPayload;
            if (command === "library.state") {
              return makeLibraryState(skills);
            }
            if (command === "library.get_markdown") {
              const id = String(input?.skill_id ?? "");
              if (id === "skill-a") {
                return makeMarkdownReply(
                  "skill-a",
                  "---\nname: Alpha\n---\n# Alpha Base",
                  "# Alpha Base",
                );
              }
              return makeMarkdownReply(
                "skill-b",
                "---\nname: Beta\n---\n# Beta Target",
                "# Beta Target",
              );
            }
            return { ok: false, code: "UNKNOWN", message: command };
          },
        },
      },
    });

    await app.waitFor("text:Alpha Skill");
    await app.press("Preview");
    await app.waitFor("text:Alpha Base");
    await app.press("Edit SKILL.md");
    await app.type("label:Edit SKILL.md", "\nUnsaved draft line");

    // Trigger revision refresh while editing Alpha; draft must be preserved.
    await app.host.send("test.refresh");
    await app.wait(50);
    expect(app.value("label:Edit SKILL.md")).toContain("Unsaved draft line");

    // Route-driven selection change while dirty must prompt discard; Cancel preserves Alpha draft.
    app.onDialog(() => "Cancel");
    await app.host.send("test.navigate", { page: "library", skillId: "skill-b" });
    await app.wait(50);
    expect(app.dialogs().length).toBeGreaterThanOrEqual(1);
    expect(app.dialogs()[0]?.title).toContain("Discard");
    expect(app.value("label:Edit SKILL.md")).toContain("Unsaved draft line");

    // Returning with Discard changes switches cleanly to Beta.
    app.onDialog(() => "Discard changes");
    await app.host.send("test.set_active", false);
    await app.host.send("test.set_active", true);
    await app.waitFor("text:Beta Target");
  });

  test("nested scopePath stays aligned across status, conflict replace, undeploy, and root switch", async () => {
    const skills = makeSkills();
    const recorded: CommandPayload[] = [];
    let webDeployed = false;
    let conflictInjected = false;

    const app = await open("./LibraryTestHarness.tsx", {
      platform: "macos",
      width: 1200,
      height: 800,
      host: {
        params: {
          initialRoute: { page: "library", projectId: "proj-1", scopePath: "apps/web" },
        },
        handlers: {
          "unoskills.command": (raw: unknown) => {
            const payload = raw as CommandPayload;
            recorded.push(payload);
            const { command, input } = payload;
            const scope = String(input?.scope_path ?? "");
            const makeStatuses = (
              s: string,
            ): NonNullable<LibraryStateResponse["project_statuses"]> => ({
              "claude-code": {
                "skill-a": {
                  placement: s === "apps/web" && webDeployed ? "managed" : "missing",
                  global_available: false,
                },
                "skill-b": {
                  placement: "missing",
                  global_available: false,
                },
              },
            });

            if (command === "library.state") {
              return makeLibraryState(
                skills,
                scope,
                input?.project_id ? makeStatuses(scope) : null,
              );
            }
            if (command === "library.project_statuses") {
              const value: ProjectStatusesResponse = {
                project_id: String(input?.project_id ?? ""),
                scope_path: scope,
                statuses: makeStatuses(scope),
              };
              return { ok: true, value };
            }
            if (command === "library.get_markdown") {
              return makeMarkdownReply("skill-a", "# Alpha", "# Alpha");
            }
            if (command === "library.deploy_project_skill") {
              if (!input?.force && !conflictInjected) {
                conflictInjected = true;
                return {
                  ok: false,
                  code: "TARGET_CONFLICT",
                  message: "TARGET_CONFLICT: existing unmanaged directory",
                };
              }
              webDeployed = true;
              const value: ProjectDeployment = {
                id: "dep-1",
                skill_id: "skill-a",
                project_id: "proj-1",
                agent_key: "claude-code",
                scope_path: scope,
                target_path: "/tmp/demo-repo/apps/web/.claude/skills/alpha",
                mode: "symlink",
                synced_at: 1743465600,
              };
              return { ok: true, value };
            }
            if (command === "library.undeploy_project_skill") {
              webDeployed = false;
              return {
                ok: true,
                value: {
                  undeployed: true,
                  skill_id: "skill-a",
                  project_id: "proj-1",
                  scope_path: scope,
                  agent_key: "claude-code",
                },
              };
            }
            return { ok: false, code: "UNKNOWN", message: command };
          },
        },
      },
    });

    await app.waitFor("text:Demo Repo · apps/web");
    await app.waitUntil(() => app.texts().includes("/tmp/demo-repo/apps/web"));

    // First library.state must request apps/web scope.
    const stateCall = recorded.find((entry) => entry.command === "library.state");
    expect(stateCall?.input).toMatchObject({ project_id: "proj-1", scope_path: "apps/web" });

    // Deploy triggers TARGET_CONFLICT showing apps/web path, then force-replaces with scope_path.
    app.onDialog(() => "Replace");
    await app.press("label:Deploy to Project · Claude Code");
    await app.waitFor("label:Deployed to project · Claude Code");

    expect(app.dialogs()[0]?.message ?? "").toContain(
      "/tmp/demo-repo/apps/web/.claude/skills/alpha",
    );
    const deployCalls = recorded.filter((entry) => entry.command === "library.deploy_project_skill");
    expect(deployCalls).toHaveLength(2);
    expect(deployCalls[0]?.input).toMatchObject({
      project_id: "proj-1",
      scope_path: "apps/web",
      force: false,
    });
    expect(deployCalls[1]?.input).toMatchObject({
      project_id: "proj-1",
      scope_path: "apps/web",
      force: true,
    });

    // Undeploy also passes scope_path: "apps/web".
    await app.press("label:Deployed to project · Claude Code");
    await app.waitFor("label:Deploy to Project · Claude Code");
    const undeployCall = recorded.find(
      (entry) => entry.command === "library.undeploy_project_skill",
    );
    expect(undeployCall?.input).toMatchObject({
      project_id: "proj-1",
      scope_path: "apps/web",
    });

    // Project Overview preserves nested scopePath back to Projects.
    await app.press("Project Overview");
    await app.waitUntil(() => app.texts().join("\n").includes('"scopePath":"apps/web"'));

    // Return to library scope and explicitly switch to project root via Scope menu.
    await app.host.send("test.navigate", {
      page: "library",
      projectId: "proj-1",
      scopePath: "apps/web",
    });
    await app.waitUntil(() => app.texts().includes("/tmp/demo-repo/apps/web"));
    await app.press("label:Deployment scope");
    await app.press("text:Demo Repo");
    await app.waitUntil(() => app.texts().includes("/tmp/demo-repo"));

    const rootStatusCall = recorded.find(
      (entry) => entry.command === "library.project_statuses",
    );
    expect(rootStatusCall?.input).toMatchObject({
      project_id: "proj-1",
      scope_path: "",
    });
  });

  test("Discover adopt and quick install routes invoke matching host commands", async () => {
    const recorded: CommandPayload[] = [];
    const candidate: AdoptCandidate = {
      agent_key: "claude-code",
      agent_name: "Claude Code",
      name: "Unmanaged One",
      description: "Found in Claude directory",
      path: "/tmp/home/.claude/skills/unmanaged-one",
      estimated_tokens: 30,
    };
    const app = await open("./LibraryTestHarness.tsx", {
      platform: "macos",
      width: 1200,
      height: 800,
      host: {
        params: { initialRoute: { page: "discover", tab: "adopt" } },
        handlers: {
          "unoskills.command": (raw: unknown) => {
            const payload = raw as CommandPayload;
            recorded.push(payload);
            const { command } = payload;
            if (command === "library.state") {
              return makeLibraryState(makeSkills());
            }
            if (command === "discover.scan_adoptable") {
              const value: ScanAdoptableResponse = { candidates: [candidate] };
              return { ok: true, value };
            }
            if (command === "discover.adopt") {
              const value: AdoptBatchResponse = {
                adopted: [makeSkills()[0]],
                adopted_count: 1,
                failed: [],
              };
              return { ok: true, value };
            }
            if (command === "discover.install") {
              return { ok: true, value: makeSkills()[0] };
            }
            if (command === "discover.leaderboard") {
              const value: LeaderboardResponse = {
                board: "trending",
                page: 0,
                total: 0,
                has_more: false,
                items: [],
              };
              return { ok: true, value };
            }
            return { ok: false, code: "UNKNOWN", message: command };
          },
        },
      },
    });

    await app.waitFor("text:Unmanaged One");
    await app.press("Unmanaged One");
    app.onDialog(() => "Add to library");
    await app.press("Add to library (1)");
    await app.waitUntil(() => recorded.some((entry) => entry.command === "discover.adopt"));

    const adoptCall = recorded.find((entry) => entry.command === "discover.adopt");
    expect(adoptCall?.input).toMatchObject({
      candidates: [{ path: "/tmp/home/.claude/skills/unmanaged-one" }],
    });

    // Switch to Quick Install route and submit a source.
    await app.host.send("test.navigate", { page: "discover", tab: "install" });
    await app.waitFor("label:Quick Install");
    await app.type("label:Quick Install", "owner/repo");
    await app.press("Install");
    await app.waitUntil(() => recorded.some((entry) => entry.command === "discover.install"));

    const installCall = recorded.find((entry) => entry.command === "discover.install");
    expect(installCall?.input).toMatchObject({ source_ref: "owner/repo" });
  });

  test("Marketplace renders legacy rank tiers, official badge, responsive columns, and board changes", async () => {
    const skills = makeSkills();
    skills[0].source_ref = "acme/skills@second-skill";

    const trendingItems = [
      {
        id: "mkt-1",
        skill_id: "first-skill",
        name: "First Skill",
        source: "acme/official",
        installs: 12400,
        description: "Top ranked official skill",
        is_official: true,
        install_ref: "acme/official@first-skill",
        detail_url: "https://skills.sh/acme/official/first-skill",
      },
      {
        id: "mkt-2",
        skill_id: "second-skill",
        name: "Second Skill",
        source: "acme/skills",
        installs: 3200,
        description: "Already installed skill",
        is_official: false,
        install_ref: "acme/skills@second-skill",
        detail_url: "https://skills.sh/acme/skills/second-skill",
      },
      {
        id: "mkt-3",
        skill_id: "Beta Skill",
        name: "Beta Skill",
        source: "other/repo",
        installs: 890,
        description: "Conflicts with local skill name",
        is_official: false,
        install_ref: "other/repo@beta-skill",
        detail_url: null,
      },
      {
        id: "mkt-4",
        skill_id: "fourth-skill",
        name: "Fourth Skill",
        source: "acme/official",
        installs: 450,
        description: "Standard ranked skill",
        is_official: false,
        install_ref: "acme/official@fourth-skill",
        detail_url: null,
      },
    ];

    const app = await open("./LibraryTestHarness.tsx", {
      platform: "macos",
      width: 1200,
      height: 800,
      host: {
        params: { initialRoute: { page: "discover", tab: "trending" } },
        handlers: {
          "unoskills.command": (raw: unknown) => {
            const { command, input } = raw as CommandPayload;
            if (command === "library.state") {
              return makeLibraryState(skills);
            }
            if (command === "discover.scan_adoptable") {
              return { ok: true, value: { candidates: [] } };
            }
            if (command === "discover.leaderboard") {
              const board = String(input?.board ?? "trending");
              if (board === "hot") {
                const value: LeaderboardResponse = {
                  board: "hot",
                  page: 0,
                  total: 1,
                  has_more: false,
                  items: [trendingItems[3]],
                };
                return { ok: true, value };
              }
              const value: LeaderboardResponse = {
                board: "trending",
                page: 0,
                total: 24,
                has_more: true,
                items: trendingItems,
              };
              return { ok: true, value };
            }
            return { ok: false, code: "UNKNOWN", message: command };
          },
        },
      },
    });

    await app.waitFor("text:First Skill");
    const initialTexts = app.texts().join("\n");
    expect(initialTexts).toContain("#1");
    expect(initialTexts).toContain("#2");
    expect(initialTexts).toContain("#3");
    expect(initialTexts).toContain("#4");
    expect(initialTexts).toContain("4 / 24");
    expect(app.shows("label:Official")).toBe(true);
    expect(app.shows("id:marketplace-grid")).toBe(true);

    // Installed card offers View skill; same-name card action is disabled.
    expect(app.enabled("label:View skill · Second Skill · acme/skills")).toBe(true);
    expect(app.enabled("label:Name in library · Beta Skill · other/repo")).toBe(false);
    await expect(app.press("label:Name in library · Beta Skill · other/repo")).rejects.toMatchObject({
      code: "ERR_NSH_TEST_DISABLED",
    });

    // Responsive viewport checks: 2-column medium and 1-column narrow layouts stay healthy.
    await app.setViewport(700, 800);
    await app.wait(40);
    expect(app.shows("id:marketplace-grid")).toBe(true);
    expect(app.shows("text:First Skill")).toBe(true);

    await app.setViewport(480, 800);
    await app.wait(40);
    expect(app.shows("id:marketplace-grid")).toBe(true);
    expect(app.shows("text:First Skill")).toBe(true);

    await app.setViewport(1200, 800);
    await app.wait(40);

    // Clicking a card's source filters the grid to matching skills and exposes clear button.
    await app.press("label:Filter loaded results · First Skill · acme/official");
    await app.wait(40);
    const filteredTexts = app.texts().join("\n");
    expect(filteredTexts).toContain("First Skill");
    expect(filteredTexts).toContain("Fourth Skill");
    expect(filteredTexts).not.toContain("Second Skill");
    await app.press("label:Clear source filter");
    await app.waitFor("text:Second Skill");

    await app.press("label:Leaderboard");
    await app.press("text:Hot");
    await app.waitFor("text:Fourth Skill");
    expect(app.shows("text:First Skill")).toBe(false);
    expect(app.shows("id:marketplace-grid")).toBe(true);
  });

  test("Library split panes stay interactive across desktop and narrow viewports", async () => {
    const skills = makeSkills();
    const recorded: CommandPayload[] = [];
    let alphaDeployed = false;

    const app = await open("./LibraryTestHarness.tsx", {
      platform: "macos",
      width: 1200,
      height: 800,
      host: {
        params: { initialRoute: { page: "library" } },
        handlers: {
          "unoskills.command": (raw: unknown) => {
            const payload = raw as CommandPayload;
            recorded.push(payload);
            const { command, input } = payload;
            if (command === "library.state") {
              const current = makeSkills();
              if (alphaDeployed) {
                current[0].deployments = [
                  {
                    id: "gdep-1",
                    skill_id: "skill-a",
                    agent_key: "claude-code",
                    target_path: "/tmp/home/.claude/skills/alpha",
                    mode: "symlink",
                    synced_at: 1743465600,
                  },
                ];
              }
              return makeLibraryState(current);
            }
            if (command === "library.get_markdown") {
              const id = String(input?.skill_id ?? "skill-a");
              return makeMarkdownReply(id, `# ${id}`, `# ${id}`);
            }
            if (command === "library.deploy_skill") {
              alphaDeployed = true;
              return {
                ok: true,
                value: {
                  id: "gdep-1",
                  skill_id: "skill-a",
                  agent_key: "claude-code",
                  target_path: "/tmp/home/.claude/skills/alpha",
                  mode: "symlink",
                  synced_at: 1743465600,
                },
              };
            }
            return { ok: false, code: "UNKNOWN", message: command };
          },
        },
      },
    });

    await app.waitFor("text:Alpha Skill");
    await app.press("label:Deployments · Claude Code · Global");
    await app.waitUntil(() => recorded.some((entry) => entry.command === "library.deploy_skill"));
    expect(app.texts()).toContain("1 / 1");

    // Shrink viewport to narrow split and verify selection + Preview remain usable.
    await app.setViewport(680, 720);
    await app.wait(40);
    await app.press("Beta Skill");
    await app.press("Preview");
    await app.waitFor("text:skill-b");
    expect(app.enabled("Edit SKILL.md")).toBe(true);
    void skills;
  });
});
