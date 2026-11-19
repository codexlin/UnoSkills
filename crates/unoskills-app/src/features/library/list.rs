use std::ops::Range;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex,
    input::Input,
    scroll::ScrollableElement,
    v_flex,
};
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, uniform_list,
};

use crate::components::badges::{StatusBadge, TokenBadge};
use crate::state::NavTab;

use super::LibraryView;

impl LibraryView {
    pub(super) fn render_skill_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let skills_empty = self.workspace.read(cx).skills().is_empty();
        let item_count = self.filtered_indices.len();

        v_flex()
            .size_full()
            .min_h_0()
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_base().font_semibold().child(s.my_skills))
                            .child(
                                Button::new("lib-add-skill")
                                    .primary()
                                    .xsmall()
                                    .icon(IconName::Plus)
                                    .label(s.btn_install)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.workspace
                                            .update(cx, |ws, cx| ws.navigate(NavTab::Discover, cx));
                                    })),
                            ),
                    )
                    .child(Input::new(&self.library_search).small().cleanable(true)),
            )
            .child(if item_count == 0 {
                v_flex()
                    .flex_1()
                    .size_full()
                    .p_4()
                    .items_center()
                    .justify_center()
                    .child(
                        Empty::new()
                            .header(
                                EmptyHeader::new()
                                    .media(
                                        EmptyMedia::new()
                                            .child(Icon::new(IconName::BookOpen).size_6()),
                                    )
                                    .title(EmptyTitle::new().child(if skills_empty {
                                        s.empty_library_title
                                    } else {
                                        s.empty_search_title
                                    }))
                                    .description(EmptyDescription::new().child(if skills_empty {
                                        s.empty_library_desc
                                    } else {
                                        s.empty_search_desc
                                    })),
                            )
                            .when(skills_empty, |empty| {
                                empty.content(
                                    EmptyContent::new().child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("empty-go-stacks")
                                                    .primary()
                                                    .small()
                                                    .label(s.btn_browse_stacks)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.workspace.update(cx, |ws, cx| {
                                                            ws.navigate(NavTab::Stacks, cx)
                                                        });
                                                    })),
                                            )
                                            .child(
                                                Button::new("empty-go-discover")
                                                    .outline()
                                                    .small()
                                                    .label(s.btn_discover_adopt)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.workspace.update(cx, |ws, cx| {
                                                            ws.navigate(NavTab::Discover, cx)
                                                        });
                                                    })),
                                            ),
                                    ),
                                )
                            }),
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
                            "library-skill-vlist",
                            item_count,
                            cx.processor(|this, range: Range<usize>, _window, cx| {
                                let theme = cx.theme();
                                let s = this.workspace.read(cx).t();
                                let skills = this.workspace.read(cx).skills();
                                let accent = theme.accent;
                                let bg_norm = theme.background;
                                let primary = theme.primary;
                                let border_subtle = theme.border.opacity(0.4);
                                let muted = theme.muted_foreground;
                                let selected_id = this.selected_skill_id.clone();

                                range
                                    .filter_map(|ix| {
                                        let skill = skills.get(*this.filtered_indices.get(ix)?)?;
                                        let is_selected =
                                            selected_id.as_deref() == Some(skill.id.as_str());
                                        let sid = skill.id.clone();
                                        let dep_count = skill.deployments.len();

                                        Some(
                                            div().w_full().px_2().py(px(3.)).child(
                                                v_flex()
                                                    .id(SharedString::from(format!(
                                                        "skill-row-{}",
                                                        skill.id
                                                    )))
                                                    .role(gpui_kit::Role::ListItem)
                                                    .aria_label(skill.name.clone())
                                                    .aria_selected(is_selected)
                                                    .w_full()
                                                    .h(px(74.))
                                                    .px_3()
                                                    .py_2()
                                                    .rounded_md()
                                                    .bg(if is_selected { accent } else { bg_norm })
                                                    .border_1()
                                                    .border_color(if is_selected {
                                                        primary
                                                    } else {
                                                        border_subtle
                                                    })
                                                    .cursor_pointer()
                                                    .justify_between()
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.select_skill(
                                                                sid.clone(),
                                                                window,
                                                                cx,
                                                            );
                                                        },
                                                    ))
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .justify_between()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .flex_1()
                                                                    .min_w_0()
                                                                    .text_sm()
                                                                    .font_medium()
                                                                    .truncate()
                                                                    .child(skill.name.clone()),
                                                            )
                                                            .child(div().flex_shrink_0().child(
                                                                TokenBadge::tokens(
                                                                    skill.estimated_tokens,
                                                                ),
                                                            )),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted)
                                                            .truncate()
                                                            .child(
                                                                skill
                                                                    .description
                                                                    .clone()
                                                                    .unwrap_or_default(),
                                                            ),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .gap_1()
                                                            .child(if dep_count > 0 {
                                                                StatusBadge::success(format!(
                                                                    "{dep_count} {}",
                                                                    s.agents_unit
                                                                ))
                                                                .outline()
                                                                .into_any_element()
                                                            } else {
                                                                StatusBadge::secondary(
                                                                    s.library_only,
                                                                )
                                                                .into_any_element()
                                                            })
                                                            .children(
                                                                skill.tags.iter().take(2).map(
                                                                    |t| {
                                                                        StatusBadge::secondary(
                                                                            t.clone(),
                                                                        )
                                                                        .outline()
                                                                    },
                                                                ),
                                                            ),
                                                    ),
                                            ),
                                        )
                                    })
                                    .collect()
                            }),
                        )
                        .size_full()
                        .py_1()
                        .track_scroll(&self.library_scroll_handle),
                    )
                    .vertical_scrollbar(&self.library_scroll_handle)
                    .into_any_element()
            })
    }
}
