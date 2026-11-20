use std::ops::Range;

use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, StyledExt, WindowExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
    spinner::Spinner,
    tag::Tag,
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px, rems,
    uniform_list,
};

use unoskills_core::{MarketplaceSkill, source::skillssh_detail_url};

use crate::components::badges::StatusBadge;
use crate::components::interactive::InteractiveCard;
use crate::state::MarketInstallStatus;

use super::{DiscoverView, MarketBoard};

const MARKET_COLUMNS: usize = 3;
const MARKET_CARD_HEIGHT: f32 = 4.75;

fn format_installs(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn market_rank(rank: usize, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let label = format!("#{rank}");
    let rank_label = match rank {
        1 => h_flex()
            .gap_1()
            .text_xs()
            .font_semibold()
            .child(
                Icon::new(gpui_kit::assets::IconName::Trophy)
                    .size_3()
                    .text_color(theme.primary),
            )
            .child(label)
            .into_any_element(),
        2 | 3 => Tag::custom(
            theme.secondary,
            theme.foreground,
            theme.border.blend(theme.foreground.opacity(0.18)),
        )
        .small()
        .text_xs()
        .font_semibold()
        .child(label)
        .into_any_element(),
        _ => div()
            .text_xs()
            .font_family(theme.mono_font_family.clone())
            .text_color(theme.muted_foreground)
            .child(label)
            .into_any_element(),
    };
    h_flex()
        .min_w(rems(1.75))
        .flex_shrink_0()
        .justify_center()
        .child(rank_label)
}

impl DiscoverView {
    pub(super) fn render_marketplace_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let item_count = self.market_result_count();
        let filtered = self.market_source_filter.is_some();
        let row_count = item_count.div_ceil(MARKET_COLUMNS);
        let show_footer = !self.market_search_active
            && !filtered
            && item_count > 0
            && (self.market_has_more || self.market_loading_more);
        let virtual_row_count = row_count + usize::from(show_footer);
        let is_loading = self.market_loading;
        let loading_label = if is_loading {
            s.market_loading
        } else {
            s.btn_trending
        };
        let current_board = self.market_board;
        let count_badge = match self.market_total {
            Some(total) if !self.market_search_active && !filtered && total > item_count => {
                format!("{item_count} / {total}")
            }
            _ => format!("{item_count}"),
        };

        let boards = [
            (MarketBoard::Trending, s.board_trending),
            (MarketBoard::Hot, s.board_hot),
            (MarketBoard::AllTime, s.board_all_time),
        ];
        let board_label = boards
            .iter()
            .find(|(board, _)| *board == current_board)
            .map_or(s.board_trending, |(_, label)| *label);
        let board_view = cx.weak_entity();

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                h_flex()
                    .min_h(px(48.))
                    .w_full()
                    .px_5()
                    .py_2()
                    .gap_2()
                    .items_center()
                    .flex_wrap()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(160.))
                            .max_w(px(360.))
                            .child(Input::new(&self.market_search_input).cleanable(true)),
                    )
                    .child(
                        Button::new("search-market-btn")
                            .ghost()
                            .icon(IconName::Search)
                            .tooltip(s.btn_search)
                            .accessibility_label(s.btn_search)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let query = this.market_search_input.read(cx).value().to_string();
                                this.search_marketplace_async(query, window, cx);
                            })),
                    )
                    .child(
                        Button::new("market-board-menu")
                            .outline()
                            .icon(IconName::ChevronDown)
                            .label(if self.market_search_active {
                                s.market_search_results
                            } else {
                                board_label
                            })
                            .tooltip(s.market_sort)
                            .dropdown_menu(move |mut menu, _, _| {
                                for (board, label) in boards {
                                    let view = board_view.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(label)
                                            .checked(current_board == board)
                                            .on_click(move |_, window, cx| {
                                                let _ = view.update(cx, |view, cx| {
                                                    view.switch_market_board(
                                                        board, false, window, cx,
                                                    );
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(self.source_filter_menu(cx))
                    .child(
                        Button::new("refresh-market-btn")
                            .ghost()
                            .icon(IconName::RefreshCw)
                            .tooltip(loading_label)
                            .accessibility_label(loading_label)
                            .loading(is_loading)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.market_search_active {
                                    let query =
                                        this.market_search_input.read(cx).value().to_string();
                                    this.search_marketplace_async(query, window, cx);
                                } else {
                                    this.switch_market_board(current_board, true, window, cx);
                                }
                            })),
                    )
                    .child(div().flex_1())
                    .when(item_count > 0, |row| {
                        row.child(
                            div()
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(count_badge),
                        )
                    }),
            )
            .when_some(self.market_error.clone(), |page, error| {
                page.child(
                    div()
                        .px_5()
                        .pb_2()
                        .child(Alert::error("market-error", error).banner().small()),
                )
            })
            .when_some(self.market_source_filter.as_ref(), |page, filter| {
                page.child(
                    h_flex().w_full().px_5().pt_2().child(
                        Button::new("market-clear-source-filter")
                            .outline()
                            .small()
                            .min_w_0()
                            .icon(gpui_kit::assets::IconName::X)
                            .label(format!("{} · {}", s.market_source_filter, filter.source))
                            .accessibility_label(s.market_clear_source_filter)
                            .tooltip(s.market_clear_source_filter)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_market_source_filter(None, cx);
                            })),
                    ),
                )
            })
            .child(if is_loading && item_count == 0 {
                v_flex()
                    .id("market-loading")
                    .role(gpui_kit::Role::Status)
                    .aria_label(s.market_loading)
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .px_5()
                    .py_2()
                    .children((0..4).map(|_| {
                        h_flex()
                            .w_full()
                            .gap_3()
                            .py_1p5()
                            .children((0..MARKET_COLUMNS).map(|_| {
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .h(rems(MARKET_CARD_HEIGHT))
                                    .p_2()
                                    .justify_between()
                                    .rounded(theme.radius_lg)
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.group_box)
                                    .child(div().w_1_2().h_3().rounded_sm().bg(theme.skeleton))
                                    .child(div().w_2_5().h_2().rounded_sm().bg(theme.skeleton))
                            }))
                    }))
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
                                let loaded = this.market_result_count();
                                let row_count = loaded.div_ceil(MARKET_COLUMNS);
                                // Trigger infinite-scroll next page when viewport approaches end.
                                if !this.market_search_active
                                    && this.market_source_filter.is_none()
                                    && this.market_has_more
                                    && !this.market_loading
                                    && !this.market_loading_more
                                    && !this.market_load_more_failed
                                    && (range.end * MARKET_COLUMNS) + 16 >= loaded
                                {
                                    this.market_loading_more = true;
                                    cx.defer_in(window, |this, window, cx| {
                                        this.fetch_next_market_page(window, cx);
                                    });
                                }

                                let theme = cx.theme();
                                let s = this.workspace.read(cx).t();
                                let muted = theme.muted_foreground;
                                let loading_more = this.market_loading_more;

                                range
                                    .map(|row_ix| {
                                        // Footer sentinel row
                                        if row_ix == row_count {
                                            return div().w_full().px_5().py_1p5().child(
                                                h_flex()
                                                    .w_full()
                                                    .h(rems(MARKET_CARD_HEIGHT))
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
                                                                    .child(s.market_loading_more),
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
                                            );
                                        }

                                        let start = row_ix * MARKET_COLUMNS;
                                        let cards = (start..start + MARKET_COLUMNS).map(|index| {
                                            div().flex_1().min_w_0().children(
                                                this.render_market_card(index, window, cx),
                                            )
                                        });
                                        div()
                                            .w_full()
                                            .px_5()
                                            .py_1p5()
                                            .child(h_flex().w_full().gap_3().children(cards))
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

    fn source_filter_menu(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let s = self.workspace.read(cx).t();
        let view = cx.weak_entity();
        Button::new("market-source-menu")
            .outline()
            .icon(IconName::ChevronDown)
            .label(s.market_source_label)
            .tooltip(s.market_filter_scope)
            .dropdown_menu(move |mut menu, _, cx| {
                let Some((sources, selected)) = view
                    .update(cx, |view, _| {
                        let sources = view
                            .marketplace_results
                            .iter()
                            .filter(|item| !item.source.is_empty())
                            .map(|item| item.source.clone())
                            .collect::<std::collections::BTreeSet<_>>();
                        (
                            sources,
                            view.market_source_filter
                                .as_ref()
                                .map(|filter| filter.source.clone()),
                        )
                    })
                    .ok()
                else {
                    return menu;
                };
                let selection_view = view.clone();
                menu = menu.item(
                    PopupMenuItem::new(s.market_all_sources)
                        .checked(selected.is_none())
                        .on_click(move |_, _, cx| {
                            let _ = selection_view
                                .update(cx, |view, cx| view.set_market_source_filter(None, cx));
                        }),
                );
                for source in sources {
                    let selection_view = view.clone();
                    let checked = selected.as_deref() == Some(source.as_str());
                    menu = menu.item(
                        PopupMenuItem::new(source.clone())
                            .checked(checked)
                            .on_click(move |_, _, cx| {
                                let _ = selection_view.update(cx, |view, cx| {
                                    view.set_market_source_filter(Some(source.clone()), cx);
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn render_market_card(
        &self,
        index: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (index, item) = self.market_result(index)?;
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let rank = (!self.market_search_active).then_some(index + 1);
        let installs = format!("{} {}", format_installs(item.installs), s.installs_unit);
        let name = item.name.clone();
        let source = item.source.clone();
        let status = ws.market_install_status(item);
        let detail = item.clone();
        let card = InteractiveCard::new(format!("market-card-{}", item.id), v_flex(), false, cx);
        let card = if rank == Some(1) {
            card.surface(
                theme.group_box,
                theme.list_hover,
                theme.border.blend(theme.primary.opacity(0.48)),
            )
        } else {
            card
        };
        Some(
            card.accessible(format!("{name} · {source}"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_market_detail(detail.clone(), window, cx)
                }))
                .w_full()
                .h(rems(MARKET_CARD_HEIGHT))
                .p_3()
                .overflow_hidden()
                .justify_between()
                .child(
                    h_flex()
                        .gap_2()
                        .h_6()
                        .when_some(rank, |row, rank| row.child(market_rank(rank, cx)))
                        .child(
                            div()
                                .id("market-name")
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .font_semibold()
                                .truncate()
                                .tooltip({
                                    let name = name.clone();
                                    move |window, cx| Tooltip::new(name.clone()).build(window, cx)
                                })
                                .child(name),
                        )
                        .when(item.is_official, |row| {
                            row.child(
                                Icon::new(gpui_kit::assets::IconName::BadgeCheck)
                                    .size_4()
                                    .text_color(muted),
                            )
                        })
                        .when(status == MarketInstallStatus::Installed, |row| {
                            row.child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(theme.success)
                                    .child(s.already_in_library),
                            )
                        }),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .h_5()
                        .text_xs()
                        .text_color(muted)
                        .child(
                            div()
                                .id("market-source")
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .tooltip({
                                    let source = source.clone();
                                    move |window, cx| Tooltip::new(source.clone()).build(window, cx)
                                })
                                .child(source),
                        )
                        .child(div().flex_shrink_0().child(installs)),
                )
                .into_any_element(),
        )
    }

    fn open_market_detail(
        &self,
        item: MarketplaceSkill,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workspace = self.workspace.downgrade();
        window.open_sheet(cx, move |sheet, _, cx| {
            let Some(workspace) = workspace.upgrade() else {
                return sheet;
            };
            let ws = workspace.read(cx);
            let s = ws.t();
            let theme = cx.theme();
            let muted = theme.muted_foreground;
            let source = item.install_ref();
            let status = ws.market_install_status(&item);
            let pending = ws.is_source_installing(&source);
            let in_library = ws
                .skills()
                .iter()
                .find(|skill| {
                    skill
                        .source_ref
                        .as_deref()
                        .is_some_and(|r| r.to_lowercase() == source.to_lowercase())
                })
                .map(|skill| skill.id.clone());
            let workspace = workspace.downgrade();
            let action = match status {
                MarketInstallStatus::Installed => s.market_open_skill,
                _ => s.market_add_to_library,
            };
            let details = v_flex()
                .w_full()
                .gap_3()
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(s.market_source_label),
                        )
                        .child(div().text_sm().child(item.source.clone())),
                )
                .child(
                    div()
                        .text_sm()
                        .child(format!("{} {}", item.installs, s.installs_unit)),
                )
                .when(item.is_official, |pane| {
                    pane.child(StatusBadge::info(s.market_official).outline())
                })
                .when(status == MarketInstallStatus::Installed, |pane| {
                    pane.child(StatusBadge::success(s.already_in_library).outline())
                })
                .when(status == MarketInstallStatus::SameName, |pane| {
                    pane.child(StatusBadge::warning(s.market_same_name).outline())
                });

            sheet
                .size(px(416.))
                .title(item.name.clone())
                .child(details)
                .footer(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .justify_end()
                        .flex_wrap()
                        .when_some(skillssh_detail_url(&item), |row, url| {
                            row.child(
                                Button::new("market-detail-external")
                                    .outline()
                                    .icon(IconName::ExternalLink)
                                    .label(s.market_view_details)
                                    .on_click(move |_, _, cx| cx.open_url(url.as_str())),
                            )
                        })
                        .child(
                            Button::new("market-detail-install")
                                .primary()
                                .label(action)
                                .loading(pending)
                                .disabled(status == MarketInstallStatus::SameName)
                                .on_click(move |_, window, cx| {
                                    if let Some(id) = &in_library {
                                        window.close_sheet(cx);
                                        let _ = workspace.update(cx, |ws, cx| {
                                            ws.open_library_skill(id.clone(), cx)
                                        });
                                    } else if status == MarketInstallStatus::Available {
                                        let _ = workspace.update(cx, |ws, cx| {
                                            ws.install_source_async(&source, false, window, cx)
                                        });
                                    }
                                }),
                        ),
                )
        });
    }
}
