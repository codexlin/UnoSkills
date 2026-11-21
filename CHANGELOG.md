# Changelog

All notable changes to **UnoSkills** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

## [v0.6.2] - 2026-11-21

### Fixed / 修复

- **Project skill availability / 项目技能可用状态**. The Library counts globally available skills in project view and shows where each Agent gets the skill. Removing a project deployment keeps the global installation available. 技能库在项目视图中统计全局可用技能，并标明每个 Agent 的技能来源。移除项目部署后，已全局安装的技能仍然可用。
- **Project stack deployment / 组合部署到项目**. The preview distinguishes globally available skills from missing skills. Deploy only the missing skills by default, or include global skills to place them in the project directory. 部署预览区分全局可用和待部署技能。默认只补齐缺少项，也可勾选将全局可用项写入项目目录。

## [v0.6.1] - 2026-11-21

### Changed / 改进

- **In-app update flow / 应用内更新流程**. Open the update dialog from the title bar. Confirm once to download, watch progress in the dialog or title bar, and open the installer when verification finishes. 从标题栏直接打开更新窗口。确认后可在窗口或标题栏查看进度，校验完成后直接打开安装包。

### Fixed / 修复

- **Release downloads / 发布附件下载**. If a direct GitHub download fails, retry through the release asset API and verify the installer against the published SHA-256 checksum. GitHub 直连下载失败时，通过附件 API 重试，并使用发布时提供的 SHA-256 校验安装包。

## [v0.6.0] - 2026-11-21

### Added / 新增

- **Project skill recommendations / 项目技能推荐**. Scan local project and subproject configuration to identify technologies, see which files were detected, and save selected recommendations as a reusable stack. 从本地配置识别项目及子项目的技术栈，查看识别依据，并将选中的推荐技能保存为可复用的组合。
- **Subproject deployment / 子项目部署**. Choose the project root or a detected subproject in the deployment preview. Each directory tracks its own deployments and preserves existing files on conflict. 在部署预览中选择项目根目录或识别到的子项目。各目录独立记录部署状态，遇到冲突时保留原有文件。

### Changed / 改进

- **Install and preview / 安装并预览**. Install missing recommended skills from the stack and continue to the deployment preview. Files are written to the selected project directory after confirmation. 补齐组合中尚未入库的技能后，直接进入部署预览。确认后再写入所选项目目录。

## [v0.5.0] - 2026-11-21

### Added / 新增

- **Home workspace / 首页工作区** — Open the skill library, local skills, Agent settings, and My Stacks from Home. See recent additions, the seven-day intake chart, and the skills.sh leaderboard.
- **Project stack deployment / 组合部署到项目** — Select a project and an Agent, check what is already deployed, and add the missing skills from your library in one operation.
- **Follow system appearance / 跟随系统外观** — Let UnoSkills follow the operating system theme and switch to light or dark manually in Settings.

### Changed / 改进

- **Project deployment checks / 项目部署核对** — Check the files on disk, restore missing copies or links in their original mode, and keep user files when a target is occupied or changed.
- **Marketplace and Stacks / 市场与技能组合** — Browse a three-column marketplace with direct install and source filtering. My Stacks opens first, and individual skills in a stack can be installed from the detail view.
- **Local skills / 本机技能** — Find existing Agent skills under a clearer label and review what will be managed before adding them to the library.

## [v0.4.0] - 2026-11-20

### ✨ Features / 新特性

- **Desktop workbench redesign / 桌面工作台改版**: refreshed sidebar and settings navigation, compact Library with per-Agent deployment switches, three-column Marketplace, and grouped adoption.
- **Community Stacks Remote Index (Stage 1) / 社区组合远程索引**：fetches curated stacks from the GitHub-hosted `community/index.json` via the `StackRegistry` trait with **ETag caching** (`304` never re-downloads) and an **offline cached fallback**; the default URL is overridable via the `stacks_registry_url` setting.
  - New `community/` tree in the repo: seed `index.json` (5 curated stacks) + a contribution guide — submitting a community stack is now just a PR.
  - Stacks UI merges featured + community stacks with a **Community badge**; CLI `stack list` gains a Community section and `stack install` resolves community ids.
- **Stack Search & Tag Filtering / 组合搜索与标签过滤**: live search (name / author / description / tag) plus clickable tag pills above the Stacks list.
- **GitHub Gist Sharing / 分享到 Gist**: publish any stack as a **secret Gist** from the Stacks detail view (uses the stored GitHub PAT; CLI: `unoskills stack share <id>`, with `--device` for the OAuth Device Flow when a client id is configured). Core local features never require login.
- New `StackSource` provenance (`Featured` / `Community` / `Local`) on every stack manifest, with serde defaults keeping all legacy TOML files parsing unchanged.
- **In-App Guided Updater v1 / 应用内引导式更新**: 24-hour rate-limited silent startup check, TitleBar update pill, explicit download & install confirmation `AlertDialog`, automatic `SHA256SUMS.txt` digest verification (tampered downloads deleted immediately), per-platform guided install paths (Windows Inno Setup launcher, macOS DMG mount, Linux `.deb` file reveal & Release page), and CLI `unoskills check-update`.
- **Private Git Backup & Multi-Machine Sync / 私有 Git 备份与多机同步**: bind a personal private Git repo in **Settings** (or via `unoskills backup { status | configure | push | pull }`) to push/pull plain-text `skills/` and `stacks/*.toml` via a dedicated mirror repo — never syncing binary SQLite or local secrets, automatically rebuilding the local SQLite index and refreshing `Copy`-mode deployments on pull.

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
