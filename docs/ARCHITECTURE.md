# UnoSkills Architecture & Engineering Best Practices

This document explains how **UnoSkills** is structured, why key technical decisions were made, and the patterns contributors should follow when extending the core engine, CLI, or native NSH desktop application.

> Looking for the short, mandatory rules checklist for AI agents and PR reviews? See [AGENTS.md](../AGENTS.md).

---

## 1. Workspace Layering & Crate Boundaries

UnoSkills uses a strict **3-crate Cargo workspace** where dependencies flow strictly downwards:

```text
  unoskills-app (Native Shell)       unoskills-cli (clap + --json)
                 \                           /
                  ▼                         ▼
         unoskills-core (pure Rust domain, SQLite WAL, sync, source, stacks)
```

- **`crates/unoskills-core`**: Zero UI or windowing dependencies. Owns domain types, strict `SKILL.md` parsing/linting, SQLite WAL persistence (`~/.unoskills/unoskills.db`), filesystem sync safety, `skills.sh` marketplace client, adopt scanner, and declarative Stack registry.
- **`crates/unoskills-cli`**: Thin `clap` wrapper around `UnoSkillsManager` with machine-readable `--json` output for scripts and AI agents.
- **`crates/unoskills-app`**: Native desktop workbench built on Native Shell (`0.6.1`) with TSX screens. Never touches SQLite or raw filesystem sync directly — every mutation passes through `UnoSkillsManager`.

### Crate-Internal Embedded Assets (`cargo publish` Ready)

All `include_str!` / `include_bytes!` assets live **inside** the crate that embeds them, never via `../../../` paths escaping crate roots:

- `crates/unoskills-core/data/agents.toml` — declarative registry of 22 AI coding agents.
- `crates/unoskills-core/data/featured_stacks.toml` — curated starter Skill Stacks.
- `crates/unoskills-app/assets/icons/` — embedded brand mark (`uno-mark.svg`).
- `packaging/icons/` — installer/bundle metadata separate from Rust compilation.

---

## 2. Core Engine Design & Safety Invariants (`unoskills-core`)

`UnoSkillsManager` (`crates/unoskills-core/src/manager.rs`) is the single facade shared by both the desktop app and CLI.

### Filesystem & Sync Safety

1. **Strict Skill Detection (`parser.rs`)**:
   - Only `SKILL.md` (canonical) and `skill.md` (legacy compatibility) identify a skill directory.
   - `README.md` and `CLAUDE.md` are explicitly rejected to prevent false positives during Git or local directory scans.
   - `strip_frontmatter()` strips YAML `--- ... ---` headers cleanly for Markdown preview while keeping raw frontmatter intact for editing.
2. **No Silent Clobbering (`sync.rs`)**:
   - Target classification uses `std::fs::symlink_metadata` (never `Path::is_dir()`, which follows symlinks/junctions).
   - Deployments default to `ReplacePolicy::NoClobber`: if an unmanaged directory/file already exists at the target path, UnoSkills refuses with `TARGET_CONFLICT` instead of deleting user data.
   - `remove_managed_target` checks that on-disk type matches the recorded `DeployMode` (`Symlink` / `Copy` / `Junction`) before deleting anything.
3. **Shared-Directory Reference Counting (`store.rs` + `manager.rs`)**:
   - Multiple AI agents can share one physical skills folder (e.g., `~/.agents/skills`, `~/.config/agents/skills`).
   - Undeploying an agent queries `store.count_deployments_at_path(&target_path, agent_key)` and deletes the physical link/copy **only** when the remaining reference count is `0`.
4. **Path Traversal & Overlap Prevention**:
   - Every skill/stack slug passes `parser::sanitize_slug` (strips `..`, rejects Windows reserved names and invalid path characters).
   - Every copy/link operation passes `sync::ensure_dst_not_overlapping_src` to prevent recursive self-copying or source deletion.
