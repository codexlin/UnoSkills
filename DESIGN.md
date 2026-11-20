# UnoSkills — DESIGN.md (UI redesign spec, draft v1)

Status: proposal. No code changes yet. Source: `/impeccable critique` of the v0.3.x UI (design health 25/40).
Surface mode: **Operate** — the user is here to get a job done (deploy skills to agents), so scanability,
consistency and native expectations outrank expression. Brand lives in precise details.

## 1. Product idea → signature move

**One source, many agents.** UnoSkills keeps one central copy of every skill and fans it out to N agents.
The UI should make that fan-out visible at a glance.

**Signature move: the Deployment Matrix** — rows are skills, columns are agents, each cell is a deploy-state dot.
It replaces the current "list + per-skill chip row" as the Library's primary view. Everything else stays standard
(`Sidebar`, `TabBar::segmented`, `Input`, `Button`, `DataTable`/`uniform_list`).

## 2. Tone & direction

Calm and precise. Dense but readable. One brand accent, quiet neutrals, colour only where it carries meaning.
Not a terminal costume, not a web dashboard.

## 3. Colour roles (applied through the gpui-kit `Theme`, never hardcoded per element)

| Role | Meaning | Used for | Proposed (dark) |
|---|---|---|---|
| Accent (brand) | "this is yours / selected / primary" | primary buttons, selection, active nav, focus ring | `#6C8CFF` |
| Success | deployed, verified, up to date | deployed dots, `SHA-256 Verified`, `Verified` stack | `#4CAF7A` |
| Warning | needs attention | lint/token-budget warning, unmanaged skills, conflicts | `#E0A030` |
| Danger | destructive | remove, detach | `#E5534B` |
| Neutral | everything else | metadata: tokens, source, path, tags | theme `muted_foreground` |

Rules:
- Token counts, source type, tags are **metadata → neutral text**, never a coloured badge.
- "Will Install" is a *state of the stack row*: neutral by default, accent only on the primary Install action.
- Max one accent-coloured element per region (the primary action).
- Light theme mirrors the same roles (re-tune lightness, keep hue); verify AA contrast (4.5:1) for body text on both.
- Brand icon: keep the white mark, put it on the accent (or keep `#202024` tile + accent dot) so the brand has a colour.

## 4. Typography

- **UI chrome & prose: proportional sans** (`Auto` picks per language: Inter/system-ui for en, system CJK sans for zh/ja).
- **Mono only for**: file paths, `SKILL.md` editor/preview code blocks, agent ids, TOML.
- Scale (px): 12 meta · 14 body (default) · 16 section title · 20 page title. Muted text is 12px minimum and must pass AA.
- Never use 12px muted for primary information (e.g. skill description in list rows becomes 13–14px `foreground` at 70% emphasis).

## 5. Density & spacing

Compact scale only: 2 · 4 · 8 · 12 · 16 · 24. Row heights: list/matrix row 40px, dense table 32px, form row 56px.
Radii: `rounded_md` for controls/rows, `rounded_lg` for panels. No nested bordered boxes: one container level per region,
separate groups with spacing or a single `Separator`, not another border.

## 6. Shell

```
┌ TitleBar: [mark] UnoSkills ···················· [busy status] [update pill] [theme] ┐
├ Sidebar ─────────┬ Content ────────────────────────────────────────────────────────┤
│ Library      12  │  PageHeader (title + subtitle + right slot)                     │
│ Stacks           │  Toolbar (search · scope · bulk actions)                        │
│ Discover    +58  │  Main region                                                    │
│ Settings         │                                                                 │
│ ───────────────  │                                                                 │
│ ● 6 agents  ▾    │  (footer opens agent-status popover)                            │
└──────────────────┴─────────────────────────────────────────────────────────────────┘
```

