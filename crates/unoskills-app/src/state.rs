use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Theme, WindowExt};
use gpui_kit::{
    App, ClipboardItem, Context, EventEmitter, PathPromptOptions, SharedString, Window,
};
use unoskills_core::updater::{DownloadedUpdate, UpdateCheckOutcome, launch_platform_installer};
use unoskills_core::{
    AdoptCandidate, AgentCategory, AgentInfo, CustomAgentRecord, DeployMode, MarketplacePage,
    MarketplaceSkill, ProjectWorkspace, Skill, StackManifest, StackSkillRef, UnoSkillsManager,
};

use crate::fonts::{
    self, AccentPalette, AppearancePreference, DEFAULT_CORNER_RADIUS, DEFAULT_FONT_SIZE, FONT_AUTO,
    FontSettings,
};
use crate::i18n::{Lang, Strings};

/// Top-level navigation target, shared between the shell and feature views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTab {
    Home,
    Library,
    Stacks,
    Discover,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarketBoard {
    Trending,
    Hot,
    AllTime,
}

impl MarketBoard {
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::Trending => "trending",
            Self::Hot => "hot",
            Self::AllTime => "all-time",
        }
    }

    pub fn label(self, strings: &Strings) -> &'static str {
        match self {
            Self::Trending => strings.board_trending,
            Self::Hot => strings.board_hot,
            Self::AllTime => strings.board_all_time,
        }
    }
}

pub enum MarketFirstPage {
    Loading,
    Loaded(MarketplacePage),
    Failed(String),
}

/// Sub-section inside the Settings view, supporting direct deep-linking
/// (e.g. from the TitleBar update pill or the Sidebar detected-agents footer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    General,
    Appearance,
    SyncAndNetwork,
    Agents,
    Updates,
}

/// Events broadcast to every subscribed feature view.
///
/// GPUI defers `cx.emit` through its effects queue, so emitters never run
/// inside another entity's mutable borrow.
#[derive(Debug, Clone)]
pub enum WorkspaceEvent {
    /// Domain snapshots (skills, stacks, agents, adoptable skills) were reloaded.
    Reloaded,
    /// Lightweight preference/status update (fonts, theme, deploy mode, update check).
    /// Avoids forcing views like `LibraryView` to re-read `SKILL.md` from disk.
    SettingsChanged,
    /// UI language changed: views must refresh placeholders and re-render.
    LanguageChanged,
    /// The shell should switch its active top-level tab.
    NavigateTo(NavTab),
    /// The Settings view should switch its active sub-section.
    NavigateToSettings(SettingsSection),
    OpenMyStacks,
    OpenLocalSkills,
    OpenMarketBoard(MarketBoard),
    MarketFirstPageChanged(MarketBoard),
    OpenInstallDialog,
    SelectLibrarySkill(String),
    /// The shared busy indicator (TitleBar spinner) changed.
    BusyChanged,
    /// A skill or stack installation started or finished.
    InstallStateChanged,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum MarketInstallStatus {
    Available,
    Installed,
    SameName,
}

/// Shared application state: owns the `UnoSkillsManager` facade, preferences,
/// cached domain snapshots, and the concurrent busy task stack. Feature views
/// subscribe to [`WorkspaceEvent`] and stay thin.
pub struct WorkspaceState {
    manager: Arc<UnoSkillsManager>,
    lang: Lang,
    appearance: AppearancePreference,
    skills: Vec<Skill>,
    installed_skill_names: HashSet<String>,
    installed_source_refs: HashSet<String>,
    installing_refs: HashSet<String>,
    installing_stack_ids: HashSet<String>,
    stack_install_failures: HashMap<String, Vec<(String, String)>>,
    agents: Vec<AgentInfo>,
    projects: Vec<ProjectWorkspace>,
    featured_stacks: Vec<StackManifest>,
    community_stacks: Vec<StackManifest>,
    local_stacks: Vec<StackManifest>,
    adopt_candidates: Vec<AdoptCandidate>,
    market_first_pages: HashMap<MarketBoard, MarketFirstPage>,
    deploy_mode: DeployMode,
    font_settings: FontSettings,
    checking_updates: bool,
    downloading_update: bool,
    available_update: Option<UpdateCheckOutcome>,
    downloaded_update: Option<DownloadedUpdate>,
    updating_skill_id: Option<String>,
    adopting_all: bool,
    backup_busy: bool,
    next_busy_token: u64,
    busy_tasks: Vec<(u64, String)>,
    last_added_skill_id: Option<String>,
}

impl EventEmitter<WorkspaceEvent> for WorkspaceState {}

impl WorkspaceState {
    pub fn new() -> Self {
        let manager = Arc::new(
            UnoSkillsManager::open_default()
                .expect("Failed to initialize UnoSkills data directory (~/.unoskills)"),
        );
        let lang = manager
            .get_setting("language")
            .ok()
            .flatten()
            .map(|code| Lang::from_code(&code))
            .unwrap_or(Lang::ZhCn);
        gpui_kit::component::set_locale(lang.code());

        let appearance = manager
            .get_setting("theme")
            .ok()
            .flatten()
            .map(|value| AppearancePreference::from_code(&value))
            .unwrap_or_default();

        let ui_font = manager
            .get_setting("ui_font")
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| FONT_AUTO.to_string());
        let mono_font = manager
            .get_setting("mono_font")
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| FONT_AUTO.to_string());
        let font_size = manager
            .get_setting("font_size")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(DEFAULT_FONT_SIZE);
        let palette = manager
            .get_setting("accent_palette")
            .ok()
            .flatten()
            .map(|s| AccentPalette::from_code(&s))
            .unwrap_or_default();
        let corner_radius = manager
            .get_setting("corner_radius")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(DEFAULT_CORNER_RADIUS);

