# UnoSkills Stacks & Community Registry Specification

## 1. Philosophy

Individual AI skills are building blocks; **Skill Stacks** are proven workflows.
A Stack does **not** duplicate or re-host skill source files. It is a lightweight declarative TOML recipe referencing canonical skill sources, pairing notes, and recommended agents.

## 2. Manifest Schema (`*.unoskills-stack.toml`)

```toml
[stack]
id = "rust-gpui-workbench"
name = "Rust + GPUI Native Stack"
author = "your-github-handle"
description = "Curated skills for building native Rust desktop apps with GPUI Kit."
tags = ["rust", "desktop", "gpui"]
recommended_agents = ["claude_code", "codex", "cursor"]
verified = false

[[stack.skills]]
name = "skill-creator"
source = "anthropics/skills@skill-creator"
note = "Author and lint SKILL.md files"
```

## 3. Identity & Community Governance (Two-Stage Evolution)

- **Local-First (No Login Wall)**: Installing skills, browsing stacks, exporting/importing TOML, and syncing local agents never require an account.
- **GitHub-Native Identity**: Community publishing uses GitHub OAuth Device Flow (`gh`-style 8-digit code; token stored in OS Keychain). No custom password database.
- **Stage 1 — Zero-Server GitHub Registry (`unoskills/stacks`)**:
  - Directory-per-author ownership: `stacks/<github-username>/<stack-id>.toml`.
  - GitHub Actions validate TOML schema, verify skill sources exist, estimate token budgets, and enforce `PR author == directory owner`.
  - Merged PRs compile a static `index.json` served via GitHub Pages / CDN.
  - Instant peer-to-peer sharing works anytime via GitHub Gist or copied `.unoskills-stack.toml`.
- **Stage 2 — Lightweight Cloud Hub (Optional Future Scale)**:
  - Clients talk to `unoskills_core::stacks::StackRegistry`.
  - Swap in a Cloudflare Workers + D1 or Supabase backend for instant publishing, stars, install counts, and author profiles (`unoskills.dev/@handle`) without changing UI code.
