mod en;
mod ja;
mod zh_cn;

use unoskills_core::DeployMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    ZhCn,
    En,
    Ja,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::ZhCn, Lang::En, Lang::Ja];

    pub fn code(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::En => "en",
            Self::Ja => "ja",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::ZhCn => "简体中文",
            Self::En => "English",
            Self::Ja => "日本語",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::ZhCn => "中",
            Self::En => "EN",
            Self::Ja => "日",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::ZhCn => Self::En,
            Self::En => Self::Ja,
            Self::Ja => Self::ZhCn,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::ZhCn => 0,
            Self::En => 1,
            Self::Ja => 2,
        }
    }

    pub fn from_display_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.display_name() == name)
    }

    pub fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" | "en-gb" => Self::En,
            "ja" | "ja-jp" => Self::Ja,
            _ => Self::ZhCn,
        }
    }

    pub fn t(self) -> &'static Strings {
        match self {
            Self::ZhCn => &zh_cn::STRINGS,
            Self::En => &en::STRINGS,
            Self::Ja => &ja::STRINGS,
        }
    }

    // ── Formatted dynamic toast / dialog / status messages ──

    pub fn toast_saved_skill(self, name: &str, tokens: u32) -> String {
        match self {
            Self::ZhCn => format!("已保存 {name}（约 {tokens} tokens）"),
            Self::En => format!("Saved {name} (~{tokens} tokens)"),
            Self::Ja => format!("{name} を保存しました（約 {tokens} tokens）"),
        }
    }

    pub fn toast_undeployed(self, agent_key: &str) -> String {
        match self {
            Self::ZhCn => format!("已从 {agent_key} 取消部署"),
            Self::En => format!("Undeployed from {agent_key}"),
            Self::Ja => format!("{agent_key} からデプロイ解除しました"),
        }
    }

    pub fn toast_deployed(self, agent_key: &str, mode: DeployMode) -> String {
        match self {
            Self::ZhCn => format!("已部署到 {agent_key}（{mode:?}）"),
            Self::En => format!("Deployed to {agent_key} ({mode:?})"),
            Self::Ja => format!("{agent_key} にデプロイしました（{mode:?}）"),
        }
    }

    pub fn toast_deploy_mode(self, mode: DeployMode) -> String {
        match self {
            Self::ZhCn => format!("默认部署方式已切换为 {mode:?}"),
            Self::En => format!("Deploy mode set to {mode:?}."),
            Self::Ja => format!("既定のデプロイ方式を {mode:?} に変更しました"),
        }
    }

    pub fn busy_installing(self, source: &str) -> String {
        match self {
            Self::ZhCn => format!("正在安装 {source}..."),
            Self::En => format!("Installing {source}..."),
            Self::Ja => format!("{source} をインストール中..."),
        }
    }

    pub fn busy_adopting_all(self, count: usize) -> String {
        match self {
            Self::ZhCn => format!("正在批量收编 {count} 个技能..."),
            Self::En => format!("Adopting {count} skill(s)..."),
            Self::Ja => format!("{count} 件のスキルを取り込み中..."),
        }
    }

    pub fn toast_installed(self, name: &str, tokens: u32) -> String {
        match self {
            Self::ZhCn => format!("已安装「{name}」（约 {tokens} tokens）"),
            Self::En => format!("Installed '{name}' (~{tokens} tokens)."),
            Self::Ja => format!("「{name}」をインストールしました（約 {tokens} tokens）"),
        }
    }

    pub fn toast_adopted(self, name: &str, agent: &str) -> String {
        match self {
            Self::ZhCn => format!("已从 {agent} 收编「{name}」到技能库"),
            Self::En => format!("Adopted '{name}' from {agent} into Library."),
            Self::Ja => format!("{agent} から「{name}」をライブラリに取り込みました"),
        }
    }

    pub fn toast_adopted_all(self, ok: usize, failed: usize) -> String {
        match self {
            Self::ZhCn => format!("已批量收编 {ok} 个技能（跳过/失败 {failed} 个）"),
            Self::En => format!("Adopted {ok} skill(s) into Library ({failed} skipped/failed)."),
            Self::Ja => format!("{ok} 件のスキルを取り込みました（スキップ/失敗 {failed} 件）"),
        }
    }

    pub fn busy_installing_stack(self, name: &str) -> String {
        match self {
            Self::ZhCn => format!("正在安装组合「{name}」..."),
            Self::En => format!("Installing stack '{name}'..."),
            Self::Ja => format!("スタック「{name}」をインストール中..."),
        }
    }

    pub fn toast_stack_installed(
        self,
        name: &str,
        installed: usize,
        skipped: usize,
        failed: usize,
    ) -> String {
        match self {
            Self::ZhCn => {
                format!("组合「{name}」：新装 {installed}，已有跳过 {skipped}，失败 {failed}")
            }
            Self::En => {
                format!("Stack '{name}': installed {installed}, skipped {skipped}, failed {failed}")
            }
            Self::Ja => format!(
                "スタック「{name}」：新規 {installed}、既存スキップ {skipped}、失敗 {failed}"
            ),
        }
    }

    pub fn toast_stack_toml_copied(self, stack_id: &str) -> String {
        match self {
            Self::ZhCn => format!("已复制「{stack_id}.unoskills-stack.toml」到剪贴板！"),
            Self::En => format!("Copied '{stack_id}.unoskills-stack.toml' to clipboard!"),
            Self::Ja => {
                format!("「{stack_id}.unoskills-stack.toml」をクリップボードにコピーしました")
            }
        }
    }

    pub fn toast_stack_created(self, name: &str, skill_count: usize) -> String {
        match self {
            Self::ZhCn => format!("已创建组合「{name}」（包含 {skill_count} 个技能）"),
            Self::En => format!("Created Stack '{name}' ({skill_count} skills)!"),
            Self::Ja => format!("スタック「{name}」を作成しました（{skill_count} 件のスキル）"),
        }
    }

    pub fn toast_stack_imported(self, name: &str) -> String {
        match self {
            Self::ZhCn => format!("已导入组合「{name}」到我的组合"),
            Self::En => format!("Imported Stack '{name}' into My Stacks."),
            Self::Ja => format!("スタック「{name}」をマイスタックにインポートしました"),
        }
    }

    pub fn toast_gist_shared(self, url: &str) -> String {
        match self {
            Self::ZhCn => format!("已分享为私密 Gist（链接已复制）：{url}"),
            Self::En => format!("Shared as a secret Gist (URL copied): {url}"),
            Self::Ja => format!("非公開 Gist として共有し URL をコピーしました：{url}"),
        }
    }

    pub fn busy_sharing_stack(self, name: &str) -> String {
        match self {
            Self::ZhCn => format!("正在分享组合「{name}」..."),
            Self::En => format!("Sharing Stack '{name}'..."),
            Self::Ja => format!("スタック「{name}」を共有中..."),
        }
    }

    pub fn busy_searching(self, query: &str) -> String {
        match self {
            Self::ZhCn => format!("正在搜索「{query}」..."),
            Self::En => format!("Searching '{query}'..."),
            Self::Ja => format!("「{query}」を検索中..."),
        }
    }

    pub fn toast_update_available(self, latest: &str) -> String {
        match self {
            Self::ZhCn => format!("发现新版本 {latest}！可前往 GitHub Releases 下载更新。"),
            Self::En => format!("New version {latest} is available on GitHub Releases!"),
            Self::Ja => format!("新しいバージョン {latest} が利用可能です（GitHub Releases）。"),
        }
    }

    pub fn toast_up_to_date(self, current: &str) -> String {
        match self {
            Self::ZhCn => format!("当前已是最新版本（v{current}）"),
            Self::En => format!("You're up to date (v{current})."),
            Self::Ja => format!("最新バージョンです（v{current}）"),
        }
    }

    pub fn confirm_download_update_desc(
        self,
        tag: &str,
        asset_name: &str,
        has_checksums: bool,
    ) -> String {
        match self {
            Self::ZhCn => {
                if has_checksums {
                    format!(
                        "即将从 GitHub Releases 下载 {tag} 安装包（{asset_name}），并在下载完成后自动校验 SHA256SUMS.txt 完整性。校验通过后再由你确认是否安装。"
                    )
                } else {
                    format!(
                        "即将从 GitHub Releases 下载 {tag} 安装包（{asset_name}）。下载完成后再由你确认是否安装。"
                    )
                }
            }
            Self::En => {
                if has_checksums {
                    format!(
                        "Download {tag} ({asset_name}) from GitHub Releases and verify its SHA-256 digest against SHA256SUMS.txt. Nothing is installed without your confirmation."
                    )
                } else {
                    format!(
                        "Download {tag} ({asset_name}) from GitHub Releases. Nothing is installed without your confirmation."
                    )
                }
            }
            Self::Ja => {
                if has_checksums {
                    format!(
                        "GitHub Releases から {tag}（{asset_name}）をダウンロードし、SHA256SUMS.txt で整合性を検証します。確認なしに自動インストールされることはありません。"
                    )
                } else {
                    format!(
                        "GitHub Releases から {tag}（{asset_name}）をダウンロードします。確認なしに自動インストールされることはありません。"
                    )
                }
            }
        }
    }

    pub fn confirm_install_update_desc(
        self,
        version: &str,
        file_path: &str,
        sha_short: &str,
    ) -> String {
        #[cfg(target_os = "windows")]
        let hint = match self {
            Self::ZhCn => "点击「启动安装程序」将运行官方 Inno Setup 升级向导。",
            Self::En => "Click 'Run Installer' to launch the Inno Setup upgrade wizard.",
            Self::Ja => {
                "「インストーラーを起動」をクリックすると Inno Setup の更新ウィザードが開きます。"
            }
        };
        #[cfg(target_os = "macos")]
        let hint = match self {
            Self::ZhCn => {
                "点击「打开 DMG」挂载镜像后，将 UnoSkills.app 拖入 Applications 覆盖即可。"
            }
            Self::En => {
                "Click 'Open DMG' to mount the disk image, then drag UnoSkills.app into Applications."
            }
            Self::Ja => {
                "「DMG を開く」でイメージをマウントし、UnoSkills.app を Applications にドラッグしてください。"
            }
        };
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let hint = match self {
            Self::ZhCn => {
                "Linux .deb 安装包需通过包管理器安装（如 sudo apt install ./安装包.deb）。点击按钮可在文件管理器中定位该文件。"
            }
            Self::En => {
                "Install the downloaded .deb via your package manager (e.g. sudo apt install ./package.deb). Click below to reveal the file."
            }
            Self::Ja => {
                "Linux の .deb はパッケージマネージャ（例: sudo apt install ./package.deb）で導入してください。下のボタンでファイルを表示します。"
            }
        };
        match self {
            Self::ZhCn => format!(
                "{version} 已下载并通过 SHA-256 校验（{sha_short}...）。\n文件位置：{file_path}\n\n{hint}"
            ),
            Self::En => format!(
                "{version} downloaded and passed SHA-256 verification ({sha_short}...).\nSaved to: {file_path}\n\n{hint}"
            ),
            Self::Ja => format!(
                "{version} をダウンロードし SHA-256 検証に合格しました（{sha_short}...）。\n保存先：{file_path}\n\n{hint}"
            ),
        }
    }

    pub fn busy_downloading_update(self, tag: &str) -> String {
        match self {
            Self::ZhCn => format!("正在下载并校验 {tag} 安装包..."),
            Self::En => format!("Downloading & verifying {tag}..."),
            Self::Ja => format!("{tag} をダウンロード・検証中..."),
        }
    }

    pub fn toast_update_verified(self, tag: &str) -> String {
        match self {
            Self::ZhCn => format!("{tag} 已下载并通过 SHA-256 校验。"),
            Self::En => format!("{tag} downloaded and SHA-256 verified."),
            Self::Ja => format!("{tag} のダウンロードと SHA-256 検証が完了しました。"),
        }
    }

    pub fn confirm_delete_skill_desc(self, skill_name: &str, deploy_count: usize) -> String {
        match self {
            Self::ZhCn => format!(
                "将从中央技能库永久删除「{skill_name}」，并清理其在 {deploy_count} 个 Agent 中的部署。此操作不可撤销。"
            ),
            Self::En => format!(
                "Permanently remove '{skill_name}' from the central library and clean up {deploy_count} agent deployment(s). This cannot be undone."
            ),
            Self::Ja => format!(
                "「{skill_name}」を中央ライブラリから完全に削除し、{deploy_count} 件のエージェント配備も解除します。この操作は元に戻せません。"
            ),
        }
    }

    pub fn confirm_delete_stack_desc(self, stack_name: &str) -> String {
        match self {
            Self::ZhCn => {
                format!("将从「我的组合」中删除「{stack_name}」（不会卸载已安装的技能）。")
            }
            Self::En => {
                format!("Delete '{stack_name}' from My Stacks (installed skills are kept).")
            }
            Self::Ja => format!(
                "マイスタックから「{stack_name}」を削除します（導入済みスキルは残ります）。"
            ),
        }
    }

    pub fn confirm_conflict_desc(self, agent_name: &str, path: &str) -> String {
        match self {
            Self::ZhCn => format!(
                "{agent_name} 目标路径已存在未托管的目录或文件：\n{path}\n\n是否替换为 UnoSkills 受管部署？"
            ),
            Self::En => format!(
                "An unmanaged folder or file already exists for {agent_name} at:\n{path}\n\nReplace it with the UnoSkills managed deployment?"
            ),
            Self::Ja => format!(
                "{agent_name} の配備先に未管理のフォルダまたはファイルが存在します：\n{path}\n\nUnoSkills の管理対象に置き換えますか？"
            ),
        }
    }

    pub fn busy_updating_skill(self, name: &str) -> String {
        match self {
            Self::ZhCn => format!("正在检查并更新「{name}」..."),
            Self::En => format!("Updating '{name}'..."),
            Self::Ja => format!("「{name}」を更新確認中..."),
        }
    }

    pub fn toast_skill_updated(self, name: &str, tokens: u32, copies: usize) -> String {
        match self {
            Self::ZhCn => {
                if copies > 0 {
                    format!(
                        "已更新「{name}」（约 {tokens} tokens，已同步刷新 {copies} 个副本部署）"
                    )
                } else {
                    format!("已更新「{name}」（约 {tokens} tokens）")
                }
            }
            Self::En => {
                if copies > 0 {
                    format!(
                        "Updated '{name}' (~{tokens} tokens, refreshed {copies} copy deployment(s))."
                    )
                } else {
                    format!("Updated '{name}' (~{tokens} tokens).")
                }
            }
            Self::Ja => {
                if copies > 0 {
                    format!(
                        "「{name}」を更新しました（約 {tokens} tokens・コピー配備 {copies} 件を同期）"
                    )
                } else {
                    format!("「{name}」を更新しました（約 {tokens} tokens）")
                }
            }
        }
    }

    pub fn confirm_detach_project_desc(self, project_name: &str, project_path: &str) -> String {
        match self {
            Self::ZhCn => format!(
                "将取消关联项目「{project_name}」（{project_path}），并移除 UnoSkills 在该项目内创建的受管技能部署（不会影响全局技能库及项目源码）。"
            ),
            Self::En => format!(
                "Detach project '{project_name}' ({project_path}) and remove UnoSkills-managed skill deployments inside it (your global library and repo code are untouched)."
            ),
            Self::Ja => format!(
                "プロジェクト「{project_name}」（{project_path}）の関連付けを解除し、プロジェクト内の UnoSkills 管理対象配備を削除します（中央ライブラリとソースコードは保持されます）。"
            ),
        }
    }

    pub fn toast_project_attached(self, project_name: &str) -> String {
        match self {
            Self::ZhCn => format!("已关联项目「{project_name}」，可单独勾选部署到该项目。"),
            Self::En => format!("Attached project '{project_name}' for repo-local skills."),
            Self::Ja => format!("プロジェクト「{project_name}」を関連付けました。"),
        }
    }

    pub fn toast_project_deployed(
        self,
        project_name: &str,
        agent_key: &str,
        mode: DeployMode,
    ) -> String {
        match self {
            Self::ZhCn => format!("已部署到项目「{project_name}」的 {agent_key}（{mode:?}）"),
            Self::En => format!("Deployed to '{project_name}' -> {agent_key} ({mode:?})"),
            Self::Ja => format!("「{project_name}」の {agent_key} に配備しました（{mode:?}）"),
        }
    }

    pub fn toast_project_undeployed(self, project_name: &str, agent_key: &str) -> String {
        match self {
            Self::ZhCn => format!("已从项目「{project_name}」的 {agent_key} 取消部署"),
            Self::En => format!("Undeployed from '{project_name}' ({agent_key})"),
            Self::Ja => format!("「{project_name}」の {agent_key} から配備解除しました"),
        }
    }

    pub fn toast_skill_up_to_date(self, name: &str) -> String {
        match self {
            Self::ZhCn => format!("「{name}」已是最新版本，无需更新。"),
            Self::En => format!("'{name}' is already up to date."),
            Self::Ja => format!("「{name}」はすでに最新です。"),
        }
    }

    pub fn toast_custom_agent_added(self, name: &str) -> String {
        match self {
            Self::ZhCn => format!("已添加自定义 Agent「{name}」"),
            Self::En => format!("Added custom agent '{name}'."),
            Self::Ja => format!("カスタムエージェント「{name}」を追加しました"),
        }
    }
}