        let mut state = Self {
            manager,
            lang,
            appearance,
            skills: Vec::new(),
            installed_skill_names: HashSet::new(),
            installed_source_refs: HashSet::new(),
            installing_refs: HashSet::new(),
            installing_stack_ids: HashSet::new(),
            stack_install_failures: HashMap::new(),
            agents: Vec::new(),
            projects: Vec::new(),
            featured_stacks: Vec::new(),
            community_stacks: Vec::new(),
            local_stacks: Vec::new(),
            adopt_candidates: Vec::new(),
            market_first_pages: HashMap::new(),
            deploy_mode: DeployMode::Symlink,
            font_settings: FontSettings {
                ui_font,
                mono_font,
                font_size,
                palette,
                corner_radius,
            },
            checking_updates: false,
            downloading_update: false,
            available_update: None,
            downloaded_update: None,
            updating_skill_id: None,
            adopting_all: false,
            backup_busy: false,
            next_busy_token: 1,
            busy_tasks: Vec::new(),
            last_added_skill_id: None,
        };
        state.refresh_snapshots();
        state
    }

    /// Applies the persisted appearance and language-aware fonts at startup.
    pub fn apply_initial_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        Theme::change(
            self.appearance.resolve(window.appearance()),
            Some(window),
            cx,
        );
        fonts::apply_fonts_to_theme(&self.font_settings, self.lang, cx);
        // Fetch the remote community stacks index once at startup, silently.
        self.refresh_community_stacks(true, window, cx);
        // Rate-limited (24h) silent startup update check — never downloads silently.
        if self.manager.should_auto_check_updates() {
            self.check_for_updates_async(true, window, cx);
        }
    }

    // ── Read accessors ──

    pub fn manager(&self) -> &Arc<UnoSkillsManager> {
        &self.manager
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    pub fn t(&self) -> &'static Strings {
        self.lang.t()
    }

    pub fn appearance(&self) -> AppearancePreference {
        self.appearance
    }

    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    pub fn agents(&self) -> &[AgentInfo] {
        &self.agents
    }

    pub fn projects(&self) -> &[ProjectWorkspace] {
        &self.projects
    }

    pub fn installed_agent_count(&self) -> usize {
        self.agents.iter().filter(|a| a.installed).count()
    }

    pub fn featured_stacks(&self) -> &[StackManifest] {
        &self.featured_stacks
    }

    pub fn community_stacks(&self) -> &[StackManifest] {
        &self.community_stacks
    }

    pub fn local_stacks(&self) -> &[StackManifest] {
        &self.local_stacks
    }

    pub fn adopt_candidates(&self) -> &[AdoptCandidate] {
        &self.adopt_candidates
    }

    pub fn deploy_mode(&self) -> DeployMode {
        self.deploy_mode
    }

    pub fn font_settings(&self) -> &FontSettings {
        &self.font_settings
    }

    pub fn checking_updates(&self) -> bool {
        self.checking_updates
    }

    pub fn downloading_update(&self) -> bool {
        self.downloading_update
    }

    pub fn available_update(&self) -> Option<&UpdateCheckOutcome> {
        self.available_update.as_ref()
    }

    pub fn downloaded_update(&self) -> Option<&DownloadedUpdate> {
        self.downloaded_update.as_ref()
    }

    pub fn updating_skill_id(&self) -> Option<&str> {
        self.updating_skill_id.as_deref()
    }

    pub fn adopting_all(&self) -> bool {
        self.adopting_all
    }

    pub fn backup_busy(&self) -> bool {
        self.backup_busy
    }

    pub fn busy_label(&self) -> Option<&str> {
        self.busy_tasks.last().map(|(_, label)| label.as_str())
    }

    pub fn market_install_status(&self, item: &MarketplaceSkill) -> MarketInstallStatus {
        if self
            .installed_source_refs
            .contains(&item.install_ref().to_lowercase())
        {
            MarketInstallStatus::Installed
        } else if self
            .installed_skill_names
            .contains(&item.skill_id.to_lowercase())
            || self
                .installed_skill_names
                .contains(&item.name.to_lowercase())
        {
            MarketInstallStatus::SameName
        } else {
            MarketInstallStatus::Available
        }
    }

    /// Marks a freshly installed/adopted skill so the Library auto-selects it
    /// after the next reload.
    pub fn note_skill_added(&mut self, skill_id: &str) {
        self.last_added_skill_id = Some(skill_id.to_string());
    }

    /// Consumes the "auto-select this skill" hint (one-shot).
    pub fn take_last_added_skill_id(&mut self) -> Option<String> {
        self.last_added_skill_id.take()
    }

    // ── Toasts & concurrent busy indicator ──

    pub fn notify_toast(
        &mut self,
        is_error: bool,
        message: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let note = if is_error {
            Notification::error(message.into())
        } else {
            Notification::success(message.into())
        };
        window.push_notification(note, cx);
    }

    /// Registers a concurrent background task in the TitleBar spinner stack and
    /// returns its unique token for [`Self::end_busy`].
    pub fn begin_busy(&mut self, label: impl Into<SharedString>, cx: &mut Context<Self>) -> u64 {
        let token = self.next_busy_token;
        self.next_busy_token = self.next_busy_token.wrapping_add(1).max(1);
        self.busy_tasks.push((token, label.into().to_string()));
        cx.emit(WorkspaceEvent::BusyChanged);
        token
    }

    /// Removes only the matching task token so overlapping async operations
    /// never prematurely clear each other's spinner.
    pub fn end_busy(&mut self, token: u64, cx: &mut Context<Self>) {
        if let Some(pos) = self.busy_tasks.iter().position(|(t, _)| *t == token) {
            self.busy_tasks.remove(pos);
            cx.emit(WorkspaceEvent::BusyChanged);
        }
    }

    // ── Shared mutations ──

    pub fn reload_all(&mut self, cx: &mut Context<Self>) {
        self.refresh_snapshots();
        cx.emit(WorkspaceEvent::Reloaded);
    }

    fn refresh_snapshots(&mut self) {
        self.skills = self.manager.list_skills().unwrap_or_default();
        self.installed_skill_names.clear();
        self.installed_source_refs.clear();
        for skill in &self.skills {
            self.installed_skill_names.insert(skill.slug.to_lowercase());
            self.installed_skill_names.insert(skill.name.to_lowercase());
            if let Some(source_ref) = &skill.source_ref {
                self.installed_source_refs.insert(source_ref.to_lowercase());
            }
        }
        self.agents = self.manager.list_agents().unwrap_or_default();
        self.projects = self.manager.list_projects().unwrap_or_default();
        self.featured_stacks = self.manager.featured_stacks().unwrap_or_default();
        self.local_stacks = self.manager.list_local_stacks().unwrap_or_default();
        self.adopt_candidates = self.manager.scan_adoptable().unwrap_or_default();
        self.deploy_mode = self.manager.default_deploy_mode();
    }

    pub fn set_appearance(
        &mut self,
        appearance: AppearancePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.manager.set_setting("theme", appearance.code()) {
            self.notify_toast(true, error.to_string(), window, cx);
            return;
        }
        self.appearance = appearance;
        self.sync_appearance(window, cx);
        cx.emit(WorkspaceEvent::SettingsChanged);
    }

    pub fn sync_appearance(&self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = self.appearance.resolve(window.appearance());
        if mode != Theme::global(cx).mode {
            Theme::change(mode, Some(window), cx);
            fonts::apply_fonts_to_theme(&self.font_settings, self.lang, cx);
        }
    }

    pub fn set_language(&mut self, lang: Lang, cx: &mut Context<Self>) {
        if self.lang == lang {
            return;
        }
        self.lang = lang;
        let _ = self.manager.set_setting("language", lang.code());
        gpui_kit::component::set_locale(lang.code());
        let fs = self.font_settings.clone();
        fonts::apply_fonts_to_theme(&fs, lang, cx);
        cx.emit(WorkspaceEvent::LanguageChanged);
    }

    pub fn set_ui_font(&mut self, ui_font: String, cx: &mut Context<Self>) {
        self.font_settings.ui_font = ui_font.clone();
        let _ = self.manager.set_setting("ui_font", &ui_font);
        let fs = self.font_settings.clone();
        let lang = self.lang;
        fonts::apply_fonts_to_theme(&fs, lang, cx);
        cx.emit(WorkspaceEvent::SettingsChanged);
    }

    pub fn set_mono_font(&mut self, mono_font: String, cx: &mut Context<Self>) {
        self.font_settings.mono_font = mono_font.clone();
        let _ = self.manager.set_setting("mono_font", &mono_font);
        let fs = self.font_settings.clone();
        let lang = self.lang;
        fonts::apply_fonts_to_theme(&fs, lang, cx);
        cx.emit(WorkspaceEvent::SettingsChanged);
    }

    pub fn set_font_size(&mut self, font_size: f32, cx: &mut Context<Self>) {
        self.font_settings.font_size = font_size;
        let _ = self
            .manager
            .set_setting("font_size", &font_size.to_string());
        let fs = self.font_settings.clone();
        let lang = self.lang;
        fonts::apply_fonts_to_theme(&fs, lang, cx);
        cx.emit(WorkspaceEvent::SettingsChanged);
    }

    pub fn set_accent_palette(&mut self, palette: AccentPalette, cx: &mut Context<Self>) {
        self.font_settings.palette = palette;
        let _ = self.manager.set_setting("accent_palette", palette.code());
        let fs = self.font_settings.clone();
        let lang = self.lang;
        fonts::apply_fonts_to_theme(&fs, lang, cx);
        cx.emit(WorkspaceEvent::SettingsChanged);
    }

    pub fn set_corner_radius(&mut self, radius: f32, cx: &mut Context<Self>) {
        self.font_settings.corner_radius = radius;
        let _ = self
            .manager
            .set_setting("corner_radius", &radius.to_string());
        let fs = self.font_settings.clone();
        let lang = self.lang;
        fonts::apply_fonts_to_theme(&fs, lang, cx);
        cx.emit(WorkspaceEvent::SettingsChanged);
    }

    pub fn check_for_updates_async(
        &mut self,
        silent: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.checking_updates {
            return;
        }
        self.checking_updates = true;
        let busy_token = if silent {
            0
        } else {
            self.begin_busy(self.t().busy_checking_updates, cx)
        };
        cx.emit(WorkspaceEvent::SettingsChanged);

        let current_ver = env!("CARGO_PKG_VERSION").to_string();
        let mgr = self.manager.clone();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg
                .spawn(async move { mgr.check_app_update(&current_ver, true) })
                .await;

            let _ = this.update_in(cx, |this, window, cx| {
                this.checking_updates = false;
                if !silent {
                    this.end_busy(busy_token, cx);
                }
                match res {
                    Ok(outcome) => {
                        if outcome.update_available
                            && let Some(release) = &outcome.latest
                        {
                            let tag = release.tag_name.clone();
                            this.available_update = Some(outcome);
                            let msg = this.lang.toast_update_available(&tag);
                            this.notify_toast(false, msg, window, cx);
                        } else {
                            this.available_update = None;
                            if !silent {
                                let msg = this.lang.toast_up_to_date(env!("CARGO_PKG_VERSION"));
                                this.notify_toast(false, msg, window, cx);
                            }
                        }
                    }
                    Err(e) if !silent => {
                        this.notify_toast(true, format!("{e}"), window, cx);
                    }
                    Err(_) => {}
                }
                cx.emit(WorkspaceEvent::SettingsChanged);
            });
        })
        .detach();
    }

    /// Prompts for explicit user consent via an `AlertDialog` before downloading
    /// and SHA-256 verifying the platform installer.
    pub fn prompt_download_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(outcome) = self.available_update.clone() else {
            return;
        };
        let Some(release) = outcome.latest.clone() else {
            return;
        };
        let Some(asset) = outcome.recommended_asset.clone() else {
            if !release.html_url.is_empty() {
                cx.open_url(&release.html_url);
            }
            return;
        };
        let has_checksums = outcome.checksums_asset.is_some();
        let ws_entity = cx.entity().clone();
        let s = self.t();
        let title = s.confirm_download_update_title;
        let desc =
            self.lang
                .confirm_download_update_desc(&release.tag_name, &asset.name, has_checksums);
        let ok_label = s.btn_download_update;
        let cancel_label = s.btn_cancel;

        window.open_alert_dialog(cx, move |alert, _, _| {
            let ws_entity = ws_entity.clone();
            alert
                .confirm()
                .centered()
                .title(title)
                .description(desc.clone())
                .ok_text(ok_label)
                .ok_variant(ButtonVariant::Primary)
                .cancel_text(cancel_label)
                .on_ok(move |_, window, cx| {
                    ws_entity.update(cx, |ws, cx| {
                        ws.download_update_async(window, cx);
                    });
                    true
                })
        });
    }

    fn download_update_async(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.downloading_update {
            return;
        }
        let Some(outcome) = self.available_update.clone() else {
            return;
        };
        let Some(release) = outcome.latest else {
            return;
        };
        let Some(asset) = outcome.recommended_asset else {
            return;
        };
        let checksums = outcome.checksums_asset;

        self.downloading_update = true;
        let busy_token = self.begin_busy(self.lang.busy_downloading_update(&release.tag_name), cx);
        cx.emit(WorkspaceEvent::SettingsChanged);

        let mgr = self.manager.clone();
        let tag = release.tag_name.clone();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg
                .spawn(async move { mgr.download_app_update(&tag, &asset, checksums.as_ref()) })
                .await;
            let _ = this.update_in(cx, |state, window, cx| {
                state.downloading_update = false;
                state.end_busy(busy_token, cx);
                match res {
                    Ok(downloaded) => {
                        let version = downloaded.version.clone();
                        state.downloaded_update = Some(downloaded.clone());
                        let msg = state.lang.toast_update_verified(&version);
                        state.notify_toast(false, msg, window, cx);
                        state.prompt_install_downloaded_update(&downloaded, window, cx);
                    }
                    Err(e) => {
                        state.notify_toast(true, format!("{e}"), window, cx);
                    }
                }
                cx.emit(WorkspaceEvent::SettingsChanged);
            });
        })
        .detach();
    }

    fn prompt_install_downloaded_update(
        &self,
        downloaded: &DownloadedUpdate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ws_entity = cx.entity().clone();
        let s = self.t();
        let title = s.confirm_install_update_title;
        let sha_short = downloaded.sha256.get(..12).unwrap_or(&downloaded.sha256);
        let desc = self.lang.confirm_install_update_desc(
            &downloaded.version,
            &downloaded.file_path.to_string_lossy(),
            sha_short,
        );
        let ok_label = platform_install_button_label(s);
        let cancel_label = s.btn_cancel;

        window.open_alert_dialog(cx, move |alert, _, _| {
            let ws_entity = ws_entity.clone();
            alert
                .confirm()
                .centered()
                .title(title)
                .description(desc.clone())
                .ok_text(ok_label)
                .ok_variant(ButtonVariant::Primary)
                .cancel_text(cancel_label)
                .on_ok(move |_, window, cx| {
                    ws_entity.update(cx, |ws, cx| {
                        ws.apply_downloaded_update(window, cx);
                    });
                    true
                })
        });
    }

    /// Executes the platform-appropriate install guidance for a verified download:
    /// Windows launches Inno Setup `.exe`, macOS mounts `.dmg`, Linux reveals `.deb`.
    pub fn apply_downloaded_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(downloaded) = self.downloaded_update.clone() else {
            return;
        };
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            if let Err(e) = launch_platform_installer(&downloaded.file_path) {
                self.notify_toast(true, format!("{e}"), window, cx);
            }
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = (window, launch_platform_installer);
            cx.reveal_path(&downloaded.file_path);
        }
    }

    pub fn navigate(&mut self, tab: NavTab, cx: &mut Context<Self>) {
        cx.emit(WorkspaceEvent::NavigateTo(tab));
    }

    pub fn open_my_stacks(&mut self, cx: &mut Context<Self>) {
        self.navigate(NavTab::Stacks, cx);
        cx.emit(WorkspaceEvent::OpenMyStacks);
    }

    pub fn open_local_skills(&mut self, cx: &mut Context<Self>) {
        cx.emit(WorkspaceEvent::OpenLocalSkills);
        self.navigate(NavTab::Discover, cx);
    }

    pub fn open_market_board(&mut self, board: MarketBoard, cx: &mut Context<Self>) {
        cx.emit(WorkspaceEvent::OpenMarketBoard(board));
        self.navigate(NavTab::Discover, cx);
    }

    pub fn market_first_page(&self, board: MarketBoard) -> Option<&MarketFirstPage> {
        self.market_first_pages.get(&board)
    }

    pub fn load_market_first_page(
        &mut self,
        board: MarketBoard,
        refresh: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.market_first_page(board) {
            Some(MarketFirstPage::Loading) => return,
            Some(MarketFirstPage::Loaded(_)) if !refresh => return,
            _ => {}
        }

        self.market_first_pages
            .insert(board, MarketFirstPage::Loading);
        cx.emit(WorkspaceEvent::MarketFirstPageChanged(board));
        let busy_token = self.begin_busy(self.t().busy_loading_market, cx);
        let manager = self.manager.clone();
        let background = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = background
                .spawn(async move { manager.marketplace_leaderboard_page(board.as_api_str(), 0) })
                .await;
            let _ = this.update_in(cx, |state, _, cx| {
                state.end_busy(busy_token, cx);
                let page = match result {
                    Ok(page) => MarketFirstPage::Loaded(page),
                    Err(error) => MarketFirstPage::Failed(error.to_string()),
                };
                state.market_first_pages.insert(board, page);
                cx.emit(WorkspaceEvent::MarketFirstPageChanged(board));
            });
        })
        .detach();
    }

    pub fn open_library_skill(&mut self, skill_id: String, cx: &mut Context<Self>) {
        cx.emit(WorkspaceEvent::NavigateTo(NavTab::Library));
        cx.emit(WorkspaceEvent::SelectLibrarySkill(skill_id));
    }

    pub fn open_install(&mut self, cx: &mut Context<Self>) {
        cx.emit(WorkspaceEvent::NavigateTo(NavTab::Discover));
        cx.emit(WorkspaceEvent::OpenInstallDialog);
    }

    pub fn navigate_settings(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        cx.emit(WorkspaceEvent::NavigateTo(NavTab::Settings));
        cx.emit(WorkspaceEvent::NavigateToSettings(section));
    }

    pub fn save_skill_markdown(
        &mut self,
        skill_id: &str,
        content: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match self.manager.save_skill_markdown(skill_id, content) {
            Ok(updated) => {
                self.reload_all(cx);
                let msg = self
                    .lang
                    .toast_saved_skill(&updated.name, updated.estimated_tokens);
                self.notify_toast(false, msg, window, cx);
                true
            }
            Err(e) => {
                self.notify_toast(true, format!("{e}"), window, cx);
                false
            }
        }
    }

    pub fn update_skill_async(
        &mut self,
        skill_id: String,
        skill_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.updating_skill_id.is_some() {
            return;
        }
        self.updating_skill_id = Some(skill_id.clone());
        let busy_label = self.lang.busy_updating_skill(&skill_name);
        let busy_token = self.begin_busy(busy_label, cx);
        cx.emit(WorkspaceEvent::SettingsChanged);

        let mgr = self.manager.clone();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg.spawn(async move { mgr.update_skill(&skill_id) }).await;
            let _ = this.update_in(cx, |state, window, cx| {
                state.updating_skill_id = None;
                state.end_busy(busy_token, cx);
                match res {
                    Ok(outcome) => {
                        if outcome.updated {
                            state.reload_all(cx);
                            let msg = state.lang.toast_skill_updated(
                                &outcome.skill.name,
                                outcome.skill.estimated_tokens,
                                outcome.refreshed_copies,
                            );
                            state.notify_toast(false, msg, window, cx);
                        } else {
                            cx.emit(WorkspaceEvent::SettingsChanged);
                            let msg = state.lang.toast_skill_up_to_date(&outcome.skill.name);
                            state.notify_toast(false, msg, window, cx);
                        }
                    }
                    Err(e) => {
                        cx.emit(WorkspaceEvent::SettingsChanged);
                        state.notify_toast(true, format!("{e}"), window, cx);
                    }
                }
            });
        })
        .detach();
    }

    pub fn toggle_skill_deployment(
        &mut self,
        skill_id: &str,
        agent_key: &str,
        currently_deployed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = if currently_deployed {
            self.manager
                .undeploy_skill(skill_id, agent_key)
                .map(|_| self.lang.toast_undeployed(agent_key))
        } else {
            self.manager
                .deploy_skill(skill_id, agent_key, None, false)
                .map(|dep| self.lang.toast_deployed(&dep.agent_key, dep.mode))
        };
        match result {
            Ok(msg) => {
                self.reload_all(cx);
                self.notify_toast(false, msg, window, cx);
            }
            Err(e) => {
                let err_str = format!("{e}");
                if !currently_deployed && err_str.contains("TARGET_CONFLICT") {
                    let agent_name = self
                        .agents
                        .iter()
                        .find(|a| a.key == agent_key)
                        .map(|a| a.display_name.clone())
                        .unwrap_or_else(|| agent_key.to_string());
                    let target_path =
                        self.agents
                            .iter()
                            .find(|a| a.key == agent_key)
                            .and_then(|a| {
                                self.skills.iter().find(|s| s.id == skill_id).map(|s| {
                                    a.skills_dir.join(&s.slug).to_string_lossy().to_string()
                                })
                            })
                            .unwrap_or_else(|| err_str.clone());
                    let ws_entity = cx.entity().clone();
                    let skill_id_owned = skill_id.to_string();
                    let agent_key_owned = agent_key.to_string();
                    let s = self.t();
                    let title = s.confirm_conflict_title;
                    let desc = self.lang.confirm_conflict_desc(&agent_name, &target_path);
                    let ok_label = s.btn_replace;
                    let cancel_label = s.btn_cancel;

                    window.open_alert_dialog(cx, move |alert, _, _| {
                        let ws_entity = ws_entity.clone();
                        let skill_id_owned = skill_id_owned.clone();
                        let agent_key_owned = agent_key_owned.clone();
                        alert
                            .confirm()
                            .centered()
                            .title(title)
                            .description(desc.clone())
                            .ok_text(ok_label)
                            .ok_variant(ButtonVariant::Danger)
                            .cancel_text(cancel_label)
                            .on_ok(move |_, window, cx| {
                                ws_entity.update(cx, |ws, cx| {
                                    ws.force_deploy_skill(
                                        &skill_id_owned,
                                        &agent_key_owned,
                                        window,
                                        cx,
                                    );
                                });
                                true
                            })
                    });
                } else {
                    self.notify_toast(true, err_str, window, cx);
                }
            }
        }
    }

    pub fn force_deploy_skill(
        &mut self,
        skill_id: &str,
        agent_key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.manager.deploy_skill(skill_id, agent_key, None, true) {
            Ok(dep) => {
                self.reload_all(cx);
                let msg = self.lang.toast_deployed(&dep.agent_key, dep.mode);
                self.notify_toast(false, msg, window, cx);
            }
            Err(e) => self.notify_toast(true, format!("{e}"), window, cx),
        }
    }

    pub fn pick_and_attach_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_attached: impl FnOnce(String, &mut Window, &mut App) + 'static,
    ) {
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(self.t().prompt_pick_project_folder.into()),
        });
        let manager = self.manager.clone();
        let background = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let paths = match selection.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Err(error)) => {
                    let _ = this.update_in(cx, |state, window, cx| {
                        state.notify_toast(true, error.to_string(), window, cx);
                    });
                    return;
                }
                _ => return,
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let result = background
                .spawn(async move { manager.add_project(&path, None) })
                .await;
            let attached = this.update_in(cx, |state, window, cx| match result {
                Ok(project) => {
                    state.reload_all(cx);
                    let message = state.lang.toast_project_attached(&project.name);
                    state.notify_toast(false, message, window, cx);
                    Some(project.id)
                }
                Err(error) => {
                    state.notify_toast(true, error.to_string(), window, cx);
                    None
                }
            });
            if let Ok(Some(id)) = attached {
                let _ = cx.update(|window, cx| on_attached(id, window, cx));
            }
        })
        .detach();
    }

    pub fn remove_project(
        &mut self,
        project_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.manager.remove_project(project_id) {
            Ok(()) => {
                self.reload_all(cx);
                let msg = self.t().toast_project_detached;
                self.notify_toast(false, msg, window, cx);
            }
            Err(e) => self.notify_toast(true, format!("{e}"), window, cx),
        }
    }

    pub fn toggle_project_skill_deployment(
        &mut self,
        skill_id: &str,
        project_id: &str,
        agent_key: &str,
        currently_deployed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project_name = self
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| project_id.to_string());

        let result = if currently_deployed {
            self.manager
                .undeploy_skill_from_project(skill_id, project_id, agent_key)
                .map(|_| self.lang.toast_project_undeployed(&project_name, agent_key))
        } else {
            self.manager
                .deploy_skill_to_project(skill_id, project_id, agent_key, None, false)
                .map(|dep| {
                    self.lang
                        .toast_project_deployed(&project_name, &dep.agent_key, dep.mode)
                })
        };

        match result {
            Ok(msg) => {
                self.reload_all(cx);
                self.notify_toast(false, msg, window, cx);
            }
            Err(e) => {
                let err_str = format!("{e}");
                if !currently_deployed && err_str.contains("TARGET_CONFLICT") {
                    let agent_name = self
                        .agents
                        .iter()
                        .find(|a| a.key == agent_key)
                        .map(|a| a.display_name.clone())
                        .unwrap_or_else(|| agent_key.to_string());
                    let target_path = self
                        .projects
                        .iter()
                        .find(|p| p.id == project_id)
                        .and_then(|p| {
                            self.agents
                                .iter()
                                .find(|a| a.key == agent_key)
                                .and_then(|a| {
                                    self.skills.iter().find(|s| s.id == skill_id).map(|s| {
                                        p.path
                                            .join(&a.project_relative_skills_dir)
                                            .join(&s.slug)
                                            .to_string_lossy()
                                            .to_string()
                                    })
                                })
                        })
                        .unwrap_or_else(|| err_str.clone());
                    let ws_entity = cx.entity().clone();
                    let skill_id_owned = skill_id.to_string();
                    let project_id_owned = project_id.to_string();
                    let agent_key_owned = agent_key.to_string();
                    let s = self.t();
                    let title = s.confirm_conflict_title;
                    let desc = self.lang.confirm_conflict_desc(&agent_name, &target_path);
                    let ok_label = s.btn_replace;
                    let cancel_label = s.btn_cancel;

                    window.open_alert_dialog(cx, move |alert, _, _| {
                        let ws_entity = ws_entity.clone();
                        let skill_id_owned = skill_id_owned.clone();
                        let project_id_owned = project_id_owned.clone();
                        let agent_key_owned = agent_key_owned.clone();
                        alert
                            .confirm()
                            .centered()
                            .title(title)
                            .description(desc.clone())
                            .ok_text(ok_label)
                            .ok_variant(ButtonVariant::Danger)
                            .cancel_text(cancel_label)
                            .on_ok(move |_, window, cx| {
                                ws_entity.update(cx, |ws, cx| {
                                    ws.force_deploy_skill_to_project(
                                        &skill_id_owned,
                                        &project_id_owned,
                                        &agent_key_owned,
                                        window,
                                        cx,
                                    );
                                });
                                true
                            })
                    });
                } else {
                    self.notify_toast(true, err_str, window, cx);
                }
            }
        }
    }

    pub fn force_deploy_skill_to_project(
        &mut self,
        skill_id: &str,
        project_id: &str,
        agent_key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project_name = self
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| project_id.to_string());
        match self
            .manager
            .deploy_skill_to_project(skill_id, project_id, agent_key, None, true)
        {
            Ok(dep) => {
                self.reload_all(cx);
                let msg = self
                    .lang
                    .toast_project_deployed(&project_name, &dep.agent_key, dep.mode);
                self.notify_toast(false, msg, window, cx);
            }
            Err(e) => self.notify_toast(true, format!("{e}"), window, cx),
        }
    }

    pub fn remove_skill(&mut self, skill_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match self.manager.remove_skill(skill_id) {
            Ok(()) => {
                self.reload_all(cx);
                let msg = self.t().toast_removed_skill;
                self.notify_toast(false, msg, window, cx);
            }
            Err(e) => self.notify_toast(true, format!("{e}"), window, cx),
        }
    }

    pub fn is_source_installing(&self, source_ref: &str) -> bool {
        self.installing_refs.contains(source_ref.trim())
    }

    pub fn install_source_async(
        &mut self,
        source_ref: &str,
        navigate_to_library: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let trimmed = source_ref.trim().to_string();
        if trimmed.is_empty() {
            let msg = self.t().toast_enter_source;
            self.notify_toast(true, msg, window, cx);
            return false;
        }
        if !self.installing_refs.insert(trimmed.clone()) {
            return false;
        }
        cx.emit(WorkspaceEvent::InstallStateChanged);
        let busy = self.lang.busy_installing(&trimmed);
        let busy_token = self.begin_busy(busy, cx);

        let mgr = self.manager.clone();
        let bg = cx.background_executor().clone();
        let finished_ref = trimmed.clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg
                .spawn(async move { mgr.install_from_ref(&trimmed) })
                .await;
            let _ = this.update_in(cx, |state, window, cx| {
                state.end_busy(busy_token, cx);
                state.installing_refs.remove(&finished_ref);
                cx.emit(WorkspaceEvent::InstallStateChanged);
                match res {
                    Ok(skill) => {
                        state.note_skill_added(&skill.id);
                        state.reload_all(cx);
                        if navigate_to_library {
                            state.navigate(NavTab::Library, cx);
                        }
                        let msg = state
                            .lang
                            .toast_installed(&skill.name, skill.estimated_tokens);
                        state.notify_toast(false, msg, window, cx);
                    }
                    Err(e) => state.notify_toast(true, format!("{e}"), window, cx),
                }
            });
        })
        .detach();
        true
    }

    /// Adopt the confirmed selection on the background executor.
    pub fn adopt_candidates_async(
        &mut self,
        candidates: Vec<AdoptCandidate>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.adopting_all || candidates.is_empty() {
            return;
        }
        self.adopting_all = true;
        let busy = self.lang.busy_adopting_all(candidates.len());
        let busy_token = self.begin_busy(busy, cx);
        cx.emit(WorkspaceEvent::SettingsChanged);

        let mgr = self.manager.clone();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let (ok_count, failures) = bg
                .spawn(async move {
                    let mut ok_count = 0;
                    let mut failures = Vec::new();
                    for candidate in &candidates {
                        match mgr.adopt_candidate(candidate) {
                            Ok(_) => ok_count += 1,
                            Err(error) => failures.push(format!("{} · {error}", candidate.name)),
                        }
                    }
                    (ok_count, failures)
                })
                .await;

            let _ = this.update_in(cx, |state, window, cx| {
                state.adopting_all = false;
                state.end_busy(busy_token, cx);
                state.reload_all(cx);
                let mut msg = state.lang.toast_adopted_all(ok_count, failures.len());
                if let Some(detail) = failures.first() {
                    msg.push('\n');
                    msg.push_str(detail);
                }
                state.notify_toast(!failures.is_empty(), msg, window, cx);
            });
        })
        .detach();
    }

    pub fn is_stack_installing(&self, stack_id: &str) -> bool {
        self.installing_stack_ids.contains(stack_id)
    }

    pub fn stack_install_failures(&self, stack_id: &str) -> &[(String, String)] {
        self.stack_install_failures
            .get(stack_id)
            .map_or(&[], Vec::as_slice)
    }

    pub fn install_stack_skill_async(
        &mut self,
        stack_id: &str,
        skill: &StackSkillRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if skill.installed_in(&self.skills).is_none()
            && self.install_source_async(&skill.source, false, window, cx)
            && let Some(failures) = self.stack_install_failures.get_mut(stack_id)
        {
            failures.retain(|(name, _)| name != &skill.name);
        }
    }

    pub fn install_stack_async(
        &mut self,
        stack: StackManifest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_stack_installing(&stack.id)
            || stack
                .skills
                .iter()
                .any(|skill| self.is_source_installing(&skill.source))
        {
            return;
        }
        let stack_id = stack.id.clone();
        self.installing_stack_ids.insert(stack_id.clone());
        self.stack_install_failures.remove(&stack_id);
        let sources: Vec<_> = stack
            .skills
            .iter()
            .map(|skill| skill.source.trim().to_owned())
            .collect();
        self.installing_refs.extend(sources.iter().cloned());
        cx.emit(WorkspaceEvent::InstallStateChanged);

        let stack_name = stack.name.clone();
        let busy = self.lang.busy_installing_stack(&stack_name);
        let busy_token = self.begin_busy(busy, cx);

        let mgr = self.manager.clone();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg
                .spawn(async move { mgr.install_stack(&stack, &[]) })
                .await;
            let _ = this.update_in(cx, |state, window, cx| {
                state.end_busy(busy_token, cx);
                state.installing_stack_ids.remove(&stack_id);
                for source in sources {
                    state.installing_refs.remove(&source);
                }
                cx.emit(WorkspaceEvent::InstallStateChanged);
                match res {
                    Ok(summary) => {
                        state.reload_all(cx);
                        let msg = state.lang.toast_stack_installed(
                            &stack_name,
                            summary.installed.len(),
                            summary.skipped_existing.len(),
                            summary.failed.len(),
                        );
                        let has_failures = !summary.failed.is_empty();
                        if has_failures {
                            state
                                .stack_install_failures
                                .insert(stack_id.clone(), summary.failed);
                        }
                        state.notify_toast(has_failures, msg, window, cx);
                    }
                    Err(e) => state.notify_toast(true, format!("{e}"), window, cx),
                }
            });
        })
        .detach();
    }

    pub fn copy_stack_toml(
        &mut self,
        stack: &StackManifest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.manager.export_stack_toml(stack) {
            Ok(toml) => {
                cx.write_to_clipboard(ClipboardItem::new_string(toml));
                let msg = self.lang.toast_stack_toml_copied(&stack.id);
                self.notify_toast(false, msg, window, cx);
            }
            Err(e) => self.notify_toast(true, format!("{e}"), window, cx),
        }
    }

    /// Fetches the remote community stacks index (ETag-cached, offline-safe)
    /// on a background task and refreshes the snapshot. `silent` skips the
    /// busy spinner and error toast (used for the startup check).
    pub fn refresh_community_stacks(
        &mut self,
        silent: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let busy_token = if silent {
            0
        } else {
            self.begin_busy(self.t().busy_refreshing_community, cx)
        };
        let mgr = self.manager.clone();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg.spawn(async move { mgr.community_stacks() }).await;
            let _ = this.update_in(cx, |state, _window, cx| {
                if !silent {
                    state.end_busy(busy_token, cx);
                }
                match res {
                    Ok(stacks) => {
                        state.community_stacks = stacks;
                        cx.emit(WorkspaceEvent::Reloaded);
                        cx.notify();
                    }
                    Err(e) if !silent => {
                        let msg = format!("{}: {e}", state.t().toast_community_failed);
                        state.notify_toast(true, msg, _window, cx);
                    }
                    // Startup path: offline failures are silently ignored —
                    // the Stacks view falls back to builtin + local stacks.
                    Err(_) => {}
                }
            });
        })
        .detach();
    }

    /// Publishes a stack as a secret GitHub Gist via the stored PAT and toasts
    /// the returned URL. All network work stays on the background executor.
    pub fn share_stack_to_gist(
        &mut self,
        stack: StackManifest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let stack_name = stack.name.clone();
        let busy_token = self.begin_busy(self.lang.busy_sharing_stack(&stack_name), cx);
        let mgr = self.manager.clone();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg
                .spawn(async move { mgr.share_stack_to_gist(&stack) })
                .await;
            let _ = this.update_in(cx, |state, window, cx| {
                state.end_busy(busy_token, cx);
                match res {
                    Ok(url) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
                        let msg = state.lang.toast_gist_shared(&url);
                        state.notify_toast(false, msg, window, cx);
                    }
                    Err(e) => state.notify_toast(true, format!("{e}"), window, cx),
                }
            });
        })
        .detach();
    }

    pub fn save_git_backup_config(
        &mut self,
        repo_url: &str,
        branch: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.manager.configure_git_backup(repo_url, Some(branch)) {
            Ok(()) => {
                cx.emit(WorkspaceEvent::SettingsChanged);
                let msg = self.t().toast_backup_saved;
                self.notify_toast(false, msg, window, cx);
            }
            Err(e) => self.notify_toast(true, format!("{e}"), window, cx),
        }
    }

    pub fn git_backup_push_async(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.backup_busy {
            return;
        }
        self.backup_busy = true;
        let busy_token = self.begin_busy(self.t().busy_backup_pushing, cx);
        cx.emit(WorkspaceEvent::SettingsChanged);

        let mgr = self.manager.clone();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg.spawn(async move { mgr.git_backup_push() }).await;
            let _ = this.update_in(cx, |state, window, cx| {
                state.backup_busy = false;
                state.end_busy(busy_token, cx);
                cx.emit(WorkspaceEvent::SettingsChanged);
                match res {
                    Ok(outcome) => {
                        let msg = if outcome.pushed {
                            state.lang.toast_backup_pushed(
                                outcome.skills_exported,
                                outcome.stacks_exported,
                                outcome.commit_sha.as_deref().unwrap_or("-"),
                            )
                        } else {
                            state.t().toast_backup_noop.to_string()
                        };
                        state.notify_toast(false, msg, window, cx);
                    }
                    Err(e) => state.notify_toast(true, format!("{e}"), window, cx),
                }
            });
        })
        .detach();
    }

    pub fn git_backup_pull_async(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.backup_busy {
            return;
        }
        self.backup_busy = true;
        let busy_token = self.begin_busy(self.t().busy_backup_pulling, cx);
        cx.emit(WorkspaceEvent::SettingsChanged);

        let mgr = self.manager.clone();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg.spawn(async move { mgr.git_backup_pull() }).await;
            let _ = this.update_in(cx, |state, window, cx| {
                state.backup_busy = false;
                state.end_busy(busy_token, cx);
                match res {
                    Ok(outcome) => {
                        state.reload_all(cx);
                        let msg = state.lang.toast_backup_pulled(
                            outcome.skills_imported,
                            outcome.skills_updated,
                            outcome.stacks_imported,
                            outcome.refreshed_copies,
                        );
                        state.notify_toast(false, msg, window, cx);
                    }
                    Err(e) => {
                        cx.emit(WorkspaceEvent::SettingsChanged);
                        state.notify_toast(true, format!("{e}"), window, cx);
                    }
                }
            });
        })
        .detach();
    }

    /// Validates the form, persists a new local stack, reloads snapshots, and
    /// toasts. Returns the created manifest on success.
    pub fn create_local_stack(
        &mut self,
        name: &str,
        author: &str,
        description: &str,
        skill_ids: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<StackManifest> {
        if name.trim().is_empty() {
            let msg = self.t().toast_enter_stack_name;
            self.notify_toast(true, msg, window, cx);
            return None;
        }
        if skill_ids.is_empty() {
            let msg = self.t().toast_select_stack_skill;
            self.notify_toast(true, msg, window, cx);
            return None;
        }
        let author = if author.trim().is_empty() {
            "me"
        } else {
            author.trim()
        };
        match self.manager.create_stack_from_skills(
            name.trim(),
            author,
            description.trim(),
            vec![],
            skill_ids,
        ) {
            Ok(stack) => {
                self.reload_all(cx);
                let msg = self
                    .lang
                    .toast_stack_created(&stack.name, stack.skills.len());
                self.notify_toast(false, msg, window, cx);
                Some(stack)
            }
            Err(e) => {
                self.notify_toast(true, format!("{e}"), window, cx);
                None
            }
        }
    }

    pub fn import_stack_toml(
        &mut self,
        raw: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<StackManifest> {
        if raw.trim().is_empty() {
            let msg = self.t().toast_paste_toml;
            self.notify_toast(true, msg, window, cx);
            return None;
        }
        match self.manager.import_stack_toml(raw) {
            Ok(stack) => {
                self.reload_all(cx);
                let msg = self.lang.toast_stack_imported(&stack.name);
                self.notify_toast(false, msg, window, cx);
                Some(stack)
            }
            Err(e) => {
                self.notify_toast(true, format!("{e}"), window, cx);
                None
            }
        }
    }

    pub fn delete_local_stack(
        &mut self,
        stack_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.manager.delete_local_stack(stack_id);
        self.reload_all(cx);
        let msg = self.t().toast_deleted_stack;
        self.notify_toast(false, msg, window, cx);
    }

    pub fn set_agent_enabled(&mut self, agent_key: &str, enabled: bool, cx: &mut Context<Self>) {
        let _ = self.manager.set_agent_enabled(agent_key, enabled);
        self.agents = self.manager.list_agents().unwrap_or_default();
        cx.emit(WorkspaceEvent::SettingsChanged);
    }

    pub fn set_deploy_mode(
        &mut self,
        mode: DeployMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.manager.set_default_deploy_mode(mode);
        self.deploy_mode = mode;
        cx.emit(WorkspaceEvent::SettingsChanged);
        let msg = self.lang.toast_deploy_mode(mode);
        self.notify_toast(false, msg, window, cx);
    }

    pub fn save_network_settings(
        &mut self,
        proxy: &str,
        token: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.manager.set_http_proxy(Some(proxy));
        let _ = self.manager.set_github_token(Some(token));
        cx.emit(WorkspaceEvent::SettingsChanged);
        let msg = self.t().toast_network_saved;
        self.notify_toast(false, msg, window, cx);
    }

    pub fn add_custom_agent(
        &mut self,
        name: &str,
        skills_dir: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let name = name.trim();
        let skills_dir = skills_dir.trim();
        if name.is_empty() || skills_dir.is_empty() {
            let msg = self.t().toast_custom_agent_required;
            self.notify_toast(true, msg, window, cx);
            return false;
        }
        let slug = unoskills_core::parser::sanitize_slug(name)
            .unwrap_or_else(|| "custom".to_string())
            .to_lowercase()
            .replace([' ', '-'], "_");
        let key = format!("custom_{slug}");
        let rec = CustomAgentRecord {
            key,
            display_name: name.to_string(),
            skills_dir: skills_dir.to_string(),
            category: AgentCategory::Coding,
        };
        match self.manager.add_custom_agent(&rec) {
            Ok(()) => {
                self.reload_all(cx);
                let msg = self.lang.toast_custom_agent_added(name);
                self.notify_toast(false, msg, window, cx);
                true
            }
            Err(e) => {
                self.notify_toast(true, format!("{e}"), window, cx);
                false
            }
        }
    }

    pub fn remove_custom_agent(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        match self.manager.remove_custom_agent(key) {
            Ok(()) => {
                self.reload_all(cx);
                let msg = self.t().toast_custom_agent_removed;
                self.notify_toast(false, msg, window, cx);
            }
            Err(e) => self.notify_toast(true, format!("{e}"), window, cx),
        }
    }
}

pub fn platform_install_button_label(s: &'static Strings) -> &'static str {
    match unoskills_core::updater::PlatformKind::current() {
        unoskills_core::updater::PlatformKind::WindowsX64 => s.btn_install_update_win,
        unoskills_core::updater::PlatformKind::MacosArm64 => s.btn_install_update_mac,
        unoskills_core::updater::PlatformKind::LinuxX64
        | unoskills_core::updater::PlatformKind::Other => s.btn_install_update_linux,
    }
}

#[cfg(test)]
mod tests {
    use unoskills_core::updater::is_newer_semver;

    #[test]
    fn compares_semver_versions_correctly() {
        assert!(is_newer_semver("v0.2.0", "0.1.0"));
        assert!(is_newer_semver("1.0.0", "0.9.9"));
        assert!(is_newer_semver("v0.1.1", "0.1.0"));
        assert!(is_newer_semver("v0.2.0", "0.2.0-rc.1"));
        assert!(!is_newer_semver("v0.1.0", "0.1.0"));
        assert!(!is_newer_semver("v0.1.0", "0.2.0"));
        assert!(!is_newer_semver("v0.2.0-beta.1", "0.2.0"));
        assert!(!is_newer_semver("invalid", "0.1.0"));
    }
}
