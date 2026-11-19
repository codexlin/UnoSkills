<p align="center">
  <img src="assets/icon.png" width="80" alt="UnoSkills" />
</p>

<h1 align="center">UnoSkills</h1>

<p align="center">
  A <strong>fast, native Rust workbench</strong> to manage, share, and sync<br/>AI agent skills & <strong>Skill Stacks</strong> across every coding tool.
</p>

<p align="center">
  Pure Rust · Zero WebView · Zero Node.js · Instant startup
</p>

<p align="center">
  <a href="https://github.com/codexlin/UnoSkills/actions/workflows/ci.yml"><img src="https://github.com/codexlin/UnoSkills/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/codexlin/UnoSkills/releases/latest"><img src="https://img.shields.io/github/v/release/codexlin/UnoSkills?sort=semver&display_name=tag" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/codexlin/UnoSkills" alt="MIT License" /></a>
  <img src="https://img.shields.io/badge/platform-macOS%20·%20Windows%20·%20Linux-blue" alt="Platforms" />
  <img src="https://img.shields.io/badge/Rust-1.85%2B-orange" alt="Rust 1.85+" />
</p>

<p align="center">
  <a href="#features">Features</a> &nbsp;·&nbsp;
  <a href="#install">Install</a> &nbsp;·&nbsp;
  <a href="#quick-start">Quick Start</a> &nbsp;·&nbsp;
  <a href="#supported-tools">Supported Tools</a> &nbsp;·&nbsp;
  <a href="#development">Development</a>
</p>

---

<p align="center">
  <img src="assets/demo/library.png" width="800" alt="UnoSkills Library: inspect a skill, check token budget & lint, toggle per-agent deployment" />
</p>
<p align="center"><em><strong>Library</strong> — inspect skills, check lint & context-token budget, toggle per-agent deployment in one click.</em></p>

## Why UnoSkills?

