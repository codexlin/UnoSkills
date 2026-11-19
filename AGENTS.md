# AGENTS.md — UnoSkills Engineering & GPUI Kit Guidelines

This document defines the mandatory rules, architecture boundaries, and design/coding standards for all AI coding agents and human contributors working on **UnoSkills**.
For deeper architectural diagrams, event flow explanations, and contributor playbooks, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and [docs/ROADMAP.md](docs/ROADMAP.md).

Normative terminology follows GPUI Kit's convention:
- **Must**: hard correctness, safety, or ecosystem constraint. Do not violate.
- **Should**: default architectural choice; depart only with an explicit, concrete reason.
- **May**: optional technique.

Official GPUI Kit references (must be respected for all UI work):
- Style: https://gpui-kit.com/docs/style/
- Design Guides: https://gpui-kit.com/docs/design-guides/
- Coding Guides: https://gpui-kit.com/docs/coding-guides/
- Scrollable: https://gpui-kit.com/component/scrollable/
- Assets & Icons: https://gpui-kit.com/docs/assets/
- Fonts: https://gpui-kit.com/docs/fonts/
- Auto Update: https://gpui-kit.com/docs/auto-update/

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
unoskills-app (GPUI Kit UI)   unoskills-cli (clap + --json)
            \                       /
             ▼                     ▼
      unoskills-core (pure Rust domain, SQLite WAL, sync, source, stacks)