pub struct Strings {
    // Navigation & Chrome
    pub workspace_group: &'static str,
    pub nav_library: &'static str,
    pub nav_stacks: &'static str,
    pub nav_discover: &'static str,
    pub nav_settings: &'static str,
    pub tooltip_refresh: &'static str,
    pub tooltip_theme: &'static str,
    pub tooltip_lang: &'static str,
    pub agents_detected_suffix: &'static str,

    // Library
    pub my_skills: &'static str,
    pub btn_install: &'static str,
    pub ph_search_skills: &'static str,
    pub empty_library_title: &'static str,
    pub empty_library_desc: &'static str,
    pub empty_search_title: &'static str,
    pub empty_search_desc: &'static str,
    pub btn_browse_stacks: &'static str,
    pub btn_discover_adopt: &'static str,
    pub library_only: &'static str,
    pub agents_unit: &'static str,
    pub empty_select_skill_title: &'static str,
    pub empty_select_skill_desc: &'static str,
    pub btn_edit_skill: &'static str,
    pub btn_update_skill: &'static str,
    pub btn_save: &'static str,
    pub btn_cancel: &'static str,
    pub btn_delete: &'static str,
    pub btn_replace: &'static str,
    pub tooltip_remove_skill: &'static str,
    pub tooltip_reveal_folder: &'static str,
    pub confirm_delete_skill_title: &'static str,
    pub confirm_delete_stack_title: &'static str,
    pub confirm_conflict_title: &'static str,
    pub btn_browse_folder: &'static str,
    pub prompt_pick_skill_dir: &'static str,
    pub prompt_pick_agent_dir: &'static str,
    pub deploy_to_agents: &'static str,
    pub no_installed_agents: &'static str,
    pub section_description: &'static str,
    pub ph_skill_md: &'static str,

