# UnoSkills — PRODUCT.md

## Product Summary
UnoSkills is a fast, lean, local-first workbench for managing AI coding agent skills (`SKILL.md`) and declarative skill stacks (`.unoskills-stack.toml`). The Native Shell desktop app and CLI share a Rust core.

Its core architectural value is **One Source, Many Agents**: a single canonical central skill library (`~/.unoskills/skills/`) deployed via instant symlink or copy across Claude Code, Codex, Antigravity, Cursor, Gemini CLI, Windsurf, OpenCode, Kimi Code CLI, and custom agents — both globally and per-project.

## Target Users & Personas
- **Multi-Agent AI Engineers (Power Users)**: Developers running 3–8 coding agents daily who need one glance to see which skills are active on which agents, toggle deployments in one click, and keep token budgets lean (`<4k tokens` per `SKILL.md`).
- **Workflow Curators & Team Leads**: Engineers who bundle reusable skill recipes into `.unoskills-stack.toml` stacks, share them via GitHub Gists, or sync their plain-text skills & stacks across machines via a private Git backup repo.

## Core Views

Home opens local workspace shortcuts, recent projects and skills, and skills.sh trends. Projects is reached from Home and the relevant skill workflows.

1. **Library (技能库)** — Skill index with a deployment inspector for Global and Project scopes, token budget linting, and lightweight `SKILL.md` preview and editing.
2. **Stacks (技能组合)** — Featured, community (`StackRegistry`), and personal `.unoskills-stack.toml` recipes with 1-click install and secret Gist sharing.
3. **Discover (发现安装)** — Git/local Quick Install, `skills.sh` Marketplace leaderboard & search, and unmanaged local skill adoption.
4. **Settings (设置)** — Sub-navigated preferences for General, Appearance & Fonts, Sync & Network (Proxy + Private Git Backup), Agent Registry, and In-App Guided Updater.

## Brand & Visual Commitments
- **Mode**: **Operate** — scanability, spatial predictability, native desktop speed, and state clarity outrank decorative expression.
- **Visual direction**. Keep the workbench calm and compact. The active light or dark preset supplies surfaces and action colors. Use proportional text for interface labels and prose, and monospace for paths and code. [Desktop design standards](DESIGN.md)