5. **Concurrent CLI + GUI Storage**:
   - SQLite runs in `WAL` mode with `busy_timeout = 5000ms` so running `unoskills` CLI commands while the desktop app is open never fails with lock contention.

### Community Registry Abstraction

Community stacks live behind the `unoskills_core::stacks::StackRegistry` trait (`crates/unoskills-core/src/stacks.rs`). This isolates the UI from whether stacks come from embedded TOML, a static GitHub `index.json` (Stage 1), or an optional API hub (Stage 2). See [STACKS_SPEC.md](./STACKS_SPEC.md).

---

## 3. Desktop App Architecture (`unoskills-app`)

### Desktop entry and hosts

`crates/unoskills-app/build.rs` uses the pinned NSH bundler to compile `ui/App.tsx`.
The executable embeds the resulting JavaScript and opens it in NSH's GTK, AppKit, or Win32 host.
Installer names, application identity and the Rust updater remain compatible with existing UnoSkills installations.

`ui/src/Shell.tsx` owns navigation, the selected locale and theme, workspace revisions, and notifications.
`Home.tsx`, `Settings.tsx`, `Updates.tsx`, `library/`, and `projects/` own screen state and layout.
Links carry opaque IDs and an optional relative `scopePath`. An empty scope path selects the project root.

### TSX to Rust

`ui/src/shared.ts` calls `unoskills.command` over NSH's host channel. The request carries an explicit command name and
JSON input. The response contains the typed result or an error message and code. Rust dispatch lives in
`src/nsh/`. Blocking calls run on host worker threads and reach the same `UnoSkillsManager` as the CLI.
NSH's Rust file-dialog services supply the picker and reveal actions through `src/nsh/dialogs.rs`.

After a successful mutation, a screen calls `refresh()` to advance the shared revision. Subscribers use the revision
to update their own snapshots. The shell keeps visited screens mounted and exposes whether each screen is active. Bind shortcuts only while the screen is active. Retain draft text while refreshing data and discard obsolete async replies when the
target changes. Give scrollable regions a constrained viewport and use NSH's virtualized `List` for growing datasets.

### Desktop development

Use stable Rust 1.97 or later. Linux builds need GTK 4.14 or later and the development packages in CI.
The standalone CLI `nsh` v0.6.1 is used for TSX type checks and independent bundle checks. Cargo builds embed
the bundle with the Rust bundler, so developers and packaging jobs can build the desktop executable directly.

- [NSH v0.6.1 guide](https://github.com/longbridge/nsh/tree/v0.6.1/website/docs)
- [NSH v0.6.1 controls](https://github.com/longbridge/nsh/tree/v0.6.1/website/elements)
- [NSH v0.6.1 base API](https://github.com/longbridge/nsh/blob/v0.6.1/website/modules/nsh-base.md)

---

## 5. Contributor Playbooks

### Add a New AI Agent
1. Append an `[[agent]]` entry to `crates/unoskills-core/data/agents.toml`.
2. Run `cargo test -p unoskills-core` (`loads_declarative_builtin_agents` validates TOML parsing).

### Add a New Language (Locale)
1. Add the translated keys in `crates/unoskills-app/ui/src/i18n.ts` and the Library/Discover and Projects/Stacks locale tables.
2. Update the locale type in `ui/src/shared.ts` and the language choices in Settings.
3. Add the locale's automatic font choices in `ui/src/theme.ts`.

### Add a New Featured Stack
1. Append a `[[stack]]` entry to `crates/unoskills-core/data/featured_stacks.toml` (following [STACKS_SPEC.md](./STACKS_SPEC.md)).
2. Run `cargo test -p unoskills-core` (`loads_featured_and_roundtrips_toml` verifies schema + round-trip serialization).

### Pre-PR Verification Gates (Identical to CI)
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p unoskills-app --locked
nsh check crates/unoskills-app/ui
nsh build crates/unoskills-app/ui --release
```
