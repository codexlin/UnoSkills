mod adopt;
mod marketplace;

use std::collections::{HashMap, HashSet};

use gpui_kit::component::{
    Sizable,
    input::{InputEvent, InputState},
    tab::{Tab, TabBar},
    v_flex,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription,
    UniformListScrollHandle, Window, div,
};
use unoskills_core::MarketplaceSkill;

use crate::components::page_header::PageHeader;
use crate::state::{NavTab, WorkspaceEvent, WorkspaceState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DiscoverSubTab {
    Marketplace,
    Adopt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum MarketBoard {
    Trending,
    Hot,
    AllTime,
}

impl MarketBoard {
    pub(super) fn as_api_str(self) -> &'static str {
        match self {
            Self::Trending => "trending",
            Self::Hot => "hot",
            Self::AllTime => "all-time",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct CachedMarketBoard {
    pub items: Vec<MarketplaceSkill>,
    pub next_page: usize,
    pub total: Option<usize>,
    pub has_more: bool,
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
    /// Monotonic counter guarding against out-of-order async leaderboard/search replies.
    pub(super) market_req_seq: u64,
    pub(super) market_scroll_handle: UniformListScrollHandle,
    pub(super) adopt_scroll_handle: UniformListScrollHandle,
    /// Indices into `WorkspaceState::adopt_candidates()` matching the adopt
    /// filter — recomputed on filter input / reload so `render()` never
    /// clones the candidate list.
    pub(super) filtered_adopt_indices: Vec<usize>,
    pub(super) install_source_input: Entity<InputState>,
    pub(super) market_search_input: Entity<InputState>,
    pub(super) adopt_search_input: Entity<InputState>,
    /// Active source refs currently installing, keyed cleanly against
    /// `WorkspaceEvent::InstallFinished`.
    pub(super) installing_refs: HashSet<String>,
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

        let install_sub = cx.subscribe_in(
            &install_source_input,
            window,
            |this, _input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    let value = this.install_source_input.read(cx).value().to_string();
                    this.quick_install(value, window, cx);
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
                    cx.notify();
                }
            },
        );
        let workspace_sub = cx.subscribe_in(
            &workspace,
            window,
            |this, _ws, event: &WorkspaceEvent, window, cx| match event {
                WorkspaceEvent::NavigateTo(NavTab::Discover) => {
                    this.ensure_market_loaded(window, cx)
                }
                WorkspaceEvent::LanguageChanged => this.apply_language(window, cx),
                WorkspaceEvent::InstallFinished(source_ref) => {
                    if this.installing_refs.remove(source_ref) {
                        cx.notify();
                    }
                }
                WorkspaceEvent::SettingsChanged => cx.notify(),
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
            market_req_seq: 0,
            market_scroll_handle: UniformListScrollHandle::new(),
            adopt_scroll_handle: UniformListScrollHandle::new(),
            filtered_adopt_indices: Vec::new(),
            install_source_input,
            market_search_input,
            adopt_search_input,
            installing_refs: HashSet::new(),
            _subscriptions: Subscription::join(
                Subscription::join(install_sub, market_sub),
                Subscription::join(adopt_sub, workspace_sub),
            ),
        };
        view.refilter_adopt(cx);
        view
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
        cx.notify();
    }

    pub(super) fn refilter_adopt(&mut self, cx: &Context<Self>) {
        let query = self
            .adopt_search_input
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let candidates = self.workspace.read(cx).adopt_candidates();
        if query.is_empty() {
            self.filtered_adopt_indices = (0..candidates.len()).collect();
        } else {
            self.filtered_adopt_indices = candidates
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    candidate.name.to_lowercase().contains(&query)
                        || candidate.agent_name.to_lowercase().contains(&query)
                        || candidate
                            .path
                            .to_string_lossy()
                            .to_lowercase()
                            .contains(&query)
                })
                .map(|(ix, _)| ix)
                .collect();
        }
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

    pub(super) fn quick_install(
        &mut self,
        source: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let trimmed = source.trim().to_string();
        if !trimmed.is_empty() {
            self.installing_refs.insert(trimmed);
            cx.notify();
        }
        self.workspace.update(cx, |ws, cx| {
            ws.install_source_async(&source, true, window, cx)
        });
    }

    pub(super) fn switch_market_board(
        &mut self,
        board: MarketBoard,
        force_refresh: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.market_board = board;
        self.market_search_active = false;
        self.market_loading_more = false;
        self.market_load_more_failed = false;
        self.market_req_seq = self.market_req_seq.wrapping_add(1);
        let req_seq = self.market_req_seq;

        if !force_refresh && let Some(cached) = self.market_board_cache.get(&board) {
            self.market_loading = false;
            self.marketplace_results = cached.items.clone();
            self.market_next_page = cached.next_page;
            self.market_total = cached.total;
            self.market_has_more = cached.has_more;
            cx.notify();
            return;
        }

        self.market_loading = true;
        if force_refresh {
            self.market_board_cache.remove(&board);
        }
        let busy_token = self.workspace.update(cx, |ws, cx| {
            let label = ws.t().busy_loading_trending;
            ws.begin_busy(label, cx)
        });
        cx.notify();

        let mgr = self.workspace.read(cx).manager().clone();
        let api_board = board.as_api_str();
        let bg = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let res = bg
                .spawn(async move { mgr.marketplace_leaderboard_page(api_board, 0) })
                .await;

            let _ = this.update_in(cx, |view, window, cx| {
                view.workspace
                    .update(cx, |ws, cx| ws.end_busy(busy_token, cx));
                match res {
                    Ok(page_data) => {
                        let cached = CachedMarketBoard {
                            items: page_data.items,
                            next_page: page_data.page + 1,
                            total: page_data.total,
                            has_more: page_data.has_more,
                        };
                        view.market_board_cache.insert(board, cached.clone());
                        if view.market_req_seq == req_seq {
                            view.market_loading = false;
                            view.marketplace_results = cached.items;
                            view.market_next_page = cached.next_page;
                            view.market_total = cached.total;
                            view.market_has_more = cached.has_more;
                            cx.notify();
                        }
                    }
                    Err(e) => {
                        if view.market_req_seq == req_seq {
                            view.market_loading = false;
                            view.workspace.update(cx, |ws, cx| {
                                ws.notify_toast(true, format!("{e}"), window, cx)
                            });
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    /// Fetches the next page for the active leaderboard and appends deduplicated
    /// skills to both the live view and the per-board cache.
    pub(super) fn fetch_next_market_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.market_search_active || !self.market_has_more || self.market_loading {
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

        self.market_search_active = true;
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
                        cx.notify();
                    }
                    Err(e) => {
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
        let s = self.workspace.read(cx).t();
        let adopt_count = self.workspace.read(cx).adopt_candidates().len();
        let selected_idx = match self.sub_tab {
            DiscoverSubTab::Marketplace => 0,
            DiscoverSubTab::Adopt => 1,
        };

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                PageHeader::new(s.discover_title)
                    .subtitle(s.discover_subtitle)
                    .right_slot(
                        TabBar::new("discover-sub-tabs")
                            .segmented()
                            .small()
                            .selected_index(selected_idx)
                            .child(Tab::new().label(s.tab_marketplace_install))
                            .child(
                                Tab::new()
                                    .label(format!("{} ({adopt_count})", s.tab_adopt_existing)),
                            )
                            .on_click(cx.listener(|this, idx: &usize, window, cx| {
                                if *idx == 0 {
                                    this.sub_tab = DiscoverSubTab::Marketplace;
                                    this.ensure_market_loaded(window, cx);
                                } else {
                                    this.sub_tab = DiscoverSubTab::Adopt;
                                    cx.notify();
                                }
                            })),
                    ),
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
