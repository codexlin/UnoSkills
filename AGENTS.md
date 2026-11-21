# AGENTS.md — UnoSkills Engineering & Native Shell Guidelines

This document defines the mandatory rules, architecture boundaries, and design/coding standards for all AI coding agents and human contributors working on **UnoSkills**.
For deeper architectural diagrams, event flow explanations, and contributor playbooks, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and [docs/ROADMAP.md](docs/ROADMAP.md).

Normative terminology:
- **Must**: hard correctness, safety, or ecosystem constraint. Do not violate.
- **Should**: default architectural choice; depart only with an explicit, concrete reason.
- **May**: optional technique.

Desktop UI work must use the APIs documented for Native Shell v0.6.1.

- [Architecture, accessibility, reactivity, and packaging](https://github.com/longbridge/nsh/tree/v0.6.1/website/docs)
- [Element guides](https://github.com/longbridge/nsh/tree/v0.6.1/website/elements)
- [Base components](https://github.com/longbridge/nsh/blob/v0.6.1/website/modules/nsh-base.md)

---

## 1. Product Discipline (Anti-Bloat Rules)

UnoSkills is a **fast, lean, native Rust AI Skills & Stacks Workbench** — not a bloated dashboard.

- **Must** keep the desktop shell focused on its **4 core top-level views**:
  1. `Library` (技能库) — inspect, preview/quick-edit `SKILL.md`, lint/token budget, 1-click per-agent deploy.
  2. `Stacks` (技能组合) — curated & personal declarative `.unoskills-stack.toml` recipes, 1-click install & share.
  3. `Discover` (发现安装) — Quick Install, `skills.sh` Marketplace, and Adopt unmanaged skills.
  4. `Settings` (设置) — language, deploy mode, appearance, and declarative agent registry.
- **Must** stay **Local-First**: core local management, stack installation, and TOML import/export must never require login. Community identity (when added) uses GitHub OAuth Device Flow only.
- **Must** keep agent definitions declarative in `crates/unoskills-core/data/agents.toml` and starter stacks in `crates/unoskills-core/data/featured_stacks.toml`. Do not scatter hardcoded agent paths across UI code.
- **Should** keep `SKILL.md` editing lightweight (quick tweaks + save); do not turn UnoSkills into a full IDE.

---

## 2. Workspace Architecture & Layer Boundaries

Dependencies **must** strictly flow downwards:

```text
unoskills-app (NSH TSX + Rust host)   unoskills-cli (clap + --json)
            \                       /
             ▼                     ▼
      unoskills-core (pure Rust domain, SQLite WAL, sync, source, stacks)
```

- `unoskills-core` **must never** depend on NSH, UI types, or windowing crates.
- `unoskills-app` and `unoskills-cli` **must** share the same `UnoSkillsManager` facade and `~/.unoskills/unoskills.db` (SQLite WAL + `busy_timeout`).
- Community registry access **must** stay behind the `unoskills_core::stacks::StackRegistry` trait so Stage 1 (built-in + GitHub static `index.json`) can evolve to Stage 2 (lightweight API Hub) without rewriting UI views.
- **Should** compose pages and own transient interaction state in TSX. **Must** keep shared persistence and deployment rules in the Rust core. The desktop host adapts native capabilities and typed commands. See the [placement rules and worked example](docs/ARCHITECTURE.md#choosing-tsx-or-rust).

---

## 3. Desktop UI

- **Must** use the shared theme tokens for Light, Dark, and System appearance and support keyboard focus, selection, disabled, loading, errors, and destructive actions.
- **Must** prefer documented NSH base controls for input, selection, dialogs, progress and buttons. Supply accessible names, meaningful focus targets, and keyboard shortcuts.
- **Should** use a compact, consistent spacing and type scale. Put path and label text in a shrinking column so actions remain visible on narrow windows.
- **Must** give scroll containers a constrained viewport and use virtualized NSH lists for growing datasets such as Library, Marketplace, and Adopt.

## 4. Desktop State and Host Calls

- Follow the [NSH v0.6.1 coding guide](https://github.com/longbridge/nsh/blob/v0.6.1/website/zh-CN/docs/coding-guides.md) for reactive ownership, shortcuts, and stable list keys.
- Keep screen state in NSH signals, computed values and event handlers. Use effects for async requests or platform services and discard outdated responses.
- Keep blocking filesystem, database, Git and HTTP work in the Rust host off the UI and script threads. Use `UnoSkillsManager` for domain operations and preserve core error codes in host replies.
- Preserve raw frontmatter when editing. Pass `unoskills_core::parser::strip_frontmatter` content to the Markdown preview when the selection or file changes.
- Use compound navigation data for project scope. IDs are strings, and the empty scope path selects the project root.
- **Must** bind page shortcuts to the active screen and async editor results to the record that initiated the operation. Cover regressions through user actions such as switching pages during a pending read or save.

## 5. Localization and Assets

- **Must** maintain Simplified Chinese (`zh-CN`), English (`en`), and Japanese (`ja`) parity in the desktop locale tables.
- Locale switching updates the NSH locale, reactive labels and input placeholders, Auto font selection, and SQLite's `settings.language`.
- Use the app's embedded assets and application identity across platforms. Keep theme and typography settings compatible with existing desktop installations.

## 6. App Updates

- Use the existing Rust guided updater with GitHub Releases and SHA-256 verification when a checksum asset exists.
- Check on every launch and offer manual rechecks. Only download an installer or launch it after the user selects the respective action.

---

## 7. Core Filesystem, Skill Format & Sync Safety Rules (Hard Constraints)

These safety invariants are non-negotiable (informed by real-world production incidents in skill managers):

1. **Strict Skill Detection**:
   - Recognize only `SKILL.md` (canonical) and `skill.md` (legacy compatibility).
   - **Must never** classify `README.md` or `CLAUDE.md` as skill entry markers.
2. **No Silent Clobbering (`ReplacePolicy`)**:
   - Classify targets using `std::fs::symlink_metadata` — never `Path::is_dir()` (which follows symlinks/junctions).
   - Default deployments **must** use `ReplacePolicy::NoClobber`. Refuse unmanaged directories/files with `TARGET_CONFLICT` rather than deleting user data.
   - `remove_managed_target` **must** verify on-disk type matches recorded `DeployMode` before deleting anything.
3. **Shared-Directory Reference Counting**:
   - Multiple agents can resolve to the same physical skills directory (e.g. `~/.agents/skills`, `~/.config/agents/skills`).
   - Undeploying an agent **must** check `store.count_deployments_at_path(&target_path, agent_key)` and remove the on-disk link/copy **only** when count == 0.
4. **Path & Overlap Safety**:
   - Every directory slug **must** pass `parser::sanitize_slug` (strips traversal `..`, rejects Windows reserved characters and device names).
   - Every copy/link operation **must** pass `sync::ensure_dst_not_overlapping_src` to prevent infinite recursive copy or source deletion.
5. **CLI Safety**:
   - Destructive CLI commands (`remove`) **must** require `--yes`.
   - `--json` mode **must** emit clean, parseable JSON to stdout/stderr with non-zero exit codes on error.

---

## 8. Git Workflow & Verification Checklist Before Completing Work

### Branch & Pull Request Workflow (Should)
- **Should** develop features, fixes, and chore updates on dedicated branches (`feat/<topic>`, `fix/<topic>`, `chore/<topic>`) rather than committing directly to `main`.
- **Should** use Conventional Commits (`feat(scope): ...`, `fix(scope): ...`, `chore(scope): ...`) so `git-cliff` and `.github/release.yml` automatically categorize Release Notes.
- **Should** open a Pull Request against `main` and let 3-OS CI (`ubuntu-latest`, `macos-latest`, `windows-latest`) pass before merging.

### Verification Checklist
Before reporting any task complete:
1. Run `cargo test --workspace` — all unit & integration tests **must** pass. Any new core behavior or bug fix **should** include a focused test.
2. Run `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` — **must** finish with **0 errors and 0 warnings** (fix unused imports/dead fields immediately).
3. When UI code changes, run `cargo build -p unoskills-app --locked`, `nsh check crates/unoskills-app/ui`, `nsh build crates/unoskills-app/ui --release`, and `nsh test crates/unoskills-app/ui`. Match the evidence to the [layer changed](docs/ARCHITECTURE.md#review-and-evidence).

### Desktop GUI Verification (`agent-desktop`)
Use `agent-desktop` to see and operate desktop apps. Before the first use, run
`agent-desktop skills get desktop` and `agent-desktop skills get platform`, and follow them.
