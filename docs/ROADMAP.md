# UnoSkills Roadmap

UnoSkills is designed to remain a **fast, lean, local-first, open-source** native Rust workbench for managing AI Agent Skills & Skill Stacks.

---

## Delivered (`v0.1` — Foundation & Daily-Usable Workbench)

### Core Engine (`unoskills-core`)
- [x] Central skill library (`~/.unoskills`) backed by SQLite WAL (`busy_timeout = 5000ms`)
- [x] Strict `SKILL.md` / `skill.md` parser, YAML frontmatter stripping, CJK-aware token estimation, lint checks, SHA-256 directory hashing
- [x] Safe sync engine: Symlink / Copy / Windows Junction, `ReplacePolicy::NoClobber`, overlap guard, shared-directory reference counting
- [x] Declarative 22-agent registry (`crates/unoskills-core/data/agents.toml`) with automatic installation detection
- [x] Multi-source installer: local folder, GitHub URL, `owner/repo@skill`, `skills.sh` leaderboard (Trending / Hot / All Time) & search
- [x] Unmanaged local skill scanner & adopter (single + bulk)
- [x] Declarative Skill Stacks (`.unoskills-stack.toml`): featured stacks, local creation, TOML import/export, incremental one-click install

### Native Desktop App (`unoskills-app`, GPUI Kit)
- [x] Lean 4-tab desktop shell (`Library` / `Stacks` / `Discover` / `Settings`) with collapsible sidebar
- [x] Modular entity + event architecture (`WorkspaceState`, `WorkspaceEvent`, feature folders, `RenderOnce` components)
- [x] Custom Uno brand SVG icon via composite `AssetSource` (`UnoAssets` + `AppIcon`)
- [x] Full i18n parity for Simplified Chinese (`zh-CN`), English (`en`), and Japanese (`ja`) with `Select` dropdown and `gpui-kit` locale sync
- [x] Language-aware automatic UI fonts, searchable system UI & monospace font pickers, global font-size scaling
- [x] Persisted Dark / Light appearance without resetting custom fonts
- [x] Virtualized lists (`uniform_list`), cached `TextViewState` Markdown preview, quick raw-frontmatter edit with unsaved-edit protection
- [x] `AlertDialog` delete confirmation, concurrent async busy-task stack, out-of-order search guards
- [x] Manual GitHub Releases version check (HTTP status + semver comparison)

### CLI & Open-Source Engineering
- [x] `unoskills-cli` with `--json` support (`list`, `show`, `install`, `remove`, `deploy`, `undeploy`, `adopt`, `agents`, `stack`)
- [x] GitHub Actions CI/CD (3-OS matrix, `rustfmt`, `clippy -D warnings`, workspace tests, release workflow)
- [x] Issue/PR templates, `AGENTS.md`, `CONTRIBUTING.md`, `docs/ARCHITECTURE.md`, `docs/STACKS_SPEC.md`, bundled `skills/unoskills/SKILL.md`

---

## Upcoming Milestones

### `v0.2` — Daily Workflow & Sync Polish
- [ ] **Skill Upstream Update Check & Upgrade**: check Git-installed skills for newer commits, preview changes, and update safely in one click
- [ ] **Project-Scope Skills**: attach project workspaces to manage repo-local `.claude/skills`, `.cursor/skills`, `.codex/skills`, etc. alongside global skills
- [ ] **Custom Agent Paths & Network Settings (in Settings)**: allow adding custom/unlisted AI agent skill directories, plus optional HTTP Proxy and local GitHub PAT (avoids 60 req/hr API rate limits & improves GitHub reachability)
- [ ] **Interactive Conflict Resolution UI**: surface `ReplacePolicy::UserConfirmed` via an `AlertDialog` when deploying onto an unmanaged target folder
- [ ] **Native Desktop Conveniences**:
  - OS folder picker for local skill installation
  - "Reveal in File Manager" action on skills and library root
  - Keyboard shortcuts (`Cmd/Ctrl+K` focus search, `Cmd/Ctrl+S` save `SKILL.md`, `Esc` exit edit mode)
  - Confirmation dialog before deleting a local Stack

### `v0.3` — Community Stacks & Lightweight Git Sync
- [ ] **Stage 1 Remote Community Index**: fetch live curated stacks from a GitHub-hosted static `index.json` via `StackRegistry` with local ETag caching
- [ ] **Optional GitHub Device Flow Share**: publish a local Stack as a GitHub Gist or PR without requiring login for any core local feature
- [ ] **Stack Search & Tag Filtering**: filter featured and community stacks by tags, author, or target agent
- [ ] **Private Git Backup / Multi-Machine Sync (in Settings)**: bind a personal private Git repo to push/pull plain-text `skills/` + `stacks/*.toml` (never syncing binary SQLite; index rebuilds locally on pull)

### `v0.4` — Cross-Platform Packaging & Auto-Update
- [ ] **In-App Signed Self-Updater**: integrate `gpui-updater-pre` with Ed25519-signed release manifests (always opt-in, never silent)
- [ ] **Native Installers**: macOS signed/notarized `.dmg`, Linux `.AppImage` / `.deb`, Windows `.msi`
- [ ] **Real-Hardware Cross-Platform Verification**: end-to-end validation on macOS (Apple Silicon / Intel) and Windows (NTFS Junctions)
- [ ] **Crates.io Publishing**: publish `unoskills-core` and `unoskills-cli` (`cargo install unoskills-cli`)

---

## Explicit Non-Goals (Anti-Bloat Commitment)

- **No forced account or cloud lock-in**: local skill management and TOML Stacks always work offline.
- **No workspace rule-file manager (`CLAUDE.md` / `AGENTS.md` / `.cursorrules`)**: project rules are tightly bound to a single repo and belong in your IDE/Git workflow, whereas UnoSkills focuses exclusively on reusable Skills & shareable Skill Stacks.
- **No full code IDE**: `SKILL.md` editing stays lightweight and fast; complex multi-file authoring belongs in your editor.
- **No Electron / WebView runtime**: keep startup instant, memory footprint low, and rendering 100% native via GPUI.
