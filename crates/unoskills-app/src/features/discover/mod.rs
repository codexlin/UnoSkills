mod adopt;
mod marketplace;

use adopt::{AdoptGroup, AdoptRow};

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::PathBuf,
};

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::actions::Confirm;
use gpui_kit::component::{
    Disableable, IconName, IndexPath, WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    searchable_list::{SearchableListItem, SearchableVec},
    select::{SelectEvent, SelectState},
    v_flex,
};
use gpui_kit::{
    AppContext, Context, Entity, Focusable, IntoElement, ParentElement, PathPromptOptions, Render,
    ScrollStrategy, SharedString, Styled, Subscription, UniformListScrollHandle, Window, div, px,
};
use unoskills_core::MarketplaceSkill;

use crate::components::page_header::{PageHeader, page_tabs};
use crate::state::{MarketBoard, MarketFirstPage, NavTab, WorkspaceEvent, WorkspaceState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DiscoverSubTab {
    Marketplace,
    Adopt,
}

#[derive(Debug, Clone)]
pub(super) struct CachedMarketBoard {
    pub items: Vec<MarketplaceSkill>,
    pub next_page: usize,
    pub total: Option<usize>,
    pub has_more: bool,
}

struct MarketSourceFilter {
    source: String,
    indices: Vec<usize>,
}

#[derive(Clone)]
struct MarketSourceChoice {
    label: SharedString,
    source: Option<String>,
}

impl SearchableListItem for MarketSourceChoice {
    type Value = Option<String>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.source
    }
}

fn market_source_choices(
    results: &[MarketplaceSkill],
    all_sources: &'static str,
) -> SearchableVec<MarketSourceChoice> {
    let sources: BTreeSet<_> = results
        .iter()
        .map(|item| &item.source)
        .filter(|source| !source.is_empty())
        .collect();
    let all = MarketSourceChoice {
        label: all_sources.into(),
        source: None,
    };
    let choices =
        std::iter::once(all).chain(sources.into_iter().map(|source| MarketSourceChoice {
            label: source.as_str().into(),
            source: Some(source.clone()),
        }));
    SearchableVec::new(choices.collect::<Vec<_>>())
}

/// Discover page: Quick Install, the skills.sh Marketplace (native JSON
/// pagination with per-board caching, infinite scroll, and request-sequence
/// guards), and Adopt for unmanaged local skills.
pub struct DiscoverView {
    pub(super) workspace: Entity<WorkspaceState>,
    pub(super) sub_tab: DiscoverSubTab,
    pub(super) market_board: MarketBoard,
    pub(super) market_board_cache: HashMap<MarketBoard, CachedMarketBoard>,
    pub(super) marketplace_results: Vec<MarketplaceSkill>,
    pub(super) market_next_page: usize,
    pub(super) market_total: Option<usize>,
    pub(super) market_has_more: bool,
    pub(super) market_loading: bool,
    pub(super) market_loading_more: bool,
    pub(super) market_load_more_failed: bool,
    pub(super) market_search_active: bool,
    market_search_origin: Option<(MarketBoard, usize)>,
    market_error: Option<String>,
    market_source_filter: Option<MarketSourceFilter>,
    market_source_select: Entity<SelectState<SearchableVec<MarketSourceChoice>>>,
    /// Monotonic counter guarding against out-of-order async leaderboard/search replies.
    pub(super) market_req_seq: u64,
    pub(super) market_scroll_handle: UniformListScrollHandle,
    pub(super) adopt_scroll_handle: UniformListScrollHandle,
    adopt_groups: Vec<AdoptGroup>,
    adopt_rows: Vec<AdoptRow>,
    pub(super) selected_adopt_paths: HashSet<PathBuf>,
    pub(super) adopt_expansion: HashMap<String, bool>,
    pub(super) install_source_input: Entity<InputState>,
    pub(super) market_search_input: Entity<InputState>,
    pub(super) adopt_search_input: Entity<InputState>,
    _subscriptions: Subscription,
}

