# Changelog

All notable changes to **UnoSkills** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### ✨ Features / 新特性

- **Community Stacks Remote Index (Stage 1) / 社区组合远程索引**：fetches curated stacks from the GitHub-hosted `community/index.json` via the `StackRegistry` trait with **ETag caching** (`304` never re-downloads) and an **offline cached fallback**; the default URL is overridable via the `stacks_registry_url` setting.
  - New `community/` tree in the repo: seed `index.json` (5 curated stacks) + a contribution guide — submitting a community stack is now just a PR.
  - Stacks UI merges featured + community stacks with a **Community badge**; CLI `stack list` gains a Community section and `stack install` resolves community ids.
- **Stack Search & Tag Filtering / 组合搜索与标签过滤**: live search (name / author / description / tag) plus clickable tag pills above the Stacks list.
- **GitHub Gist Sharing / 分享到 Gist**: publish any stack as a **secret Gist** from the Stacks detail view (uses the stored GitHub PAT; CLI: `unoskills stack share <id>`, with `--device` for the OAuth Device Flow when a client id is configured). Core local features never require login.
- New `StackSource` provenance (`Featured` / `Community` / `Local`) on every stack manifest, with serde defaults keeping all legacy TOML files parsing unchanged.

### 📚 Documentation / 文档

- **README revamp / README 全面改版**: centered brand header, CI/release/license badges, real app screenshots for all four tabs (Library / Stacks / Discover-Adopt / Settings, captured from the running app), benefit-oriented Features, user-facing Install & Quick Start, Supported Tools (22 agents), Tech Stack table, and a Troubleshooting section.
- **Simplified Chinese README / 简体中文说明** (`README.zh-CN.md`): full-featured twin of the English page, cross-linked from both headers.
- Roadmap realigned with the shipped `v0.3.0` release-engineering milestone.

---

## [v0.3.0] - 2026-11-20

### ✨ Features / 新特性

- **User-Facing Installers for All Platforms / 三端正式安装包**（following the GPUI Kit packaging guide）
  - macOS: proper `UnoSkills.app` bundle (`Info.plist`, `.icns` icon, ad-hoc signature) shipped as a `.dmg` with drag-to-Applications layout.
  - Windows: Inno Setup installer `unoskills-setup-<version>-windows-x64.exe` with Start-menu entries, optional desktop shortcut, and uninstall support.
  - Linux: `.deb` package (`unoskills_<version>_amd64.deb`) installing both `unoskills-app` and the `unoskills` CLI with a `.desktop` entry and hicolor icon; runtime `Depends` auto-resolved from the binaries' ELF `NEEDED` entries.
- **Stable Platform Identity / 稳定平台身份**: the app now calls `set_app_identity("dev.unoskills.UnoSkills", …)` so OS notifications and taskbar grouping use the same identifier as the installers (bundle ID / Inno `AppId` / `StartupWMClass` all aligned).
- New `packaging/` asset tree: committed `unoskills.icns`, `unoskills.ico`, PNG icons, and per-platform templates (`Info.plist`, `unoskills.iss`, `.desktop`, deb `control`).

---

## [v0.2.0] - 2026-11-20

### ✨ Highlights / 核心更新

- **Skill Upstream Update Check & One-Click Upgrade / 技能一键检查更新与升级**
  - Re-fetch Git, `skills.sh`, and local-installed skills against SHA-256 directory hashes and update in place without breaking symlinks.
  - Automatically refreshes all `Copy`-mode deployments (both global and project-scope).
  - Available in both the Library inspector (**Update / 检查更新**) and CLI (`unoskills update <skill>`).
- **Project-Scope Skills / 项目级技能作用域**
  - Attach local project workspaces directly in the Library Deploy bar (**Global / Project switcher**) without adding extra top-level tabs.
  - Deploy central library skills into repo-local `.claude/skills`, `.codex/skills`, `.cursor/skills`, `.opencode/skills`, etc. with shared-directory reference counting and clean detach cleanup.
  - Full CLI parity via `unoskills project { list | add | remove | deploy | undeploy }`.
- **Interactive Conflict Resolution (`ReplacePolicy::UserConfirmed`) / 交互式冲突覆盖确认**
  - Deploying onto an unmanaged target folder safely pauses with a confirmation `AlertDialog` instead of silently clobbering user files, for both global and project-scope deployments.
- **Network Proxy, GitHub Token & Custom Agent Registry / 网络代理、GitHub Token 与自定义 Agent**
  - Configure optional HTTP/SOCKS proxy (applied to Git clones, `skills.sh` API, and GitHub update checks) and local-only GitHub PAT in **Settings**.
  - Register and remove custom AI Agent skill directories directly from **Settings**.
- **Native Desktop Conveniences & Keyboard Shortcuts / 原生桌面体验与快捷键**
  - Native OS folder picker for local skill installs, custom agents, and project workspaces.
  - **Reveal in File Manager** action on skills, attached projects, and the central library root.
  - Keyboard shortcuts: `Cmd/Ctrl+K` (focus Library search), `Cmd/Ctrl+S` (save `SKILL.md` in edit mode), `Esc` (cancel edit mode).
  - Confirmation `AlertDialog` before deleting a local Stack or detaching a Project Workspace.
- **Full 3-Language i18n Parity / 中英日三语完整适配**
  - Simplified Chinese (`zh-CN`), English (`en`), and Japanese (`ja`) across all new dialogs, settings, project scope controls, and toasts.

### 🐛 Fixes & Cross-Platform Hardening / 修复与跨平台加固

- Fixed Windows Clippy lint (`collapsible_if`) and switched integration tests to cross-platform deployment helpers so Ubuntu, macOS, and Windows CI all pass with zero warnings.
- Upgraded `skills.sh` leaderboard to native paginated JSON API with infinite scroll in **Discover**.

---

## [v0.1.0] - 2026-11-19

### ✨ Initial Foundation / 初始版本

- Local-first central skill library (`~/.unoskills`) backed by SQLite WAL with strict `SKILL.md` parsing, CJK-aware token estimation, lint checks, and safe Symlink / Copy / Windows Junction sync engine.
- Declarative 22-agent registry (`crates/unoskills-core/data/agents.toml`) with automatic installation detection and unmanaged skill adoption.
- Declarative Skill Stacks (`.unoskills-stack.toml`): featured stacks, local creation, TOML import/export, and incremental one-click install.
- Lean 4-tab native Rust GPUI desktop workbench (**Library / Stacks / Discover / Settings**) with Dark/Light themes, language-aware & custom font pickers, and `unoskills-cli` (`--json`).
