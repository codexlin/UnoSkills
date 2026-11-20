use std::ops::Range;

use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, StyledExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
    select::Select,
    spinner::Spinner,
    tag::Tag,
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, StyleRefinement, Styled, div, prelude::FluentBuilder, px, rems,
    uniform_list,
};

use gpui_kit::assets::IconName as AssetIcon;
use unoskills_core::source::skillssh_detail_url;

use crate::components::badges::format_install_count;
use crate::state::MarketInstallStatus;

use super::{DiscoverView, MarketBoard};

const MARKET_COLUMNS: usize = 3;
const MARKET_CARD_HEIGHT: f32 = 6.25;

fn market_rank(rank: usize, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let label = format!("#{rank}");
    let rank_label = match rank {
        1 => h_flex()
            .gap_1()
            .text_xs()
            .font_semibold()
            .child(
                Icon::new(AssetIcon::Trophy)
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
    h_flex().flex_shrink_0().justify_center().child(rank_label)
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
            _ if filtered => format!("{} · {item_count}", s.market_source_filter),
            Some(total) if !self.market_search_active && total > item_count => {
                format!("{item_count} / {total}")
            }
            _ => item_count.to_string(),
        };

        let boards = [
            MarketBoard::Trending,
            MarketBoard::Hot,
            MarketBoard::AllTime,
        ];
        let board_label = current_board.label(s);
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
                        div().flex_1().min_w(px(160.)).max_w(px(360.)).child(
                            Input::new(&self.market_search_input)
                                .aria_label(s.ph_market_search)
                                .cleanable(true),
                        ),
                    )
                    .child(
                        Button::new("search-market-btn")
                            .ghost()
                            .icon(IconName::Search)
                            .tooltip(s.market_search_hint)
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
                                for board in boards {
                                    let view = board_view.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(board.label(s))
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
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap_1()
                            .child(
                                div().w(rems(11.)).child(
                                    Select::new(&self.market_source_select)
                                        .id("market-source-filter")
                                        .accessibility_label(s.market_source_label)
                                        .search_placeholder(s.market_source_search)
                                        .menu_width(px(288.))
                                        .menu_max_h(px(300.))
                                        .disabled(self.marketplace_results.is_empty()),
                                ),
                            )
                            .when(filtered, |row| {
                                row.child(
                                    Button::new("market-clear-source-filter")
                                        .ghost()
                                        .icon(IconName::Close)
                                        .accessibility_label(s.market_clear_source_filter)
                                        .tooltip(s.market_clear_source_filter)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.set_market_source_filter(None, window, cx);
                                        })),
                                )
                            }),
                    )
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
                                    .px_3()
                                    .py_2()
                                    .justify_between()
                                    .rounded(theme.radius_lg)
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.group_box)
                                    .child(div().w_1_2().h_3().rounded_sm().bg(theme.skeleton))
                                    .child(div().w_2_5().h_2().rounded_sm().bg(theme.skeleton))
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .child(
                                                div().w_1_4().h_2().rounded_sm().bg(theme.skeleton),
                                            )
                                            .child(
                                                div().w_1_4().h_6().rounded_sm().bg(theme.skeleton),
                                            ),
                                    )
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
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .children(this.render_market_card(index, cx))
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

    fn render_market_card(&self, index: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (index, item) = self.market_result(index)?;
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let rank = (!self.market_search_active).then_some(index + 1);
        let name = item.name.clone();
        let source = item.source.clone();
        let identity = format!("{name} · {source}");
        let source_selected = self
            .market_source_filter
            .as_ref()
            .is_some_and(|filter| filter.source == source);
        let installs = format!(
            "{} {}",
            format_install_count(item.installs),
            s.installs_unit
        );
        let total_installs = format!("{} {}", item.installs, s.installs_unit);
        let status = ws.market_install_status(item);
        let pending = ws.is_source_installing(&item.install_ref());
        let border = if rank == Some(1) {
            theme.border.blend(theme.primary.opacity(0.48))
        } else {
            theme.border
        };
        let (action_label, action_icon, action_tooltip) = match status {
            MarketInstallStatus::Available => (
                s.market_add_to_library,
                AssetIcon::Plus,
                s.market_add_to_library,
            ),
            MarketInstallStatus::Installed => {
                (s.market_open_skill, AssetIcon::Check, s.already_in_library)
            }
            MarketInstallStatus::SameName => (
                s.market_same_name,
                AssetIcon::CircleAlert,
                s.market_same_name,
            ),
        };
        let item_for_action = item.clone();
        let primary_action = Button::new("market-skill-action")
            .outline()
            .small()
            .flex_shrink_0()
            .icon(action_icon)
            .label(action_label)
            .tooltip(action_tooltip)
            .accessibility_label(format!("{action_label} · {identity}"))
            .loading(pending)
            .disabled(status == MarketInstallStatus::SameName)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.workspace.update(cx, |ws, cx| {
                    let source = item_for_action.install_ref();
                    match ws.market_install_status(&item_for_action) {
                        MarketInstallStatus::Available => {
                            ws.install_source_async(&source, false, window, cx);
                        }
                        MarketInstallStatus::Installed => {
                            let key = source.to_lowercase();
                            if let Some(id) = ws.skills().iter().find_map(|skill| {
                                skill
                                    .source_ref
                                    .as_ref()
                                    .filter(|reference| reference.to_lowercase() == key)
                                    .map(|_| skill.id.clone())
                            }) {
                                ws.open_library_skill(id, cx);
                            }
                        }
                        MarketInstallStatus::SameName => {}
                    }
                });
            }));

        Some(
            GroupBox::new()
                .id(format!("market-card-{}", item.id))
                .outline()
                .w_full()
                .content_style(
                    StyleRefinement::default()
                        .h(rems(MARKET_CARD_HEIGHT))
                        .px_3()
                        .py_2()
                        .gap_1()
                        .justify_between()
                        .rounded(theme.radius_lg)
                        .bg(theme.group_box)
                        .border_color(border),
                )
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .h_5()
                        .child(
                            div()
                                .id("market-name")
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .font_semibold()
                                .truncate()
                                .tooltip(move |window, cx| {
                                    Tooltip::new(name.clone()).build(window, cx)
                                })
                                .child(item.name.clone()),
                        )
                        .when(item.is_official, |row| {
                            row.child(
                                div()
                                    .id("market-official")
                                    .flex_shrink_0()
                                    .aria_label(s.market_official)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(s.market_official).build(window, cx)
                                    })
                                    .child(
                                        Icon::new(AssetIcon::BadgeCheck)
                                            .size_4()
                                            .text_color(theme.primary),
                                    ),
                            )
                        })
                        .when_some(rank, |row, rank| row.child(market_rank(rank, cx))),
                )
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .h_5()
                        .text_xs()
                        .text_color(muted)
                        .when(!source.is_empty(), |row| {
                            row.child(
                                Button::new("market-filter-card-source")
                                    .ghost()
                                    .xsmall()
                                    .flex_1()
                                    .min_w_0()
                                    .px_0()
                                    .text_color(if source_selected {
                                        theme.primary
                                    } else {
                                        muted
                                    })
                                    .icon(AssetIcon::GitFork)
                                    .accessibility_label(format!(
                                        "{} · {identity}",
                                        s.market_filter_scope
                                    ))
                                    .tooltip(format!("{} · {source}", s.market_filter_scope))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_left()
                                            .truncate()
                                            .child(source.clone()),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.set_market_source_filter(
                                            Some(source.clone()),
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                        })
                        .when_some(skillssh_detail_url(item), |row, url| {
                            row.child(
                                Button::new("market-open-external")
                                    .ghost()
                                    .xsmall()
                                    .flex_shrink_0()
                                    .icon(IconName::ExternalLink)
                                    .tooltip(s.market_view_details)
                                    .accessibility_label(format!(
                                        "{} · {identity}",
                                        s.market_view_details
                                    ))
                                    .on_click(move |_, _, cx| cx.open_url(url.as_str())),
                            )
                        }),
                )
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .h_6()
                        .child(
                            h_flex()
                                .id("market-installs")
                                .flex_1()
                                .min_w_0()
                                .gap_2()
                                .text_xs()
                                .text_color(muted)
                                .tooltip(move |window, cx| {
                                    Tooltip::new(total_installs.clone()).build(window, cx)
                                })
                                .child(Icon::new(AssetIcon::Download).size_3().flex_shrink_0())
                                .child(div().min_w_0().truncate().child(installs)),
                        )
                        .child(primary_action),
                )
                .into_any_element(),
        )
    }
}