- TitleBar keeps: mark + name, busy status, update pill, theme toggle. **Remove** version tag (→ Settings › About), language toggle (→ Settings › General), refresh (→ Library toolbar).
- Sidebar badges only for **actionable** counts: Discover `+N unmanaged` (dismissible), Settings dot when an update exists. Stacks/Library show no catalogue counts.
- Footer "N agents" becomes a popover listing detected agents + path + enable switch (no need to open Settings).
- Fix collapse-button tooltip (currently "Workspace").
- Set `window_min_size` (~960×600) and test at the minimum.
- All three views use `PageHeader` (Library and Settings currently don't).

## 7. Library (primary redesign)

Layout: `h_resizable` — **Matrix (left, flexible)** | **Inspector (right, 360–480px, collapsible)**.

Matrix toolbar: `Input` search (ctrl-K) · **Scope segmented control** `[ Global | proj-a | proj-b | + ]` ·
sort · `Install` (→ Discover). Scope is a `TabBar::segmented`, not primary/outline buttons.

Matrix columns: ☐ select · Skill (name + one-line description) · **one column per enabled agent** (glyph header + tooltip name) ·
Tokens (right-aligned, neutral) · state.
Cell states: ● deployed (success) · ○ available (click to deploy) · ◌ conflict/unmanaged target (warning, tooltip explains `TARGET_CONFLICT`) · – agent not installed.
Row actions on hover only: open, reveal, remove.

Multi-select → sticky action bar: `Deploy to… ▾` · `Remove from…` · `Delete`. Keyboard: ↑/↓ move, Space toggle cell/row, Enter open inspector, Esc clears selection.

Inspector (top → bottom, one priority each): title + source + path (one line) → primary action row (`Edit SKILL.md` primary, overflow `…` menu for Update/Reveal/Delete) →
lint / token-budget `Alert` (only if present) → description → rendered `SKILL.md`. Deployment controls move into the matrix; the inspector shows a read-only "Deployed to" summary with a link back to the row.
Editor height fills the panel (remove fixed 460px).

Empty library (first run): explain in one sentence what a skill is, then 2 paths — "Browse stacks" (primary), "Adopt existing skills".

## 8. Stacks

List rows: name, 1-line description, `Verified` (success) only when true, tags as neutral text (no truncation of the count).
Detail: `Install Stack` primary (accent) · `Copy TOML` secondary. Per-skill rows show state as small icons (✓ installed / ＋ will install), no cyan tag per row.
Provide a pre-install summary line: "Installs 3 skills · ~7.2k tokens · to Claude Code, Codex".

## 9. Discover

Marketplace: unchanged structure; move `Load more` to infinite scroll with spinner; show install target summary before install.
Adopt: group by agent (collapsible), checkbox per row, sticky bar **"Adopt N selected · ~X tokens"** (+ `Select all in group`).
Remove one-click "Adopt All (58)" as the primary; keep it as a menu item with a confirmation that lists counts.
Row = name + short path (truncate middle) + tokens; single `Adopt` button only on hover/focus.

## 10. Settings

`PageHeader` + **left sub-nav** (`Sidebar`-style list): General · Appearance (theme, UI/mono font, base size) · Sync & Network (proxy, GitHub token, Git backup) · Agents · About & Updates.
Each section shows one `GroupBox`; setting rows 56px (label + description left, control right).
Update pill in TitleBar deep-links to **About & Updates** (new `WorkspaceEvent::NavigateTo` payload with section), shows version, check button, verified-download state.
Drop the `SQLite WAL` chip (internal detail); keep the library root path with a reveal button.

## 11. States & feedback

Loading `Spinner` / `.loading(true)`; errors `Notification::error` **plus** inline `Alert` where the field caused it (proxy, backup repo, token).
Destructive: `AlertDialog` with counts (already present). Add undo toast for "remove from agent" (restores symlink/copy).
Keyboard hints in tooltips via `KeyBinding` display.

## 12. Implementation notes (respect AGENTS.md)

- All new strings through `i18n/{mod,zh_cn,en,ja}.rs`; no literals in UI files.
- Matrix and Adopt lists use `gpui_kit::uniform_list` + `UniformListScrollHandle`; retain `Entity<*State>` outside `render()`.
- Colours/fonts via `Theme::update`; open question: confirm gpui-kit exposes accent/semantic overrides (otherwise ship a custom theme file).
- Suggested PR order on `feat/ui-redesign`: (1) tokens + typography + shell cleanup, (2) Library matrix + inspector, (3) Settings sub-nav + Adopt multi-select.
- Re-run `/impeccable critique` after each PR; target ≥ 32/40.
