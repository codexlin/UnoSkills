import { describe, expect, open, test } from "nsh/test";
import type {
  AdoptBatchResponse,
  AdoptCandidate,
  AdoptionMergePreview,
  AdoptionMergeRequest,
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
  test("managed skills show the library path and keep their original source labeled", async () => {
    const [alpha] = makeSkills();
    alpha.source_type = "adopted";
    alpha.source_ref = "C:\\Users\\Demo\\.claude\\skills\\alpha";
    alpha.central_path = "D:\\UnoSkills Data\\private-library\\skills\\alpha";
    const app = await open("./LibraryTestHarness.tsx", {
      platform: "linux", width: 960, height: 600,
      host: {
        params: { locale: "zh-CN" },
        handlers: {
          "unoskills.command": ({ command }: CommandPayload) => {
            if (command === "library.state") return makeLibraryState([alpha]);
            return makeMarkdownReply(alpha.id, "# Alpha", "# Alpha");
          },
        },
      },
    });

    await app.waitFor("text:技能库位置");
    expect(app.texts()).toContain("…\\private-library\\skills\\alpha");
    expect(app.texts()).toContain(alpha.central_path);
    expect(app.texts()).toContain("来源");
    expect(app.texts()).toContain(alpha.source_ref);
    await app.press("label:技能操作");
    await app.waitFor("text:复制技能库路径");
    expect(app.texts()).toContain("在文件管理器中打开目录");
  });

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

  test("saving A preserves CRLF and frontmatter when its response arrives after switching to B", async () => {
    const skills = makeSkills();
    const alphaSave = deferred<{ ok: true; value: SaveMarkdownResponse }>();
    const alphaSaveStarted = deferred<void>();
    const originalMarkdown = "---\r\nname: Alpha\r\n---\r\n# Alpha Initial";
    let alphaMarkdown = originalMarkdown;

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
              alphaMarkdown = String(input?.content ?? alphaMarkdown);
              alphaSaveStarted.resolve();
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
    await app.type("label:Edit SKILL.md", "\n");

    app.onDialog(() => "Discard changes");
    const savePress = app.keyboard.press("mod+s");
    try {
      await alphaSaveStarted.promise;
      expect(alphaMarkdown).toBe("\r\n" + originalMarkdown);
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
      await savePress;
    }
    await app.waitFor("text:success:Saved Alpha Skill (~99 tokens)");
    await app.waitFor("text:Beta Original");

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
    const deployment = {
      id: "dep-1", skill_id: "skill-a", project_id: "proj-1",
      agent_key: "claude-code", scope_path: "apps/web", mode: "copy",
      target_path: "/tmp/demo-repo/apps/web/.claude/skills/alpha",
      synced_at: 1743465600,
    } satisfies ProjectDeployment;
    let conflictInjected = false;
    const olderRefresh = deferred<unknown>();
    const refreshStarted = deferred<void>();
    const refreshedLibrary = makeLibraryState([{ ...skills[0], name: "Alpha after refresh" }, skills[1]], "apps/web");
    let holdNextRefresh = false;

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
              if (holdNextRefresh && scope === "apps/web") {
                holdNextRefresh = false;
                refreshStarted.resolve();
                return olderRefresh.promise;
              }
              return makeLibraryState(
                [{ ...skills[0], project_deployments: webDeployed ? [deployment] : [] }, skills[1]],
                scope,
                input?.project_id ? makeStatuses(scope) : null,
              );
            }
            if (command === "library.project_statuses") {
              // Release the old read only after the menu choice has changed the scope.
              if (scope === "") olderRefresh.resolve(refreshedLibrary);
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
              return { ok: true, value: deployment };
            }
            if (command === "project.review_conflict") {
              return { ok: true, value: {
                path: deployment.target_path, kind: "modified", open_folder: true, detail: null,
                preview_id: "reviewed-copy", files: [{ path: "SKILL.md", kind: "modified", diff: "- local\n+ original" }],
              } };
            }
            if (command === "project.cancel_conflict") return { ok: true, value: null };
            if (command === "project.apply_conflict") {
              webDeployed = true;
              return { ok: true, value: null };
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

    await app.press("label:Deploy to Project · Claude Code");
    await app.waitFor("text:UnoSkills manages this copy. Its files have changed.");
    expect(app.texts()).toContain(deployment.target_path);
    expect(app.texts()).toContain("- local\n+ original");
    await app.press("label:Review Overwrite");
    expect(recorded.some((entry) => entry.command === "project.apply_conflict")).toBe(false);
    await app.press("label:Overwrite Project Copy");
    await app.waitFor("label:Remove Project Deployment · Claude Code");

    const deployCalls = recorded.filter((entry) => entry.command === "library.deploy_project_skill");
    expect(deployCalls).toHaveLength(1);
    expect(deployCalls[0]?.input).toMatchObject({
      project_id: "proj-1",
      scope_path: "apps/web",
      force: false,
    });
    expect(recorded.find((entry) => entry.command === "project.apply_conflict")?.input)
      .toEqual({ preview_id: "reviewed-copy" });

    await app.press("label:Remove Project Deployment · Claude Code");
    await app.waitFor("text:Claude Code · Demo Repo · apps/web");
    await app.press("label:Cancel");
    expect(recorded.some((entry) => entry.command === "library.undeploy_project_skill")).toBe(false);
    await app.press("label:Remove Project Deployment · Claude Code");
    await app.press("label:Confirm Remove Project Deployment");
    await app.waitFor("label:Deploy to Project · Claude Code");
    const undeployCall = recorded.find(
      (entry) => entry.command === "library.undeploy_project_skill",
    );
    expect(undeployCall?.input).toMatchObject({
      project_id: "proj-1",
      scope_path: "apps/web",
      deployment: { id: deployment.id, target_path: deployment.target_path },
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
    holdNextRefresh = true;
    const refresh = app.host.send("test.refresh");
    try {
      await refreshStarted.promise;
      await app.press("text:Demo Repo");
      await app.waitUntil(() => app.texts().includes("/tmp/demo-repo"));

      const rootStatusCall = recorded.find(
        (entry) => entry.command === "library.project_statuses",
      );
      expect(rootStatusCall?.input).toMatchObject({
        project_id: "proj-1",
        scope_path: "",
      });
    } finally {
      olderRefresh.resolve(refreshedLibrary);
      await refresh;
    }
    await app.waitFor("text:Alpha after refresh");
    expect(app.texts()).toContain("/tmp/demo-repo");
    expect(app.texts()).not.toContain("/tmp/demo-repo/apps/web");
  });

  test("the project Agent filter keeps conflicts and hides the inspector for an excluded selection", async () => {
    const skills = makeSkills();
    const app = await open("./LibraryTestHarness.tsx", {
      platform: "linux", width: 960, height: 600,
      host: {
        params: { initialRoute: { page: "library", projectId: "proj-1", scopePath: "apps/web", skillId: "skill-a" } },
        handlers: {
          "unoskills.command": ({ command }: CommandPayload) => {
            if (command === "library.state") return makeLibraryState(skills, "apps/web", {
              "claude-code": {
                "skill-a": { placement: "missing", global_available: false },
                "skill-b": { placement: "conflict", global_available: false },
              },
            });
            return makeMarkdownReply("skill-a", "# Alpha", "# Alpha");
          },
        },
      },
    });
    await app.waitFor("text:Alpha Skill");
    await app.press("label:Filter by Agent");
    await app.press("text:Claude Code");
    await app.waitFor("label:Beta Skill");
    expect(app.shows("label:Alpha Skill")).toBe(false);
    expect(app.texts()).not.toContain("Alpha Skill");
    expect(app.shows("text:Conflict")).toBe(true);
    await app.press("label:Beta Skill");
    await app.waitFor("label:Resolve conflict · Claude Code");
  });

  test("blocked library deletion reviews each placement and keeps recovery details in the dialog", async () => {
    const [alpha] = makeSkills();
    const target = "/tmp/demo-repo/apps/web/.claude/skills/alpha";
    const global = "/fixture/.claude/skills/alpha";
    alpha.deployments = [{ id: "global", skill_id: alpha.id, agent_key: "claude-code", target_path: global, mode: "copy", synced_at: 0 }];
    alpha.project_deployments = [{
      id: "project", skill_id: alpha.id, project_id: "proj-1", scope_path: "apps/web", agent_key: "claude-code",
      target_path: target, mode: "copy", synced_at: 0,
    }];
    const failed = deferred<unknown>();
    const sent: unknown[] = [];
    const rescue = "/tmp/demo-repo/apps/web/.claude/skills/.unoskills-undeploy-xyz/original";
    const app = await open("./LibraryTestHarness.tsx", {
      platform: "linux", width: 960, height: 600,
      host: {
        handlers: {
          "unoskills.command": ({ command, input }: CommandPayload) => {
            if (command === "library.state") return makeLibraryState([alpha]);
            if (command === "library.remove_skill") {
              sent.push(input);
              return failed.promise;
            }
            return makeMarkdownReply(alpha.id, "# Alpha", "# Alpha");
          },
        },
      },
    });
    await app.press("label:Skill actions");
    await app.press("text:Permanently Delete Skill");
    await app.waitFor("text:Claude Code · Demo Repo · apps/web");
    expect(app.texts()).toContain("This permanently deletes the skill files from your UnoSkills library. This action cannot be undone.");
    expect(app.texts()).toContain("This also uninstalls the skill from the locations below by removing its managed copies or symlinks. Shared paths still in use will be kept.");
    expect(app.texts()).toContain("Claude Code · Global");
    expect(app.texts()).toContain(global);
    expect(app.texts()).toContain(target);
    const confirmation = app.press("label:Confirm Permanent Skill Deletion");
    try {
      await app.waitUntil(() => sent.length === 1);
      expect(app.enabled("label:Confirm Permanent Skill Deletion")).toBe(false);
      expect(sent).toEqual([{ skill_id: alpha.id, deployments: [
        { id: "global", target_path: global }, { id: "project", target_path: target },
      ] }]);
    } finally {
      failed.resolve({ ok: false, code: "TARGET_CONFLICT", message: "raw failure", affected_path: target, recovery_paths: [rescue] });
      await confirmation;
    }
    await app.waitFor("text:Automatic restoration was incomplete. Recover the original files from these locations before retrying.");
    expect(app.texts()).toContain(rescue);
    expect(app.texts()).not.toContain("raw failure");
    await app.press("label:Diagnostic Details");
    expect(app.texts()).toContain("raw failure");
    await app.press("label:Cancel");
    await app.waitFor("text:Alpha Skill");
    expect(sent).toHaveLength(1);
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
    const otherCandidate = { ...candidate, name: "Unmanaged Two", path: `${candidate.path}-two` };
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
              const value: ScanAdoptableResponse = { candidates: [candidate, otherCandidate] };
              return { ok: true, value };
            }
            if (command === "discover.adopt") {
              const value: AdoptBatchResponse = {
                adopted: [],
                adopted_count: 0,
                failed: [
                  { name: candidate.name, path: candidate.path, error: "SKILL_SLUG_CONFLICT: Existing entry" },
                  { name: otherCandidate.name, path: otherCandidate.path, error: "Permission denied" },
                ],
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
    await app.press("Unmanaged Two");
    app.onDialog(() => "Add to library");
    await app.press("Add to library (2)");
    await app.waitUntil(() => recorded.some((entry) => entry.command === "discover.adopt"));

    const adoptCall = recorded.find((entry) => entry.command === "discover.adopt");
    expect(adoptCall?.input).toMatchObject({
      candidates: [{ path: candidate.path }, { path: otherCandidate.path }],
    });
    await app.waitFor("text:Added 0 skills to the library (2 failed).");
    await app.press("View failures");
    await app.waitFor("text:This folder name is already in use in the library. The local skill was kept.");
    await app.waitFor("text:Permission denied");
    await app.press("Close");
    await app.waitFor("text:Add to library (2)");

    // Switch to Quick Install route and submit a source.
    await app.host.send("test.navigate", { page: "discover", tab: "install" });
    await app.waitFor("label:Quick Install");
    await app.type("label:Quick Install", "unfinished");
    await app.keyboard.press("escape");
    expect(app.shows("label:Quick Install")).toBe(false);
    await app.press("Install from Link");
    await app.waitFor("label:Quick Install");
    await app.clear("label:Quick Install");
    await app.press("Install");
    await app.waitFor("text:Enter a local path, Git URL, or owner/repo@skill first.");
    expect(app.texts()).not.toContain("error:Enter a local path, Git URL, or owner/repo@skill first.");
    expect(recorded.some((entry) => entry.command === "discover.install")).toBe(false);
    await app.type("label:Quick Install", "owner/repo");
    await app.press("Install");
    await app.waitUntil(() => recorded.some((entry) => entry.command === "discover.install"));

    const installCall = recorded.find((entry) => entry.command === "discover.install");
    expect(installCall?.input).toMatchObject({ source_ref: "owner/repo" });
  });

  test("local adoption reviews duplicate locations and keeps differing skills out of the bulk merge", async () => {
    const names = ["Same One", "Same Two", "Different"];
    const reviews: AdoptionMergePreview[] = names.map((name, index) => {
      const slug = name.toLowerCase().replace(" ", "-");
      const local = index === 2;
      return {
        token: `review-${index}`,
        candidate: {
          agent_key: "codex", agent_name: "Codex", name, description: null,
          path: `/home/demo/.codex/skills/${slug}`, estimated_tokens: 10,
        },
        library_path: `/home/demo/.unoskills/skills/${slug}`,
        existing_path: local ? null : `/home/demo/.agents/skills/${slug}`,
        existing_present: !local, keep_existing_for_peers: false,
        targets: local ? ["detected"] : ["existing", "detected"],
        identical: !local,
        library_changes: local ? [{ path: "SKILL.md", kind: "modified" }] : [],
        existing_changes: [],
        skill_diff: local ? "- Library version\n+ Local version\n" : null,
        local_version_impacts: [],
        shared_existing: [],
        shared_detected: [],
      };
    });
    let discovered = reviews.map((review) => review.candidate);
    const merges: AdoptionMergeRequest[][] = [];
    const app = await open("./LibraryTestHarness.tsx", {
      platform: "linux",
      width: 960,
      height: 600,
      host: {
        params: { initialRoute: { page: "discover", tab: "adopt" } },
        handlers: {
          "unoskills.command": (raw: unknown) => {
            const { command, input } = raw as CommandPayload;
            if (command === "library.state") return makeLibraryState(makeSkills());
            if (command === "discover.scan_adoptable") return { ok: true, value: { candidates: discovered } };
            if (command === "discover.adopt") {
              return { ok: true, value: { adopted: [], adopted_count: 0, failed: reviews.map((merge) => ({
                name: merge.candidate.name, path: merge.candidate.path,
                error: merge.existing_path ? "TARGET_CONFLICT: Existing deployment" : "SKILL_SLUG_CONFLICT: Existing folder name",
                merge,
              })) } };
            }
            if (command === "discover.merge_adoption") {
              const requests = input?.requests as AdoptionMergeRequest[];
              merges.push(requests);
              const merged = requests.map((request) => {
                const review = reviews.find(({ token }) => token === request.token)!;
                return {
                  path: request.candidate.path,
                  deployment: {
                    id: review.token, skill_id: review.token, agent_key: "codex",
                    target_path: (request.target === "existing" ? review.existing_path : null) ?? request.candidate.path,
                    mode: "symlink", synced_at: 0,
                  },
                  backup: review.identical ? null : "/home/demo/.unoskills/.adoption-backups/merge-1",
                  deployment_issues: [],
                };
              });
              discovered = discovered.filter(({ path }) => !merged.some((item) => item.path === path));
              return { ok: true, value: { merged, failed: [] } };
            }
            return { ok: false, code: "UNKNOWN", message: command };
          },
        },
      },
    });

    await app.waitFor("text:Same One");
    for (const name of names) await app.press(name);
    app.onDialog(() => "Add to library");
    await app.press("Add to library (3)");
    await app.waitFor("text:Added 0 skills to the library (3 failed).");
    await app.press("View failures");
    await app.press("Merge identical copies (2)");
    await app.waitUntil(() => app.texts().includes(reviews[0].library_path));
    expect(app.texts()).toContain(reviews[0].existing_path);
    expect(app.texts()).toContain(reviews[0].candidate.path);
    await app.press("Merge and manage");
    await app.waitUntil(() => merges.length === 1);
    expect(merges[0].map((request) => request.token)).toEqual(["review-0", "review-1"]);
    await app.waitFor("text:Added 2 skills to the library (1 failed).");
    expect(app.texts()).toContain("The library already has a skill with this folder name. Review the two versions before merging.");
    await app.waitFor("text:Review and merge");
    await app.press("Review and merge");
    await app.waitFor("label:Version to keep");
    await app.press("label:Version to keep");
    await app.press("text:Use detected local version");
    await app.press("label:Allow a managed copy");
    await app.press("Merge and manage");
    await app.waitUntil(() => merges.length === 2);
    expect(merges[0].every((request) => !request.allow_copy)).toBe(true);
    expect(merges[1][0]).toMatchObject({ token: "review-2", content: "local", target: "detected", allow_copy: true });
    await app.waitFor("text:View merge results");
    await app.press("View merge results");
    await app.waitFor("text:Show recovery folder");
    expect(app.texts()).toContain("/home/demo/.unoskills/.adoption-backups/merge-1");
  });

  test("a delayed local skill scan cannot replace a newer list or its selection", async () => {
    const candidate: AdoptCandidate = {
      agent_key: "claude-code",
      agent_name: "Claude Code",
      name: "Current Local Skill",
      description: null,
      path: "/tmp/home/.claude/skills/current",
      estimated_tokens: 10,
    };
    const oldScan = deferred<{ ok: true; value: ScanAdoptableResponse }>();
    let scans = 0;
    const app = await open("./LibraryTestHarness.tsx", {
      platform: "macos",
      width: 1200,
      height: 800,
      host: {
        params: { initialRoute: { page: "discover", tab: "adopt" }, showRefreshControl: true },
        handlers: {
          "unoskills.command": (raw: unknown) => {
            const { command } = raw as CommandPayload;
            if (command === "library.state") return makeLibraryState(makeSkills());
            if (command === "discover.scan_adoptable") {
              scans += 1;
              if (scans === 2) return oldScan.promise;
              return { ok: true, value: { candidates: scans === 1 ? [] : [candidate] } };
            }
            return { ok: false, code: "UNKNOWN", message: command };
          },
        },
      },
    });

    await app.waitFor("text:Everything is managed");
    const refresh = app.press("Refresh test screen");
    let latestRefresh: typeof refresh | undefined;
    try {
      await app.waitUntil(() => scans === 2);
      latestRefresh = app.press("Refresh test screen");
      await app.waitFor("text:Current Local Skill");
      await app.press("Current Local Skill");
      expect(app.enabled("Add to library (1)")).toBe(true);
    } finally {
      oldScan.resolve({ ok: true, value: {
        candidates: [{ ...candidate, name: "Stale Local Skill", path: "/tmp/home/.claude/skills/stale" }],
      } });
      await Promise.all([refresh, latestRefresh]);
    }

    await app.wait(50);
    expect(app.texts()).toContain("Current Local Skill");
    expect(app.texts()).not.toContain("Stale Local Skill");
    expect(app.enabled("Add to library (1)")).toBe(true);
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

    const leaderboard = "label:Leaderboard";
    await app.press(leaderboard);
    await app.waitFor("text:Hot");
    expect(app.viewStyle(leaderboard)?.backgroundColor).toBe("#eef2ffff");
    await app.keyboard.press("escape");
    await app.waitUntil(() => !app.shows("text:Hot"));
    await app.press(leaderboard);
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

test("skill update confirms the reviewed files and shows deployment issues", async () => {
  const skills = makeSkills();
  const commands: CommandPayload[] = [];
  let previewNumber = 0;
  const app = await open("./LibraryTestHarness.tsx", {
    platform: "linux", width: 1200, height: 800,
    host: {
      params: { initialRoute: { page: "library" } },
      handlers: {
        "unoskills.command": (raw: unknown) => {
          const payload = raw as CommandPayload;
          const { command, input } = payload;
          if (command === "library.state") return makeLibraryState(skills);
          if (command === "library.get_markdown") return makeMarkdownReply(String(input?.skill_id), "# Body", "# Body");
          commands.push(payload);
          const issue = { path: "/tmp/project/.claude/skills/alpha", agents: ["claude-code"], kind: "conflict" };
          if (command === "library.prepare_skill_update") return { ok: true, value: {
            id: `preview-${++previewNumber}`, skill_id: input?.skill_id, skill_name: skills[0].name,
            rollback: false, files: [{ path: "scripts/check.sh", kind: "modified" }], deployments: [issue],
          } };
          if (command === "library.cancel_skill_revision") return { ok: true, value: null };
          if (command === "library.apply_skill_revision") return { ok: true, value: {
            skill: skills[0], updated: true, refreshed_copies: 0, deployment_issues: [issue],
          } };
          return { ok: false, code: "UNKNOWN", message: command };
        },
      },
    },
  });

  await app.waitFor("text:Alpha Skill");
  await app.press("Skill actions");
  await app.press("Update");
  await app.waitFor("text:scripts/check.sh");
  expect(commands.map(({ command }) => command)).toEqual(["library.prepare_skill_update"]);
  await app.press("Cancel");
  await app.waitUntil(() => !app.shows("text:Review skill update"));
  await app.press("Skill actions");
  await app.press("Update");
  await app.waitFor("text:Review skill update");
  await app.press("Apply update");
  await app.waitFor("text:Deployments to review");
  expect(commands.map(({ command, input }) => [command, input])).toEqual([
    ["library.prepare_skill_update", { skill_id: "skill-a" }],
    ["library.cancel_skill_revision", { preview_id: "preview-1" }],
    ["library.prepare_skill_update", { skill_id: "skill-a" }],
    ["library.apply_skill_revision", { preview_id: "preview-2" }],
  ]);
});