impl DiscoverView {
    pub fn new(
        workspace: Entity<WorkspaceState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let s = workspace.read(cx).t();
        let install_source_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_install_source));
        let market_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_market_search));
        let adopt_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_adopt_search));
        let market_source_select = cx.new(|cx| {
            SelectState::new(
                market_source_choices(&[], s.market_all_sources),
                Some(IndexPath::default()),
                window,
                cx,
            )
            .searchable(true)
        });
        let source_sub = cx.subscribe_in(
            &market_source_select,
            window,
            |this, _, event: &SelectEvent<_>, window, cx| {
                if let SelectEvent::Confirm(Some(source)) = event {
                    this.set_market_source_filter(source.clone(), window, cx);
                }
            },
        );

        let market_sub = cx.subscribe_in(
            &market_search_input,
            window,
            |this, _input, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    let query = this.market_search_input.read(cx).value().trim().to_string();
                    if query.is_empty() {
                        let board = this.market_board;
                        this.switch_market_board(board, false, window, cx);
                    } else {
                        this.search_marketplace_async(query, window, cx);
                    }
                }
                InputEvent::Change => {
                    let cleared = this.market_search_active
                        && this.market_search_input.read(cx).value().trim().is_empty();
                    if cleared {
                        let board = this.market_board;
                        this.switch_market_board(board, false, window, cx);
                    }
                }
                _ => {}
            },
        );
        let adopt_sub = cx.subscribe_in(
            &adopt_search_input,
            window,
            |this, _input, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.refilter_adopt(cx);
                    this.adopt_scroll_handle
                        .scroll_to_item(0, ScrollStrategy::Top);
                    cx.notify();
                }
            },
        );
        let workspace_sub = cx.subscribe_in(
            &workspace,
            window,
            |this, _ws, event: &WorkspaceEvent, window, cx| match event {
                WorkspaceEvent::NavigateTo(NavTab::Discover)
                    if this.sub_tab == DiscoverSubTab::Marketplace =>
                {
                    this.ensure_market_loaded(window, cx)
                }
                WorkspaceEvent::OpenLocalSkills => {
                    this.sub_tab = DiscoverSubTab::Adopt;
                    cx.notify();
                }
                WorkspaceEvent::OpenMarketBoard(board) => {
                    this.sub_tab = DiscoverSubTab::Marketplace;
                    this.switch_market_board(*board, false, window, cx);
                    if this.market_source_filter.is_some() {
                        this.set_market_source_filter(None, window, cx);
                    }
                    cx.notify();
                }
                WorkspaceEvent::MarketFirstPageChanged(board) => {
                    this.apply_market_first_page(*board, window, cx);
                }
                WorkspaceEvent::OpenInstallDialog => this.open_install_dialog(window, cx),
                WorkspaceEvent::LanguageChanged => this.apply_language(window, cx),
                WorkspaceEvent::InstallStateChanged | WorkspaceEvent::SettingsChanged => {
                    cx.notify()
                }
                WorkspaceEvent::Reloaded => {
                    this.refilter_adopt(cx);
                    cx.notify();
                }
                _ => {}
            },
        );

        let mut view = Self {
            workspace,
            sub_tab: DiscoverSubTab::Marketplace,
            market_board: MarketBoard::Trending,
            market_board_cache: HashMap::new(),
            marketplace_results: Vec::new(),
            market_next_page: 0,
            market_total: None,
            market_has_more: false,
            market_loading: false,
            market_loading_more: false,
            market_load_more_failed: false,
            market_search_active: false,
            market_search_origin: None,
            market_error: None,
            market_source_filter: None,
            market_source_select,
            market_req_seq: 0,
            market_scroll_handle: UniformListScrollHandle::new(),
            adopt_scroll_handle: UniformListScrollHandle::new(),
            adopt_groups: Vec::new(),
            adopt_rows: Vec::new(),
            selected_adopt_paths: HashSet::new(),
            adopt_expansion: HashMap::new(),
            install_source_input,
            market_search_input,
            adopt_search_input,
            _subscriptions: Subscription::join(
                market_sub,
                Subscription::join(source_sub, Subscription::join(adopt_sub, workspace_sub)),
            ),
        };
        view.refilter_adopt(cx);
        view
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = match self.sub_tab {
            DiscoverSubTab::Marketplace => &self.market_search_input,
            DiscoverSubTab::Adopt => &self.adopt_search_input,
        };
        let focus = input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    }

    fn apply_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.workspace.read(cx).t();
        let ph_install = s.ph_install_source;
        let ph_market = s.ph_market_search;
        let ph_adopt = s.ph_adopt_search;
        self.install_source_input.update(cx, |state, cx| {
            state.set_placeholder(ph_install, window, cx)
        });
        self.market_search_input
            .update(cx, |state, cx| state.set_placeholder(ph_market, window, cx));
        self.adopt_search_input
            .update(cx, |state, cx| state.set_placeholder(ph_adopt, window, cx));
        self.refresh_market_source_choices(window, cx);
        cx.notify();
    }

    pub(super) fn ensure_market_loaded(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.market_loading
            && !self.market_search_active
            && !self.market_board_cache.contains_key(&self.market_board)
        {
            let board = self.market_board;
            self.switch_market_board(board, false, window, cx);
        } else {
            cx.notify();
        }
    }

    pub(super) fn open_install_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let input = self.install_source_input.clone();
        let workspace = self.workspace.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let ws = workspace.read(cx);
            let s = ws.t();
            let value = input.read(cx).value();
            let pending = ws.is_source_installing(&value);
            let browse_view = view.clone();
            let submit_input = input.clone();
            let submit_workspace = workspace.clone();
            dialog
                .title(s.quick_install_title)
                .w(px(480.))
                .child(Input::new(&input).disabled(pending))
                .on_ok(move |_, window, cx| {
                    let source = submit_input.read(cx).value();
                    submit_workspace.update(cx, |ws, cx| {
                        ws.install_source_async(&source, true, window, cx)
                    })
                })
                .footer(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .justify_between()
                        .child(
                            Button::new("browse-local-skill-btn")
                                .outline()
                                .icon(IconName::FolderOpen)
                                .label(s.btn_browse_folder)
                                .disabled(pending)
                                .on_click(move |_, window, cx| {
                                    let _ = browse_view.update(cx, |view, cx| {
                                        view.browse_install_folder(window, cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("direct-install-btn")
                                .primary()
                                .label(s.btn_install)
                                .loading(pending)
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(
                                        Box::new(Confirm { secondary: false }),
                                        cx,
                                    );
                                }),
                        ),
                )
        });
        self.install_source_input
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx);
    }

    fn browse_install_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.workspace.read(cx).t();
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(s.prompt_pick_skill_dir.into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = selection.await;
            let _ = this.update_in(cx, |view, window, cx| match result {
                Ok(Ok(Some(paths))) => {
                    if let Some(path) = paths.into_iter().next() {
                        let source = path.to_string_lossy().into_owned();
                        view.install_source_input.update(cx, |input, cx| {
                            input.set_value(source, window, cx);
                        });
                        view.install_source_input
                            .read(cx)
                            .focus_handle(cx)
                            .focus(window, cx);
                        window.dispatch_action(Box::new(Confirm { secondary: false }), cx);
                    }
                }
                Ok(Err(error)) => view.workspace.update(cx, |ws, cx| {
                    ws.notify_toast(true, error.to_string(), window, cx);
                }),
                _ => {}
            });
        })
        .detach();
    }

    fn reset_market_list(&self) {
        self.market_scroll_handle
            .scroll_to_item(0, ScrollStrategy::Top);
    }

    fn set_market_source_filter(
        &mut self,
        source: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.market_source_filter = source.map(|source| {
            let indices = self.market_source_indices(&source);
            MarketSourceFilter { source, indices }
        });
        self.sync_market_source_selection(window, cx);
        self.reset_market_list();
        cx.notify();
    }

    fn market_source_indices(&self, source: &str) -> Vec<usize> {
        self.marketplace_results
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (item.source == source).then_some(index))
            .collect()
    }

    fn sync_market_source_selection(&self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self
            .market_source_filter
            .as_ref()
            .map(|filter| filter.source.clone());
        self.market_source_select.update(cx, |state, cx| {
            if state.selected_value() != Some(&selected) {
                state.set_selected_value(&selected, window, cx);
            }
        });
    }

    fn refresh_market_source_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(mut filter) = self.market_source_filter.take() {
            filter.indices = self.market_source_indices(&filter.source);
            self.market_source_filter = (!filter.indices.is_empty()).then_some(filter);
        }
        let choices = market_source_choices(
            &self.marketplace_results,
            self.workspace.read(cx).t().market_all_sources,
        );
        self.market_source_select
            .update(cx, |state, cx| state.set_items(choices, window, cx));
        self.sync_market_source_selection(window, cx);
    }

    fn market_result_count(&self) -> usize {
        self.market_source_filter
            .as_ref()
            .map_or(self.marketplace_results.len(), |filter| {
                filter.indices.len()
            })
    }

    fn market_result(&self, index: usize) -> Option<(usize, &MarketplaceSkill)> {
        let index = match &self.market_source_filter {
            Some(filter) => *filter.indices.get(index)?,
            None => index,
        };
        Some((index, self.marketplace_results.get(index)?))
    }

    pub(super) fn switch_market_board(
        &mut self,
        board: MarketBoard,
        force_refresh: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keep_results = self.market_board == board && !self.market_search_active;
        if !force_refresh
            && keep_results
            && (self.market_loading || self.market_board_cache.contains_key(&board))
        {
            return;
        }

        let resumed_row = self
            .market_search_origin
            .take()
            .filter(|(origin, _)| *origin == board)
            .map(|(_, row)| row);
        self.market_error = None;
        self.market_board = board;
        self.market_search_active = false;
        self.market_source_filter = None;
        self.market_loading_more = false;
        self.market_load_more_failed = false;
        self.market_req_seq = self.market_req_seq.wrapping_add(1);
        if !force_refresh && let Some(cached) = self.market_board_cache.get(&board) {
            self.market_loading = false;
            self.marketplace_results = cached.items.clone();
            self.market_next_page = cached.next_page;
            self.market_total = cached.total;
            self.market_has_more = cached.has_more;
            self.refresh_market_source_choices(window, cx);
            self.market_scroll_handle
                .scroll_to_item(resumed_row.unwrap_or(0), ScrollStrategy::Top);
            cx.notify();
            return;
        }

        self.market_loading = true;
        if !keep_results {
            self.marketplace_results.clear();
            self.market_total = None;
            self.market_has_more = false;
        }
        self.refresh_market_source_choices(window, cx);
        self.workspace.update(cx, |ws, cx| {
            ws.load_market_first_page(board, force_refresh, window, cx);
        });
        self.apply_market_first_page(board, window, cx);
        cx.notify();
    }

    fn apply_market_first_page(
        &mut self,
        board: MarketBoard,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = self.workspace.read(cx).market_first_page(board);
        if !self.market_loading || self.market_search_active || self.market_board != board {
            // A refresh from Home invalidates this view's paginated cache.
            if matches!(page, Some(MarketFirstPage::Loaded(_))) {
                self.market_board_cache.remove(&board);
            }
            return;
        }

        match page {
            Some(MarketFirstPage::Loaded(page)) => {
                self.market_loading = false;
                self.marketplace_results = page.items.clone();
                self.market_next_page = page.page + 1;
                self.market_total = page.total;
                self.market_has_more = page.has_more;
                self.market_board_cache.insert(
                    board,
                    CachedMarketBoard {
                        items: self.marketplace_results.clone(),
                        next_page: self.market_next_page,
                        total: self.market_total,
                        has_more: self.market_has_more,
                    },
                );
                self.refresh_market_source_choices(window, cx);
                self.reset_market_list();
            }
            Some(MarketFirstPage::Failed(error)) => {
                self.market_loading = false;
                let error = error.clone();
                self.market_error = Some(error.clone());
                self.workspace
                    .update(cx, |ws, cx| ws.notify_toast(true, error, window, cx));
            }
            _ => return,
        }
        cx.notify();
    }

    /// Fetches the next page for the active leaderboard and appends deduplicated
    /// skills to both the live view and the per-board cache.
    pub(super) fn fetch_next_market_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.market_search_active
            || self.market_source_filter.is_some()
            || !self.market_has_more
            || self.market_loading
        {
            self.market_loading_more = false;
            return;
        }
        self.market_loading_more = true;
        self.market_load_more_failed = false;

        let board = self.market_board;
        let next_page = self.market_next_page;
        let req_seq = self.market_req_seq;
        let mgr = self.workspace.read(cx).manager().clone();
        let api_board = board.as_api_str();
        let bg = cx.background_executor().clone();
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let res = bg
                .spawn(async move { mgr.marketplace_leaderboard_page(api_board, next_page) })
                .await;

            let _ = this.update_in(cx, |view, window, cx| {
                if view.market_req_seq != req_seq || view.market_board != board {
                    return;
                }
                view.market_loading_more = false;
                match res {
                    Ok(page_data) => {
                        let existing_ids: HashSet<String> = view
                            .marketplace_results
                            .iter()
                            .map(|item| item.id.clone())
                            .collect();
                        let mut appended = 0usize;
                        for item in page_data.items {
                            if !existing_ids.contains(&item.id) {
                                view.marketplace_results.push(item);
                                appended += 1;
                            }
                        }
                        view.market_next_page = page_data.page + 1;
                        if page_data.total.is_some() {
                            view.market_total = page_data.total;
                        }
                        // Stop auto-paging if upstream returned no new unique items.
                        view.market_has_more = page_data.has_more && appended > 0;
                        view.refresh_market_source_choices(window, cx);
                        view.market_board_cache.insert(
                            board,
                            CachedMarketBoard {
                                items: view.marketplace_results.clone(),
                                next_page: view.market_next_page,
                                total: view.market_total,
                                has_more: view.market_has_more,
                            },
                        );
                        cx.notify();
                    }
                    Err(e) => {
                        view.market_load_more_failed = true;
                        view.workspace.update(cx, |ws, cx| {
                            ws.notify_toast(true, format!("{e}"), window, cx)
                        });
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(super) fn search_marketplace_async(
        &mut self,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let q = query.trim().to_string();
        if q.is_empty() {
            let board = self.market_board;
            self.switch_market_board(board, false, window, cx);
            return;
        }

        if !self.market_search_active
            && self.market_source_filter.is_none()
            && !self.marketplace_results.is_empty()
        {
            let handle = self.market_scroll_handle.0.borrow();
            if let Some(size) = handle.last_item_size
                && size.item.height > px(0.)
            {
                let row = (-handle.base_handle.offset().y / size.item.height).max(0.) as usize;
                self.market_search_origin = Some((self.market_board, row));
            }
        }
        self.market_error = None;
        self.market_search_active = true;
        self.market_source_filter = None;
        self.marketplace_results.clear();
        self.refresh_market_source_choices(window, cx);
        self.market_loading = true;
        self.market_loading_more = false;
        self.market_load_more_failed = false;
        self.market_has_more = false;
        self.market_total = None;
        self.market_req_seq = self.market_req_seq.wrapping_add(1);
        let req_seq = self.market_req_seq;

        let busy_token = self.workspace.update(cx, |ws, cx| {
            let label = ws.lang().busy_searching(&q);
            ws.begin_busy(label, cx)
        });
        cx.notify();

        let mgr = self.workspace.read(cx).manager().clone();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg
                .spawn(async move { mgr.marketplace_search(&q, 60) })
                .await;

            let _ = this.update_in(cx, |view, window, cx| {
                view.workspace
                    .update(cx, |ws, cx| ws.end_busy(busy_token, cx));
                if view.market_req_seq != req_seq {
                    return;
                }
                view.market_loading = false;
                match res {
                    Ok(items) => {
                        view.marketplace_results = items;
                        view.refresh_market_source_choices(window, cx);
                        view.reset_market_list();
                        cx.notify();
                    }
                    Err(e) => {
                        view.market_error = Some(e.to_string());
                        view.workspace.update(cx, |ws, cx| {
                            ws.notify_toast(true, format!("{e}"), window, cx)
                        });
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}

impl Render for DiscoverView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let adopt_tab = ws
            .lang()
            .label_with_count(s.discover_adopt_tab, ws.adopt_candidates().len());
        let selected_idx = match self.sub_tab {
            DiscoverSubTab::Marketplace => 0,
            DiscoverSubTab::Adopt => 1,
        };

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                PageHeader::new(s.discover_title).right_slot(
                    Button::new("open-install-dialog-btn")
                        .outline()
                        .icon(IconName::Plus)
                        .label(s.btn_install_from_link)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_install_dialog(window, cx)),
                        ),
                ),
            )
            .child(
                page_tabs(
                    "discover-sub-tabs",
                    [
                        (AssetIcon::Store, s.discover_market_tab.to_string()),
                        (AssetIcon::ArchiveRestore, adopt_tab),
                    ],
                    selected_idx,
                    cx,
                )
                .on_click(cx.listener(|this, idx: &usize, window, cx| {
                    this.sub_tab = if *idx == 0 {
                        DiscoverSubTab::Marketplace
                    } else {
                        DiscoverSubTab::Adopt
                    };
                    if *idx == 0 {
                        this.ensure_market_loaded(window, cx);
                    }
                    cx.notify();
                })),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .overflow_hidden()
                    .child(match self.sub_tab {
                        DiscoverSubTab::Marketplace => {
                            self.render_marketplace_tab(cx).into_any_element()
                        }
                        DiscoverSubTab::Adopt => self.render_adopt_tab(cx).into_any_element(),
                    }),
            )
    }
}