```

- `unoskills-core` **must never** depend on `gpui-kit`, UI types, or windowing crates.
- `unoskills-app` and `unoskills-cli` **must** share the same `UnoSkillsManager` facade and `~/.unoskills/unoskills.db` (SQLite WAL + `busy_timeout`).
- Community registry access **must** stay behind the `unoskills_core::stacks::StackRegistry` trait so Stage 1 (built-in + GitHub static `index.json`) can evolve to Stage 2 (lightweight API Hub) without rewriting UI views.

---

## 3. GPUI Kit Design Guides (`gpui-kit.com/docs/design-guides/`)

### Core Design Thesis
1. **Clarity before personality**: make the primary task and next action immediately obvious before adding visual decoration.
2. **Composition before invention**: compose existing `gpui-kit` components rather than inventing raw custom widgets.
3. **Tokens before values**: always pull colors and semantic tokens from `cx.theme()` (`theme.background`, `theme.foreground`, `theme.muted_foreground`, `theme.border`, `theme.primary`, `theme.accent`, `theme.secondary`, `theme.danger`, `theme.warning`, etc.). Never hardcode hex/RGB colors that break Light/Dark theme switching.
4. **Desktop before web convention**: honor desktop traits — keyboard shortcuts (`Enter` to search/install), native window chrome (`TitleBar`), resizable split panes, dense scannable lists, and crisp focus/selection states.
5. **Visible state**: hover, focus, selection, disabled, loading (`Spinner`, `.loading(true)`), validation (`Alert`), and destructive (`.danger()`) states **must** be visually distinct and consistent.
6. **Geometry-driven containment**: radii, insets, and borders must form a coherent hierarchy. Use theme radius tiers (`rounded_md`, `rounded_lg`); for ordinary rectangles, corner radius **must** stay visibly smaller than half the shorter side.

### Spacing & Typography Scale
- **Should** stick to the compact semantic spacing scale: **2, 4, 8, 12, 16, 24, 32 px** (`gap_1`, `gap_2`, `gap_3`, `gap_4`, `p_2`, `p_3`, `p_4`, `p_5`).
- **Should** stick to the clean typography scale: **12, 14, 16, 18, 20 px** (`text_xs`, `text_sm`, `text_base`, `text_lg`, `text_xl`).
- **Dialogs**: size a dialog strictly to its decision complexity (short confirmation vs. medium form). Never enlarge a dialog merely to create empty whitespace.

### Mandatory `gpui-kit` Component-First Policy
Before writing custom styled `div()` blocks, **must** check and prefer built-in `gpui_kit::component::*` widgets:
- Window & Chrome: `TitleBar`, `Sidebar` / `SidebarGroup` / `SidebarMenu` / `SidebarMenuItem` / `SidebarFooter`
- Split Layouts: `h_resizable` / `v_resizable` + `resizable_panel`
- Sub-navigation & Segmented Controls: `TabBar::segmented()` + `Tab`
  - **Gotcha**: `Tab::icon(...)` renders an **icon-only** tab and hides `label`. Use `Tab::new().label(...)` when text labels are expected.
- Containers & Dividers: `GroupBox::outline()` (or `.fill()`), `Separator::horizontal()` / `Separator::vertical()`
- Feedback & States: `WindowExt::push_notification(Notification::success/error/info/warning(...), cx)`, `Alert::warning/info/error(...).banner()`, `Empty` / `EmptyHeader` / `EmptyMedia` / `EmptyTitle` / `EmptyDescription` / `EmptyContent`, `Spinner`
- Controls & Content: `Button`, `Switch`, `Input`, `Textarea`, `TextView` (`TextView::new(&Entity<TextViewState>)`), `Tag`, `Badge`, `Dialog` / `AlertDialog`, `Sheet`, `DataTable`

---

## 4. GPUI Kit Style, Layout & Scrollable Rules (`gpui-kit.com/docs/style/`, `/component/scrollable/`)

- **Style chains are frame-local**: fluent `Styled` / `StyledExt` chains describe the current frame's presentation using strongly typed Rust values, not persistent CSS stylesheets.
- **Prevent horizontal overflow on narrow/default windows (Must)**:
  - Every `h_flex()` header or list row containing variable-length text **must** place `.flex_1().min_w_0()` (and `.truncate()` where appropriate) on the text column, and `.flex_shrink_0()` on right-aligned buttons, badges, switches, or `TabBar`s.
  - Never place a long wrapping description and multi-button action bar side-by-side in a rigid single-row `h_flex()` without wrapping or stacking.
- **Scroll ownership & edge anchoring (Must)**:
  - Attach `.overflow_y_scrollbar()` (or `.overflow_scrollbar()`) to the element owning the full panel viewport with `.flex_1().min_h_0()`.
  - Place content padding (`p_4`, `p_5`) on an **inner** child element, never on the scrollable viewport itself, so the scrollbar anchors cleanly flush against the panel edge.
- **Flex height constraint (Must)**: every ancestor chain above a scrollable region or `uniform_list` **must** carry `.min_h_0()` and `.overflow_hidden()` so Taffy constrains viewport height instead of expanding to content height.

---

## 5. GPUI Kit Coding & Performance Rules (`gpui-kit.com/docs/coding-guides/`)

- **Retained state vs. declarative `render()` (Must)**:
  - Keep `render()` cheap and side-effect free.
  - Default to `RenderOnce` for lightweight value-like components; use `Entity<T>` only for retained state (inputs, editors, rich text state, subscriptions, async owners).
  - **Never** construct `InputState`, `TextareaState`, `TextViewState`, `SelectState`, or `UniformListScrollHandle` inside `render()`; store them on the owning feature `Entity` (`LibraryView`, `StacksView`, `DiscoverView`, `SettingsView`).
- **Markdown rendering (Must)**:
  - Always retain `Entity<TextViewState>` and update via `state.set_text(&body, cx)` only when selection or file content changes. **Never** call `TextView::markdown(...)` directly inside `render()` — doing so re-parses the full Markdown AST on every scroll frame.
  - Always pass `unoskills_core::parser::strip_frontmatter(&md)` to preview `TextViewState` so YAML frontmatter's closing `---` is never parsed as a CommonMark Setext `<h2>`. Keep raw frontmatter intact in edit mode.
- **Virtualized lists for unbounded / medium-large datasets (Must)**:
  - Any list that can exceed ~25 rows (`Library`, `Marketplace`, `Adopt Existing`) **must** use `gpui_kit::uniform_list` + `UniformListScrollHandle` + `.vertical_scrollbar(&handle)` so only visible viewport rows are constructed per frame.
- **Async work & Tasks (Must)**:
  - Never spawn async work unconditionally inside `render()`. Trigger background tasks from user events, lifecycle hooks, or named navigation methods.
  - Run blocking filesystem/git/network operations on `cx.background_executor()`, never on the UI thread.
  - Capture weak entity handles (`cx.spawn_in(window, async move |this, cx| ...)`) so closed views are not kept alive.
  - Always surface explicit loading (`Spinner` / `.loading(true)`) and error (`Notification::error`) states to the user — never swallow errors silently.
- **Dev build smoothness (Should)**:
  - Keep `[profile.dev.package."*"] opt-level = 2` in workspace `Cargo.toml` so GPUI, Taffy, cosmic-text, and wgpu remain fluid during local development.

---

## 6. Internationalization (i18n), Assets & Fonts Rules

- **Must** keep full **Simplified Chinese (`zh-CN`)**, **English (`en`)**, and **Japanese (`ja`)** parity for every user-facing string in `crates/unoskills-app/src/i18n/{mod,zh_cn,en,ja}.rs`.
- **Must not** hardcode user-visible prose literals directly in UI files (`shell.rs`, `components/*.rs`, `features/**`).
- Language selector in Settings **should** use `Select` dropdown (scalable to additional locales).
- Switching language **must**:
  1. Call `gpui_kit::component::set_locale(lang.code())` so built-in `gpui-kit` controls stay in sync.
  2. Update live input placeholders via `InputState::set_placeholder` / `TextareaState::set_placeholder`.
  3. Refresh language-aware font selection when UI font is set to `Auto` (`Theme::update`).
  4. Persist the chosen locale to SQLite `settings.language`.
- **Custom Icons & Assets**:
  - Embed app brand icons (`crates/unoskills-app/assets/icons/uno-mark.svg`) inside the app crate via a composite `AssetSource` that falls back to `gpui_kit::assets::Assets`, exposed through a typed `IconNamed` enum.
- **Fonts**:
  - Configure UI & monospace fonts globally through `Theme::update(cx, |theme| ...)` using `cx.text_system().all_font_names()` for system font discovery; avoid hardcoding font families per-element.
- **Auto Update**:
  - Use `gpui-updater-core` / `gpui-updater-pre` (matching `gpui-kit`'s `gpui-pre` runtime) with GitHub Releases / static manifest + Ed25519 signature verification; never auto-install silently without explicit user click.

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

## 8. Verification Checklist Before Completing Work

Before reporting any task complete:
1. Run `cargo test --workspace` — all unit & integration tests **must** pass. Any new core behavior or bug fix **should** include a focused test.
2. Run `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` — **must** finish with **0 errors and 0 warnings** (fix unused imports/dead fields immediately).
3. When UI code changes, run `cargo build -p unoskills-app` to confirm linking succeeds cleanly.

### Desktop GUI Verification (`agent-desktop`)
Use `agent-desktop` to see and operate desktop apps. Before the first use, run
`agent-desktop skills get desktop` and `agent-desktop skills get platform`, and follow them.
