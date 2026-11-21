# UnoSkills Roadmap

UnoSkills is designed to remain a **fast, lean, local-first, open-source** native Rust workbench for managing AI Agent Skills & Skill Stacks.

---

## Delivered

### Core Engine (`unoskills-core`)
- [x] Central skill library (`~/.unoskills`) backed by SQLite WAL (`busy_timeout = 5000ms`)
- [x] Strict `SKILL.md` / `skill.md` parser, YAML frontmatter stripping, CJK-aware token estimation, lint checks, SHA-256 directory hashing
- [x] Safe sync engine: Symlink / Copy / Windows Junction, `ReplacePolicy::NoClobber`, overlap guard, shared-directory reference counting
- [x] Declarative 22-agent registry (`crates/unoskills-core/data/agents.toml`) with automatic installation detection
- [x] Multi-source installer: local folder, GitHub URL, `owner/repo@skill`, `skills.sh` leaderboard (Trending / Hot / All Time) & search
- [x] Unmanaged local skill scanner & adopter (single + bulk)
- [x] Declarative Skill Stacks (`.unoskills-stack.toml`): featured stacks, local creation, TOML import/export, incremental one-click install

### Native Desktop App (`unoskills-app`, Native Shell)
- [x] Home and four working areas with a collapsible sidebar
- [x] NSH TSX screens and reusable controls, with typed commands to the shared Rust manager
- [x] Embedded UnoSkills brand and packaged theme fonts
- [x] Simplified Chinese (`zh-CN`), English (`en`), and Japanese (`ja`) with NSH locale synchronization
- [x] Language-aware automatic UI fonts, searchable system UI & monospace font pickers, global font-size scaling
- [x] Persisted Dark / Light appearance without resetting custom fonts
- [x] Native virtualized lists, selectable `TextView` Markdown, and a skill-bound source editor with unsaved-edit protection
- [x] Native confirmation dialogs, per-operation feedback, and guards against stale search and editor requests
- [x] Manual GitHub Releases version check (HTTP status + semver comparison)

### CLI & Open-Source Engineering
- [x] `unoskills-cli` with `--json` support (`list`, `show`, `install`, `remove`, `deploy`, `undeploy`, `adopt`, `agents`, `stack`)
- [x] GitHub Actions CI/CD (3-OS matrix, `rustfmt`, `clippy -D warnings`, workspace tests, release workflow)
- [x] Issue/PR templates, `AGENTS.md`, `CONTRIBUTING.md`, `docs/ARCHITECTURE.md`, `docs/STACKS_SPEC.md`, bundled `skills/unoskills/SKILL.md`

---

## Release Milestones

### `v0.2` — Daily Workflow & Sync Polish
- [x] **Skill Upstream Update Check & Upgrade**: check Git / `skills.sh` / local skills for content changes via SHA-256 hash, update in one click, and automatically refresh `Copy`-mode deployments
- [x] **Project-Scope Skills**: attach project workspaces to manage repo-local `.claude/skills`, `.cursor/skills`, `.codex/skills`, etc. alongside global skills (CLI + GUI Scope Bar)
- [x] **Custom Agent Paths & Network Settings (in Settings)**: add/remove custom AI agent skill directories, plus optional HTTP/SOCKS Proxy and local GitHub PAT (avoids 60 req/hr API rate limits & improves GitHub reachability)
- [x] **Interactive Conflict Resolution UI**: surface `ReplacePolicy::UserConfirmed` via an `AlertDialog` when deploying onto an unmanaged target folder (global & project scope)
- [x] **Native Desktop Conveniences**:
  - OS folder picker for local skill installation, custom agent paths, and project workspaces
  - "Reveal in File Manager" action on skills, project workspaces, and library root
  - Keyboard shortcuts (`Cmd/Ctrl+K` focus search, `Cmd/Ctrl+S` save `SKILL.md`, `Esc` exit edit mode)
  - Confirmation dialog before deleting a local Stack or detaching a Project Workspace

### `v0.3.0` — Release Engineering (delivered 2026-11-20)
- [x] **Branch + PR + Ruleset workflow**: three-OS CI as required checks, linear-history squash merges, Conventional Commits
- [x] **User-Facing Installers**: macOS `.app` → `.dmg`, Windows Inno Setup installer, Linux `.deb` with auto-resolved `Depends`, plus stable platform identity (`dev.unoskills.UnoSkills`)
- [x] **Curated Release Notes**: `git-cliff` with a hand-written CHANGELOG override per version
- [x] **README revamp**: brand header, badges, real app screenshots, bilingual (EN + zh-CN)

### `v0.4.0` — Community Stacks & Lightweight Sync
- [x] **Stage 1 Remote Community Index**: fetch live curated stacks from a GitHub-hosted static `index.json` via `StackRegistry` with local ETag caching
- [x] **Optional GitHub Gist Share**: publish a local Stack as a secret Gist via PAT (Device Flow supported with a configured OAuth client id), never requiring login for any core local feature
- [x] **Stack Search & Tag Filtering**: filter featured and community stacks by tags, author, or target agent
- [x] **Private Git Backup / Multi-Machine Sync (in Settings)**: bind a personal private Git repo to push/pull plain-text `skills/` + `stacks/*.toml` (never syncing binary SQLite; index rebuilds locally on pull)
- [x] **In-App Guided Updater v1**: startup rate-limited check, explicit download & install consent, SHA256SUMS verification, per-platform install paths (Inno upgrade / DMG guidance / browser for deb)

### `v0.6` — Project Skills
- [x] Project and subproject technology detection, skill recommendations, and scoped deployment
- [x] Guided updates with installer verification and manual recheck

### `v0.7.0` — Native Shell Desktop
- [x] Port the desktop windows and workflows to NSH 0.6.1 while opening existing skill libraries and settings
- [x] Theme presets and shared text scaling across interface and Markdown screens
- [x] NSH type checks and UI tests alongside the Rust workspace and three-platform CI

## Planned
- [ ] **Crates.io Publishing**: publish `unoskills-core` and `unoskills-cli` (`cargo install unoskills-cli`)
- [ ] **Signed Updates**: sign platform packages and publish signed release metadata
- [ ] **Real-Hardware Cross-Platform Verification**: end-to-end validation on macOS (Apple Silicon / Intel) and Windows (NTFS Junctions)
- [ ] **Stage 2 Community Hub**: lightweight API backend (Cloudflare Workers / Supabase) behind the same `StackRegistry` trait

---

## Explicit Non-Goals (Anti-Bloat Commitment)

- **No forced account or cloud lock-in**: local skill management and TOML Stacks always work offline.
- **No workspace rule-file manager (`CLAUDE.md` / `AGENTS.md` / `.cursorrules`)**: project rules are tightly bound to a single repo and belong in your IDE/Git workflow, whereas UnoSkills focuses exclusively on reusable Skills & shareable Skill Stacks.
- **No full code IDE**: `SKILL.md` editing stays lightweight and fast; complex multi-file authoring belongs in your editor.
- **No Electron / WebView runtime**: keep startup instant, memory footprint low, and rendering native via NSH.
