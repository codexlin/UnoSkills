# UnoSkills — PRODUCT.md

## Product Summary
UnoSkills is a fast, lean, local-first native Rust desktop & CLI workbench (`unoskills-app` on GPUI Kit + `unoskills-cli` + `unoskills-core`) for managing AI coding agent skills (`SKILL.md`) and declarative skill stacks (`.unoskills-stack.toml`).

Its core architectural value is **One Source, Many Agents**: a single canonical central skill library (`~/.unoskills/skills/`) deployed via instant symlink or copy across Claude Code, Codex, Antigravity, Cursor, Gemini CLI, Windsurf, OpenCode, Kimi Code CLI, and custom agents — both globally and per-project.

## Target Users & Personas
- **Multi-Agent AI Engineers (Power Users)**: Developers running 3–8 coding agents daily who need one glance to see which skills are active on which agents, toggle deployments in one click, and keep token budgets lean (`<4k tokens` per `SKILL.md`).
- **Workflow Curators & Team Leads**: Engineers who bundle reusable skill recipes into `.unoskills-stack.toml` stacks, share them via GitHub Gists, or sync their plain-text skills & stacks across machines via a private Git backup repo.

## Core Views (Strict 4-View Anti-Bloat Discipline)
1. **Library (技能库)** — Skill × Agent deployment matrix (Global & Project scopes), token budget linting, and lightweight `SKILL.md` preview/quick-edit.
2. **Stacks (技能组合)** — Featured, community (`StackRegistry`), and personal `.unoskills-stack.toml` recipes with 1-click install and secret Gist sharing.
3. **Discover (发现安装)** — Git/local Quick Install, `skills.sh` Marketplace leaderboard & search, and unmanaged local skill adoption.
4. **Settings (设置)** — Sub-navigated preferences for General, Appearance & Fonts, Sync & Network (Proxy + Private Git Backup), Agent Registry, and In-App Guided Updater.

## Brand & Visual Commitments
- **Mode**: **Operate** — scanability, spatial predictability, native desktop speed, and state clarity outrank decorative expression.
- **Visual Direction**: **Calm & Precise (Carbon Indigo)** — three-tier surface luminance (`L0 Rail` → `L1 Canvas` → `L2 Card`) instead of harsh pitch-black wireframes; restrained indigo brand accent (`#5E6AD2` dark / `#4F5BCE` light); proportional sans-serif for UI chrome & Markdown prose paired with monospace strictly for file paths, token counts, hashes, and code.
- **Color Discipline**: Metadata (token counts, source type, tags) is rendered as quiet neutral text; color is reserved exclusively for actionable or semantic states (Accent = primary/selected, Success = deployed/verified, Warning = token budget alert/conflict/unmanaged, Danger = delete/detach).
