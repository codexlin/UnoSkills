# UnoSkills Architecture & Engineering Best Practices

This document explains how **UnoSkills** is structured, why key technical decisions were made, and the patterns contributors should follow when extending the core engine, CLI, or native GPUI Kit desktop application.

> Looking for the short, mandatory rules checklist for AI agents and PR reviews? See [AGENTS.md](../AGENTS.md).

---

## 1. Workspace Layering & Crate Boundaries

UnoSkills uses a strict **3-crate Cargo workspace** where dependencies flow strictly downwards:

```text
  unoskills-app (GPUI Kit GUI)       unoskills-cli (clap + --json)
                 \                           /
                  ▼                         ▼
         unoskills-core (pure Rust domain, SQLite WAL, sync, source, stacks)
```

- **`crates/unoskills-core`**: Zero UI or windowing dependencies. Owns domain types, strict `SKILL.md` parsing/linting, SQLite WAL persistence (`~/.unoskills/unoskills.db`), filesystem sync safety, `skills.sh` marketplace client, adopt scanner, and declarative Stack registry.
- **`crates/unoskills-cli`**: Thin `clap` wrapper around `UnoSkillsManager` with machine-readable `--json` output for scripts and AI agents.
- **`crates/unoskills-app`**: Native desktop workbench built on `gpui-kit` (`0.10.0`). Never touches SQLite or raw filesystem sync directly — every mutation passes through `UnoSkillsManager`.

### Crate-Internal Embedded Assets (`cargo publish` Ready)

All `include_str!` / `include_bytes!` assets live **inside** the crate that embeds them, never via `../../../` paths escaping crate roots:

- `crates/unoskills-core/data/agents.toml` — declarative registry of 22 AI coding agents.
- `crates/unoskills-core/data/featured_stacks.toml` — curated starter Skill Stacks.
- `crates/unoskills-app/assets/icons/` — embedded brand SVGs (`uno-mark.svg`, `uno-app-icon.svg`).
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

### Module Layout

```text
crates/unoskills-app/src/
├── main.rs            # App entry, UnoAssets registration, Window bootstrap
├── assets.rs          # Composite AssetSource (Uno brand SVGs -> gpui-kit fallback) + AppIcon
├── fonts.rs           # System font enumeration, language-aware Auto fonts, Theme::update
├── shell.rs           # UnoSkillsShell: TitleBar, collapsible Sidebar, active tab host
├── state.rs           # WorkspaceState entity, WorkspaceEvent bus, async orchestrators
├── components/        # Pure RenderOnce reusable UI primitives (PageHeader, TokenBadge, StatusBadge)
├── i18n/              # Modular locale tables: mod.rs, zh_cn.rs, en.rs, ja.rs
└── features/
    ├── library/       # mod.rs (state + split), list.rs (virtualized), inspector.rs (preview/edit/deploy)
    ├── stacks/        # mod.rs, browser.rs, detail.rs, editor.rs (create + TOML import)
    ├── discover/      # mod.rs (caching + seq guards), marketplace.rs, adopt.rs
    └── settings/      # mod.rs (select subscriptions), sections.rs (general/fonts/updates/agents)
```

### Event-Driven State Flow (`WorkspaceState` + `WorkspaceEvent`)

Instead of a monolithic "God Entity" passing mutable state into stateless view functions, UnoSkills splits ownership across a shared **`WorkspaceState`** entity and 4 independent feature view entities (`LibraryView`, `StacksView`, `DiscoverView`, `SettingsView`):

```text
  User Action in FeatureView
        │
        ▼
  workspace.update(cx, |ws, cx| ...)
        │  (persists to SQLite / runs background executor)
        ▼
  cx.emit(WorkspaceEvent::...)  ──► GPUI deferred effects queue (re-entrancy safe)
        │
        ├──► UnoSkillsShell  (updates active tab, sidebar counts, TitleBar spinner)
        ├──► LibraryView     (refilters indices, refreshes preview only on Reloaded)
        ├──► StacksView      (re-reads stack snapshots on Reloaded)
        ├──► DiscoverView    (clears per-row spinner on InstallFinished, updates adopt list)
        └──► SettingsView    (syncs locale/font Select widgets on LanguageChanged/SettingsChanged)
```

**Why `WorkspaceEvent` is split finely:**
- `Reloaded` — domain snapshots changed (skills/stacks/agents/adopt); triggers `LibraryView` to refresh `SKILL.md` preview (while protecting unsaved edits if `editing_markdown == true`).
- `SettingsChanged` — lightweight preference change (theme, fonts, font size, deploy mode, update check status); avoids forcing `LibraryView` to re-read disk files or re-parse Markdown AST.
- `LanguageChanged` — updates live `InputState`/`TextareaState` placeholders and rebuilds `SelectState` "Auto (...)" labels across views.
- `BusyChanged` — redraws the TitleBar spinner.
- `InstallFinished(String)` — clears the exact row-level loading spinner in `DiscoverView` without racing with other concurrent tasks.
- `NavigateTo(NavTab)` — single funnel for all tab transitions (sidebar clicks, empty-state buttons, post-install jump to Library, auto-loading Trending on first Discover visit).