    // Stacks
    pub stacks_title: &'static str,
    pub stacks_subtitle: &'static str,
    pub tab_featured: &'static str,
    pub tab_my_stacks: &'static str,
    pub tab_create_share: &'static str,
    pub empty_local_stacks_title: &'static str,
    pub empty_featured_stacks_title: &'static str,
    pub empty_stacks_desc: &'static str,
    pub btn_create_first_stack: &'static str,
    pub badge_verified: &'static str,
    pub badge_local: &'static str,
    pub skills_unit: &'static str,
    pub empty_select_stack_title: &'static str,
    pub empty_select_stack_desc: &'static str,
    pub btn_install_stack: &'static str,
    pub btn_copy_toml: &'static str,
    pub recommended_agents: &'static str,
    pub included_skills: &'static str,
    pub already_in_library: &'static str,
    pub will_install: &'static str,
    pub create_stack_title: &'static str,
    pub create_stack_desc: &'static str,
    pub field_stack_name: &'static str,
    pub field_stack_author: &'static str,
    pub field_stack_desc: &'static str,
    pub field_select_skills: &'static str,
    pub no_skills_to_bundle: &'static str,
    pub btn_save_stack: &'static str,
    pub import_stack_title: &'static str,
    pub import_stack_desc: &'static str,
    pub btn_import_stack: &'static str,
    pub ph_stack_name: &'static str,
    pub ph_stack_author: &'static str,
    pub ph_stack_desc: &'static str,
    pub ph_stack_import: &'static str,