Most skill managers are Electron/Tauri apps with a WebView and a Node runtime. UnoSkills is built on [GPUI Kit](https://gpui-kit.com/) (the GPU renderer behind [Zed](https://zed.dev)), so the whole app — lists, Markdown preview, dialogs — is rendered natively on the GPU. It starts instantly, sips memory, and virtualizes long lists so large libraries keep scrolling smoothly.

But fast is only half of it. A skill manager touches your agents' config directories, so **safety is a product feature**, not a checkbox:

- **Strict skill detection** — recognizes canonical `SKILL.md` (plus legacy `skill.md`); never misclassifies `README.md` or `CLAUDE.md` as a skill.
- **No silent clobbering** — targets are classified with `symlink_metadata` (never follows links), and deployments default to `NoClobber`: an unmanaged folder stops with an explicit confirmation dialog instead of being overwritten.
- **Shared-directory reference counting** — when several agents resolve to the same physical directory (e.g. `~/.agents/skills`), undeploying one agent never deletes another agent's live skills.
- **Path hardening** — every slug passes traversal/reserved-name sanitization; every copy/link is checked against source-overlap recursion.
- **Destructive actions confirm** — CLI `remove` requires `--yes`; GUI shows an explicit `AlertDialog`.

## Features

<p align="center">
  <img src="assets/demo/stacks.png" width="800" alt="UnoSkills Stacks: one-click install of curated skill combinations" />
</p>
<p align="center"><em><strong>Stacks</strong> — install a curated skill combination in one click, or bundle your own.</em></p>

### 📚 Library — your central skill repo

- Install skills from Git repos (`owner/repo@skill`), GitHub `/tree/branch/subpath` URLs, local folders, or the [skills.sh](https://skills.sh) marketplace — everything lands in one local-first library at `~/.unoskills`.
- Preview and quick-edit `SKILL.md` in place; frontmatter lint + estimated context tokens per skill.
- **One-click update check**: compares Git / `skills.sh` / local sources via SHA-256 and upgrades in place without breaking symlinks or managed deployments.
- **Per-agent deploy switches** right on the card — Symlink (default, live-updating) or Copy mode, for 22 built-in agents and any custom tool you register.
- **Project scopes**: attach a repo workspace and deploy skills into repo-local `.claude/skills`, `.codex/skills`, `.cursor/skills`… with the same safety guarantees.

### 🧩 Stacks — skills, composed

- A Stack is a declarative `.unoskills-stack.toml` manifest — a named recipe of skills *without duplicating their source code*.
- Featured stacks ship in-app (React & Next.js, Anthropic essentials, Superpowers engineering discipline…); create your own from installed skills, export as TOML, and share via Gist or PR.
- **Incremental one-click install**: existing skills are skipped, missing ones are fetched, nothing is installed twice.

### 🔍 Discover — find & adopt

<p align="center">
  <img src="assets/demo/discover.png" width="800" alt="UnoSkills Discover: adopt existing unmanaged skills" />
</p>
<p align="center"><em><strong>Adopt</strong> — skills you already installed by hand are detected and can be absorbed into the library in one click.</em></p>

- Quick Install from `owner/repo@skill`, GitHub links, or a local directory.
- Browse the [skills.sh](https://skills.sh) leaderboard (trending / rising / all-time) with search and infinite scroll.
- **Adopt existing skills**: scan your agents' directories for unmanaged skills and bring them under management — no re-install, no duplicates.

### ⚙️ Settings

- Full **Simplified Chinese / English / Japanese** UI, Dark/Light themes, system font discovery (CJK-aware).
- Default deploy mode (Symlink/Copy), proxy support (HTTP/SOCKS for Git + marketplace), local-only GitHub PAT, and a **declarative agent registry** — 22 agents defined in one `agents.toml`, with automatic installation detection.

### 🖥️ An agent-friendly CLI ships alongside

The `unoskills` CLI shares the exact same engine and SQLite store as the desktop app — and every command speaks `--json`, so Claude Code / Codex / your scripts can manage skills themselves:

```bash
unoskills install anthropics/skills@skill-creator
unoskills deploy skill-creator --agent claude_code --agent codex
unoskills stack install rust-gpui-workbench --agent cursor
unoskills update skill-creator
unoskills adopt --dry-run          # see what's unmanaged
unoskills project deploy my-skill --project my-app --agent opencode
unoskills list --json              # machine-readable output
```

## Install

Grab the latest from [**Releases**](https://github.com/codexlin/UnoSkills/releases/latest) — every release ships a `SHA256SUMS.txt`:

| Platform | Recommended | Notes |
|---|---|---|
| macOS (Apple Silicon) | `unoskills-macos-arm64.dmg` | drag `UnoSkills.app` into `/Applications` |
| Windows x64 | `unoskills-setup-<version>-windows-x64.exe` | Inno Setup installer: Start-menu + uninstall support |
| Linux (Debian/Ubuntu) | `unoskills_<version>_amd64.deb` | `sudo apt install ./unoskills_<version>_amd64.deb` |

Bare binaries (`unoskills-app-*` GUI, `unoskills-*` CLI) and raw archives are also attached for power users.

> **Heads-up (macOS)**: builds are not yet Developer-ID-signed/notarized, so the first launch needs a right-click → **Open**. Proper signing is on the [roadmap](docs/ROADMAP.md).

## Quick Start

1. **Install a skill** — open **Discover**, paste `owner/repo@skill` or a GitHub link, pick a local folder, or grab one from the marketplace.
2. **Deploy it** — in **Library**, flip the switch next to each agent that should receive it. Symlink mode keeps deployed copies live with the central library.
3. **Bundle a workflow** — in **Stacks**, select skills → **Create**, then export the TOML to share your setup with the team.
4. **Adopt what you have** — already hand-installed skills? **Discover → Adopt Existing** absorbs them without reinstalling.
5. Optional: attach a **project workspace** in the Library toolbar and keep per-repo skills isolated.

## Sharing a Skill Stack

Every stack is a lightweight declarative TOML manifest — no skill source code is duplicated:

```toml
[stack]
id = "rust-gpui-workbench"
name = "Rust + GPUI Native Stack"
author = "your-github-handle"
description = "Curated skills for building native Rust desktop apps."
tags = ["rust", "gpui", "desktop"]
recommended_agents = ["claude_code", "codex", "cursor"]

[[stack.skills]]
name = "skill-creator"
source = "anthropics/skills@skill-creator"
note = "Author and lint SKILL.md files"
```

Install it from a URL, a local file, or straight from the app: `unoskills stack install rust-gpui-workbench --agent claude_code`.

## Supported Tools

22 agents are supported out of the box, detected automatically:

Claude Code · Codex · Cursor · Antigravity · OpenCode · Gemini CLI · GitHub Copilot · Windsurf · Cline · Roo Code · Kilo Code · Amp · Goose · TRAE IDE · Warp · Kimi Code CLI · Grok · Droid · Augment · Replit · OpenClaw · OMP Agent

**Settings** also lets you register custom tools with their own skills directories, or override paths for built-in ones.

## Tech Stack

| Layer | Tech |
|---|---|
| UI | [GPUI Kit](https://gpui-kit.com/) (GPUI — Zed's GPU renderer) |
| Domain core | Pure Rust, SQLite WAL, symlink/copy/junction sync engine |
| CLI | `clap`, `--json` machine output |
| i18n | Simplified Chinese / English / Japanese |
| Packaging | `.dmg` (app bundle) · Inno Setup `.exe` · `.deb` |

## Development

**Prerequisites**: Rust 1.85+ (edition 2024). On Linux, install the GPUI system deps (see `ci.yml` for the exact package list).

```bash
cargo test -p unoskills-core          # core unit & integration tests
cargo run -p unoskills-app             # native GPUI desktop app
cargo run -p unoskills-cli -- agents   # CLI
```

Workspace layout:

```text
unoskills/
├── crates/
│   ├── unoskills-core/   # pure Rust domain: SQLite WAL, safe sync, stacks, data/*.toml
│   ├── unoskills-cli/    # thin `unoskills` CLI (--json)
│   └── unoskills-app/    # GPUI Kit app (shell, features/*, i18n, assets)
├── docs/                 # architecture, stack spec, roadmap
├── packaging/            # per-platform installer templates & icons
└── skills/unoskills/     # bundled dogfooding SKILL.md
```

## Documentation

- **[Architecture & Best Practices](docs/ARCHITECTURE.md)** — workspace boundaries, GPUI entity/event flow, performance & sync-safety patterns
- **[Skill Stacks Spec](docs/STACKS_SPEC.md)** — declarative `.unoskills-stack.toml` format and community evolution
- **[Roadmap](docs/ROADMAP.md)** — delivered capabilities and upcoming milestones
- **[AGENTS.md](AGENTS.md)** — mandatory engineering & GPUI Kit rules for AI agents and reviewers
- **[CONTRIBUTING.md](CONTRIBUTING.md)** — how to add new agents, languages, or featured Stacks

## Troubleshooting

**macOS blocks the app on first launch.** Current builds are unsigned, so Gatekeeper shows "unidentified developer". Right-click `UnoSkills.app` → **Open** → confirm. (Developer ID signing + notarization is planned.)

**Linux: window fails to open / blank.** GPUI renders through Vulkan (wgpu). On minimal VMs install a Vulkan-capable driver (e.g. `mesa-vulkan-drivers`); on pure Wayland compositors without XWayland the X11 backend needs `libxkbcommon-x11`.

Anything else — [open an issue](https://github.com/codexlin/UnoSkills/issues) with the CLI output of `unoskills agents --json`.

## License

MIT