---

## 4. GPUI Kit Performance & UX Best Practices

All UI code follows the official GPUI Kit guides (`docs/style`, `docs/design-guides`, `docs/coding-guides`, `component/scrollable`, `docs/assets`, `docs/fonts`).

### 1. Zero Heavy Work Inside `render()`
- **Retained Markdown AST**: `LibraryView` stores `Entity<TextViewState>` and calls `state.set_text(&stripped_body, cx)` only when selection or saved file changes. Never call `TextView::markdown(...)` inside `render()`.
- **Zero-Clone Filtering**: `LibraryView` and `DiscoverView` maintain `filtered_indices: Vec<usize>`, recomputed on `InputEvent::Change` and `WorkspaceEvent::Reloaded`. `render()` indexes directly into `&[Skill]` / `&[AdoptCandidate]` without cloning vectors.
- **List Virtualization**: All lists that can grow beyond ~25 items (`Library`, `Marketplace`, `Adopt`) use `uniform_list` + `UniformListScrollHandle` + `.vertical_scrollbar(&handle)`.
- **Smooth Dev Builds**: Workspace `Cargo.toml` sets `[profile.dev.package."*"] opt-level = 2` so GPUI, Taffy, cosmic-text, and wgpu run smoothly even in debug builds.

### 2. Overflow & Scrollbar Hygiene
- Every variable-length text column inside `h_flex()` carries `.flex_1().min_w_0()` (and `.truncate()` for single-line text), while action buttons, badges, and `TabBar`s carry `.flex_shrink_0()`.
- Scrollbars (`.overflow_y_scrollbar()`) live on the full-viewport element with `.flex_1().min_h_0()`; content padding (`p_4`/`p_5`) goes on an inner container so scrollbars anchor cleanly flush against panel edges.
- Every ancestor above a scrollable container or `uniform_list` includes `.min_h_0().overflow_hidden()`.

### 3. Concurrency & Race-Condition Safety
- Blocking filesystem, Git, and HTTP calls always run on `cx.background_executor()`.
- Concurrent tasks register via `begin_busy(label, cx) -> u64` and release via `end_busy(token, cx)` so overlapping operations never prematurely dismiss the TitleBar spinner.
- Async leaderboard/search requests in `DiscoverView` carry a monotonic `market_req_seq: u64` so slow out-of-order responses are ignored instead of overwriting newer results.

### 4. Theme, Fonts, Assets & i18n
- **Appearance + Fonts**: `WorkspaceState::set_theme_mode` persists `"dark"`/`"light"` in SQLite, calls `Theme::change`, and immediately re-applies `fonts::apply_fonts_to_theme` because `Theme::change` reloads theme defaults.
- **Language-Aware Auto Fonts**: When UI font is `"auto"`, `fonts.rs` queries `cx.text_system().all_font_names()` and picks the sharpest installed font for the active locale (`zh-CN` / `ja` / `en`).
- **Component Locale Sync**: Switching language calls `gpui_kit::component::set_locale(lang.code())` so built-in GPUI Kit controls (search inputs, dialogs, empty states) translate in lockstep.
- **Custom Brand Icons**: `UnoAssets` implements `AssetSource`, serving embedded brand SVGs first and delegating all other paths to `gpui_kit::assets::Assets`.

---

## 5. Contributor Playbooks

### Add a New AI Agent
1. Append an `[[agent]]` entry to `crates/unoskills-core/data/agents.toml`.
2. Run `cargo test -p unoskills-core` (`loads_declarative_builtin_agents` validates TOML parsing).

### Add a New Language (Locale)
1. Copy `crates/unoskills-app/src/i18n/en.rs` to `crates/unoskills-app/src/i18n/<locale>.rs` and translate every field of `Strings` (the Rust compiler enforces 100% key parity).
2. Register the new `Lang` variant in `crates/unoskills-app/src/i18n/mod.rs` (`ALL`, `code`, `display_name`, `short_label`, `next`, `index`, `from_code`, `t`, and dynamic formatters).
3. Add preferred default system fonts for that language in `crates/unoskills-app/src/fonts.rs` (`resolve_ui_font`).

### Add a New Featured Stack
1. Append a `[[stack]]` entry to `crates/unoskills-core/data/featured_stacks.toml` (following [STACKS_SPEC.md](./STACKS_SPEC.md)).
2. Run `cargo test -p unoskills-core` (`loads_featured_and_roundtrips_toml` verifies schema + round-trip serialization).

### Pre-PR Verification Gates (Identical to CI)
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p unoskills-app
```