    // Discover
    pub discover_title: &'static str,
    pub discover_subtitle: &'static str,
    pub tab_marketplace_install: &'static str,
    pub tab_adopt_existing: &'static str,
    pub quick_install_title: &'static str,
    pub ph_install_source: &'static str,
    pub ph_market_search: &'static str,
    pub btn_search: &'static str,
    pub btn_trending: &'static str,
    pub board_trending: &'static str,
    pub board_hot: &'static str,
    pub board_all_time: &'static str,
    pub market_loading: &'static str,
    pub market_loading_more: &'static str,
    pub btn_load_more: &'static str,
    pub empty_market_title: &'static str,
    pub empty_market_desc: &'static str,
    pub installs_unit: &'static str,
    pub ph_adopt_search: &'static str,
    pub btn_rescan: &'static str,
    pub btn_adopt_all: &'static str,
    pub btn_adopt: &'static str,
    pub empty_adopt_done_title: &'static str,
    pub empty_adopt_done_desc: &'static str,
    pub empty_adopt_filter_title: &'static str,
    pub empty_adopt_filter_desc: &'static str,

    // Settings
    pub settings_title: &'static str,
    pub settings_general: &'static str,
    pub settings_deploy_mode: &'static str,
    pub settings_deploy_mode_desc: &'static str,
    pub mode_symlink: &'static str,
    pub mode_copy: &'static str,
    pub settings_appearance: &'static str,
    pub settings_appearance_desc: &'static str,
    pub theme_dark: &'static str,
    pub theme_light: &'static str,
    pub settings_language: &'static str,
    pub settings_language_desc: &'static str,
    pub settings_fonts: &'static str,
    pub settings_ui_font: &'static str,
    pub settings_ui_font_desc: &'static str,
    pub settings_mono_font: &'static str,
    pub settings_mono_font_desc: &'static str,
    pub settings_font_size: &'static str,
    pub settings_font_size_desc: &'static str,
    pub font_auto_by_lang: &'static str,
    pub font_auto_mono: &'static str,
    pub settings_updates: &'static str,
    pub settings_current_version: &'static str,
    pub settings_updates_desc: &'static str,
    pub btn_check_updates: &'static str,
    pub settings_network: &'static str,
    pub settings_http_proxy: &'static str,
    pub settings_http_proxy_desc: &'static str,
    pub ph_http_proxy: &'static str,
    pub settings_github_token: &'static str,
    pub settings_github_token_desc: &'static str,
    pub ph_github_token: &'static str,
    pub settings_root_dir: &'static str,
    pub settings_agents_registry: &'static str,
    pub settings_agents_registry_desc: &'static str,
    pub settings_add_custom_agent: &'static str,
    pub ph_custom_agent_name: &'static str,
    pub ph_custom_agent_dir: &'static str,
    pub btn_add_agent: &'static str,
    pub badge_detected: &'static str,
    pub badge_not_detected: &'static str,

