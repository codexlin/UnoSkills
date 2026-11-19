# UnoSkills

> A fast, native Rust workbench to manage, share, and sync AI agent skills & **Skill Stacks** across every coding tool — built with [GPUI Kit](https://gpui-kit.com/docs/).

## Why UnoSkills?

- **Pure Rust + Native GPU Rendering**: Built on Zed's GPUI via `gpui-kit` — zero WebView, zero Node.js runtime, instant startup.
- **Focused 4-Tab Workbench (No Bloat)**:
  1. **Library** — Search skills, preview & quick-edit `SKILL.md`, check frontmatter lint & estimated context tokens, and toggle per-agent deployment in one click.
  2. **Stacks** — Discover curated skill combinations, bundle your own favorite skills into a declarative `.unoskills-stack.toml` recipe, share via GitHub Gist or community PR, and install an entire workflow in one click.
  3. **Discover** — Install from `owner/repo@skill`, GitHub `/tree/branch/subpath` URLs, local folders, `skills.sh` marketplace, or one-click **Adopt** existing unmanaged skills from installed agents.
  4. **Settings** — Declarative AI agent registry (`crates/unoskills-core/data/agents.toml`), Symlink / Copy mode, language & system fonts, and persisted Dark / Light themes.
- **Safety-First Sync Engine**:
  - Recognizes canonical `SKILL.md` (and legacy `skill.md`); never misclassifies `README.md` or `CLAUDE.md`.
  - `NoClobber` & `ReplacePolicy` protect unmanaged user files from accidental overwrite.
  - Shared-directory reference counting prevents undeploying one agent from deleting another agent's live skill when they share a directory (e.g. `~/.agents/skills` or `~/.config/agents/skills`).
- **Agent-Friendly CLI Included**: Shares the exact same `unoskills-core` engine and SQLite WAL store, with `--json` output for scripts and AI agents.

## Workspace Layout

```text
unoskills/
├── .github/                   # CI/CD workflows & Issue/PR templates
├── docs/                      # Architecture & Stack manifest spec
├── packaging/                 # Desktop bundle icons & installer metadata
├── skills/unoskills/          # Bundled dogfooding SKILL.md
└── crates/
    ├── unoskills-core/        # Pure Rust domain, SQLite WAL, safe sync, stacks + data/*.toml
    ├── unoskills-cli/         # Thin `unoskills` CLI (--json)
    └── unoskills-app/         # Native GPUI Kit app (shell, state, i18n/, features/*/, assets/)
```

## Documentation

- **[Architecture & Best Practices](docs/ARCHITECTURE.md)** — workspace boundaries, GPUI Kit entity/event flow, performance & sync safety patterns, and contributor playbooks
- **[Skill Stacks Spec](docs/STACKS_SPEC.md)** — declarative `.unoskills-stack.toml` format and two-stage community evolution
- **[Roadmap](docs/ROADMAP.md)** — delivered `v0.1` capabilities and upcoming milestones
- **[AGENTS.md](AGENTS.md)** — mandatory engineering & GPUI Kit rules for AI agents and reviewers
- **[CONTRIBUTING.md](CONTRIBUTING.md)** — how to add new AI agents, languages, or featured Stacks

## Quick Start

```bash
# Run core tests
cargo test -p unoskills-core

# Run the native GPUI desktop app
cargo run -p unoskills-app

# Run the CLI
cargo run -p unoskills-cli -- agents
cargo run -p unoskills-cli -- stack list
cargo run -p unoskills-cli -- adopt --dry-run
```

## Sharing a Skill Stack

Every stack is a lightweight declarative TOML manifest (no skill source code duplication):

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

## License

MIT
