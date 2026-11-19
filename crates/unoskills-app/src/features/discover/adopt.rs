use std::ops::Range;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex,
    input::Input,
    scroll::ScrollableElement,
    v_flex,
};
use gpui_kit::{
    Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::FluentBuilder, px,
    uniform_list,
};

use crate::components::badges::StatusBadge;

use super::DiscoverView;

impl DiscoverView {
    pub(super) fn render_adopt_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let total_count = ws.adopt_candidates().len();
        let adopting_all = ws.adopting_all();
        let item_count = self.filtered_adopt_indices.len();

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                h_flex()
                    .px_5()
                    .py_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.adopt_search_input).small().cleanable(true)),
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap_2()
                            .child(
                                Button::new("rescan-adopt")
                                    .outline()
                                    .small()
                                    .icon(IconName::RefreshCw)
                                    .label(s.btn_rescan)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.workspace.update(cx, |ws, cx| ws.reload_all(cx));
                                    })),
                            )
                            .when(total_count > 0, |row| {
                                row.child(
                                    Button::new("adopt-all-btn")
                                        .primary()
                                        .small()
                                        .loading(adopting_all)
                                        .icon(IconName::Check)
                                        .label(format!("{} ({total_count})", s.btn_adopt_all))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.workspace.update(cx, |ws, cx| {
                                                ws.adopt_all_candidates_async(window, cx)
                                            });
                                        })),
                                )
                            }),
                    ),
            )
            .child(if item_count == 0 {
                v_flex()
                    .flex_1()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(
                        Empty::new().header(
                            EmptyHeader::new()
                                .media(
                                    EmptyMedia::new()
                                        .child(Icon::new(IconName::CircleCheck).size_6()),
                                )
                                .title(EmptyTitle::new().child(if total_count == 0 {
                                    s.empty_adopt_done_title
                                } else {
                                    s.empty_adopt_filter_title
                                }))
                                .description(EmptyDescription::new().child(if total_count == 0 {
                                    s.empty_adopt_done_desc
                                } else {
                                    s.empty_adopt_filter_desc
                                })),
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
                            "adopt-vlist",
                            item_count,
                            cx.processor(|this, range: Range<usize>, _window, cx| {
                                let theme = cx.theme();
                                let s = this.workspace.read(cx).t();
                                let candidates = this.workspace.read(cx).adopt_candidates();
                                let border = theme.border;
                                let muted = theme.muted_foreground;

                                range
                                    .filter_map(|ix| {
                                        let candidate = candidates
                                            .get(*this.filtered_adopt_indices.get(ix)?)?;
                                        let candidate_clone = candidate.clone();
                                        let path_str = candidate.path.to_string_lossy().to_string();
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
                                                                            .text_sm()
                                                                            .font_medium()
                                                                            .truncate()
                                                                            .child(
                                                                                candidate
                                                                                    .name
                                                                                    .clone(),
                                                                            ),
                                                                    )
                                                                    .child(StatusBadge::secondary(
                                                                        candidate
                                                                            .agent_name
                                                                            .clone(),
                                                                    ))
                                                                    .child(
                                                                        StatusBadge::info(format!(
                                                                            "~{} tok",
                                                                            candidate
                                                                                .estimated_tokens
                                                                        ))
                                                                        .outline(),
                                                                    ),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted)
                                                                    .truncate()
                                                                    .child(path_str),
                                                            ),
                                                    )
                                                    .child(
                                                        div().flex_shrink_0().child(
                                                            Button::new(SharedString::from(
                                                                format!("adopt-btn-{ix}"),
                                                            ))
                                                            .primary()
                                                            .xsmall()
                                                            .label(s.btn_adopt)
                                                            .on_click(cx.listener(
                                                                move |this, _, window, cx| {
                                                                    this.workspace.update(
                                                                        cx,
                                                                        |ws, cx| {
                                                                            ws.adopt_candidate(
                                                                                &candidate_clone,
                                                                                window,
                                                                                cx,
                                                                            )
                                                                        },
                                                                    );
                                                                },
                                                            )),
                                                        ),
                                                    ),
                                            ),
                                        )
                                    })
                                    .collect()
                            }),
                        )
                        .size_full()
                        .py_2()
                        .track_scroll(&self.adopt_scroll_handle),
                    )
                    .vertical_scrollbar(&self.adopt_scroll_handle)
                    .into_any_element()
            })
    }
}