    // Static Toasts & Busy messages
    pub toast_refreshed: &'static str,
    pub toast_removed_skill: &'static str,
    pub toast_enter_source: &'static str,
    pub toast_deleted_stack: &'static str,
    pub toast_enter_stack_name: &'static str,
    pub toast_select_stack_skill: &'static str,
    pub toast_paste_toml: &'static str,
    pub toast_network_saved: &'static str,
    pub toast_custom_agent_removed: &'static str,
    pub toast_custom_agent_required: &'static str,
    pub scope_global: &'static str,
    pub btn_add_project: &'static str,
    pub btn_detach_project: &'static str,
    pub confirm_detach_project_title: &'static str,
    pub prompt_pick_project_folder: &'static str,
    pub toast_project_detached: &'static str,
    pub busy_loading_trending: &'static str,
    pub busy_checking_updates: &'static str,
    pub ph_stack_search: &'static str,
    pub badge_community: &'static str,
    pub btn_share_gist: &'static str,
    pub btn_refresh_community: &'static str,
    pub busy_refreshing_community: &'static str,
    pub toast_community_failed: &'static str,
    pub confirm_download_update_title: &'static str,
    pub confirm_install_update_title: &'static str,
    pub btn_download_update: &'static str,
    pub btn_open_release: &'static str,
    pub btn_install_update_win: &'static str,
    pub btn_install_update_mac: &'static str,
    pub btn_install_update_linux: &'static str,
    pub badge_update_available: &'static str,
    pub badge_sha256_verified: &'static str,
}
