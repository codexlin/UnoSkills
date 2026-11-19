use std::ops::Range;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex,
    input::Input,
    scroll::ScrollableElement,
    spinner::Spinner,
    tab::{Tab, TabBar},
    v_flex,
};
use gpui_kit::{
    Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::FluentBuilder, px,
    uniform_list,
};

use crate::components::badges::StatusBadge;

use super::{DiscoverView, MarketBoard};

fn format_installs(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

impl DiscoverView {
    pub(super) fn render_marketplace_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let board_idx = match self.market_board {
            MarketBoard::Trending => 0,
            MarketBoard::Hot => 1,
            MarketBoard::AllTime => 2,
        };
        let item_count = self.marketplace_results.len();
        let show_footer = !self.market_search_active
            && item_count > 0
            && (self.market_has_more || self.market_loading_more);
        let virtual_row_count = item_count + usize::from(show_footer);
        let is_loading = self.market_loading;
        let current_board = self.market_board;
        let count_badge = match self.market_total {
            Some(total) if !self.market_search_active && total > item_count => {
                format!("{item_count} / {total}")
            }
            _ => format!("{item_count}"),
        };

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            // 1. Compact Quick Install strip
            .child(
                h_flex()
                    .w_full()
                    .px_5()
                    .py_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.secondary.opacity(0.3))
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(theme.muted_foreground)
                            .flex_shrink_0()
                            .child(s.quick_install_title),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.install_source_input).small()),
                    )
                    .child(
                        div().flex_shrink_0().child(
                            Button::new("browse-local-skill-btn")
                                .outline()
                                .small()
                                .icon(IconName::FolderOpen)
                                .tooltip(s.btn_browse_folder)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let prompt = this.workspace.read(cx).t().prompt_pick_skill_dir;
                                    let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
                                        files: false,
                                        directories: true,
                                        multiple: false,
                                        prompt: Some(SharedString::from(prompt)),
                                    });
                                    cx.spawn_in(window, async move |this, cx| {
                                        if let Ok(Ok(Some(paths))) = rx.await
                                            && let Some(dir) = paths.into_iter().next()
                                        {
                                            let path_str = dir.to_string_lossy().to_string();
                                            let _ = this.update_in(cx, |view, window, cx| {
                                                view.install_source_input.update(
                                                    cx,
                                                    |input, cx| {
                                                        input.set_value(
                                                            path_str.clone(),
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                );
                                                view.quick_install(path_str, window, cx);
                                            });
                                        }
                                    })
                                    .detach();
                                })),
                        ),
                    )
                    .child(
                        div().flex_shrink_0().child(
                            Button::new("direct-install-btn")
                                .primary()
                                .small()
                                .icon(IconName::Plus)
                                .label(s.btn_install)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let value =
                                        this.install_source_input.read(cx).value().to_string();
                                    this.quick_install(value, window, cx);
                                })),
                        ),
                    ),
            )
            // 2. Marketplace toolbar: board switcher + search + refresh + count
            .child(
                h_flex()
                    .w_full()
                    .px_5()
                    .py_3()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .items_center()
                    .justify_between()
                    .child(
                        div().flex_shrink_0().child(
                            TabBar::new("market-board-tabs")
                                .segmented()
                                .small()
                                .selected_index(board_idx)
                                .child(Tab::new().label(s.board_trending))
                                .child(Tab::new().label(s.board_hot))
                                .child(Tab::new().label(s.board_all_time))
                                .on_click(cx.listener(|this, idx: &usize, window, cx| {
                                    let board = match *idx {
                                        1 => MarketBoard::Hot,
                                        2 => MarketBoard::AllTime,
                                        _ => MarketBoard::Trending,
                                    };
                                    this.switch_market_board(board, false, window, cx);
                                })),
                        ),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        Input::new(&self.market_search_input)
                                            .small()
                                            .cleanable(true),
                                    ),
                            )
                            .child(
                                Button::new("search-market-btn")
                                    .primary()
                                    .small()
                                    .icon(IconName::Search)
                                    .label(s.btn_search)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let q = this
                                            .market_search_input
                                            .read(cx)
                                            .value()
                                            .to_string();
                                        this.search_marketplace_async(q, window, cx);
                                    })),
                            )
                            .child(
                                Button::new("refresh-market-btn")
                                    .outline()
                                    .small()
                                    .icon(IconName::RefreshCw)
                                    .tooltip(s.btn_trending)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.switch_market_board(current_board, true, window, cx);
                                    })),
                            )
                            .when(item_count > 0, |row| {
                                row.child(StatusBadge::secondary(count_badge))
                            }),
                    ),
            )
            // 3. Virtualized marketplace list with infinite scroll
            .child(if is_loading && item_count == 0 {
                v_flex()
                    .flex_1()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(Spinner::new().large())
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(s.market_loading),
                    )
                    .into_any_element()
            } else if item_count == 0 {
                v_flex()
                    .flex_1()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(
                        Empty::new().header(
                            EmptyHeader::new()
                                .media(EmptyMedia::new().child(Icon::new(IconName::Globe).size_6()))
                                .title(EmptyTitle::new().child(s.empty_market_title))
                                .description(EmptyDescription::new().child(s.empty_market_desc)),
                        ),
                    )
                    .into_any_element()
            } else {
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .child(
                        uniform_list(
                            "marketplace-vlist",
                            virtual_row_count,
                            cx.processor(|this, range: Range<usize>, window, cx| {
                                let loaded = this.marketplace_results.len();
                                // Trigger infinite-scroll next page when viewport approaches end.
                                if !this.market_search_active
                                    && this.market_has_more
                                    && !this.market_loading
                                    && !this.market_loading_more
                                    && !this.market_load_more_failed
                                    && range.end + 15 >= loaded
                                {
                                    this.market_loading_more = true;
                                    cx.defer_in(window, |this, window, cx| {
                                        this.fetch_next_market_page(window, cx);
                                    });
                                }

                                let theme = cx.theme();
                                let s = this.workspace.read(cx).t();
                                let border = theme.border;
                                let muted = theme.muted_foreground;
                                let loading_more = this.market_loading_more;

                                range
                                    .filter_map(|ix| {
                                        // Footer sentinel row (exact same 62px height as skill rows)
                                        if ix == loaded {
                                            return Some(
                                                div().w_full().px_5().py_1().child(
                                                    h_flex()
                                                        .w_full()
                                                        .h(px(62.))
                                                        .items_center()
                                                        .justify_center()
                                                        .gap_2()
                                                        .child(if loading_more {
                                                            h_flex()
                                                                .items_center()
                                                                .gap_2()
                                                                .child(Spinner::new().small())
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(muted)
                                                                        .child(
                                                                            s.market_loading_more,
                                                                        ),
                                                                )
                                                                .into_any_element()
                                                        } else {
                                                            Button::new("market-load-more-btn")
                                                                .outline()
                                                                .small()
                                                                .icon(IconName::RefreshCw)
                                                                .label(s.btn_load_more)
                                                                .on_click(cx.listener(
                                                                    |this, _, window, cx| {
                                                                        this.fetch_next_market_page(
                                                                            window, cx,
                                                                        );
                                                                    },
                                                                ))
                                                                .into_any_element()
                                                        }),
                                                ),
                                            );
                                        }

                                        let item = this.marketplace_results.get(ix)?;
                                        let install_ref = item.install_ref();
                                        let already_installed = this
                                            .workspace
                                            .read(cx)
                                            .is_market_skill_installed(item);
                                        let is_installing =
                                            this.installing_refs.contains(&install_ref);
                                        let install_ref_btn = install_ref.clone();
                                        let installs_label = format!(
                                            "{} {}",
                                            format_installs(item.installs),
                                            s.installs_unit
                                        );
                                        let name = item.name.clone();

                                        Some(
                                            div().w_full().px_5().py_1().child(
                                                h_flex()
                                                    .w_full()
                                                    .h(px(62.))
                                                    .px_3()
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(border)
                                                    .items_center()
                                                    .justify_between()
                                                    .gap_3()
                                                    .child(
                                                        v_flex()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .gap_1()
                                                            .child(
                                                                h_flex()
                                                                    .items_center()
                                                                    .gap_2()
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(muted)
                                                                            .flex_shrink_0()
                                                                            .child(format!(
                                                                                "#{}",
                                                                                ix + 1
                                                                            )),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .font_medium()
                                                                            .truncate()
                                                                            .child(name),
                                                                    )
                                                                    .child(StatusBadge::secondary(
                                                                        installs_label,
                                                                    )),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted)
                                                                    .truncate()
                                                                    .child(install_ref),
                                                            ),
                                                    )
                                                    .child(div().flex_shrink_0().child(
                                                        if already_installed {
                                                            StatusBadge::success(
                                                                s.already_in_library,
                                                            )
                                                            .outline()
                                                            .into_any_element()
                                                        } else {
                                                            Button::new(SharedString::from(
                                                                format!("market-install-{ix}"),
                                                            ))
                                                            .primary()
                                                            .xsmall()
                                                            .loading(is_installing)
                                                            .icon(IconName::Plus)
                                                            .label(s.btn_install)
                                                            .tooltip(install_ref_btn.clone())
                                                            .on_click(cx.listener(
                                                                move |this, _, window, cx| {
                                                                    this.installing_refs.insert(
                                                                        install_ref_btn.clone(),
                                                                    );
                                                                    cx.notify();
                                                                    this.workspace.update(
                                                                        cx,
                                                                        |ws, cx| {
                                                                            ws.install_source_async(
                                                                                &install_ref_btn,
                                                                                false,
                                                                                window,
                                                                                cx,
                                                                            )
                                                                        },
                                                                    );
                                                                },
                                                            ))
                                                            .into_any_element()
                                                        },
                                                    )),
                                            ),
                                        )
                                    })
                                    .collect()
                            }),
                        )
                        .size_full()
                        .py_2()
                        .track_scroll(&self.market_scroll_handle),
                    )
                    .vertical_scrollbar(&self.market_scroll_handle)
                    .into_any_element()
            })
    }
}
