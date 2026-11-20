use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex, h_resizable,
    input::Input,
    resizable_panel,
    scroll::ScrollableElement,
    tag::Tag,
    v_flex,
};
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, ScrollStrategy,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, rems, uniform_list,
};
use unoskills_core::{StackManifest, StackSource};

use crate::components::{badges::StatusBadge, interactive::InteractiveCard};

use super::{StacksSubTab, StacksView};

impl StacksView {
    pub(super) fn render_browser(
        &mut self,
        is_local: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let stacks: Vec<StackManifest> = if is_local {
            self.workspace.read(cx).local_stacks().to_vec()
        } else {
            self.merged_filtered_stacks(cx)
        };
        let selected_stack = stacks
            .iter()
            .find(|stack| self.selected_stack_id.as_deref() == Some(stack.id.as_str()))
            .or_else(|| stacks.first())
            .cloned();

        let filter_row = if is_local {
            None
        } else {
            let tags = self.available_tags(cx);
            let active_tag = self.active_tag.clone();
            let focus_border = theme.primary;
            Some(
                v_flex()
                    .w_full()
                    .flex_shrink_0()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(Input::new(&self.stack_search_input).small()),
                            )
                            .child(
                                div().flex_shrink_0().child(
                                    Button::new("refresh-community-btn")
                                        .outline()
                                        .small()
                                        .icon(IconName::RefreshCw)
                                        .tooltip(s.btn_refresh_community)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.workspace.update(cx, |ws, cx| {
                                                ws.refresh_community_stacks(false, window, cx)
                                            });
                                        })),
                                ),
                            ),
                    )
                    .when(!tags.is_empty(), |this| {
                        this.child(div().w_full().min_w_0().overflow_x_scrollbar().child(
                            h_flex().gap_1().pb_2().children(
                                std::iter::once(None).chain(tags.into_iter().map(Some)).map(
                                    |tag| {
                                        let is_active = match (&active_tag, &tag) {
                                            (Some(active), Some(tag)) => {
                                                active.eq_ignore_ascii_case(tag)
                                            }
                                            (None, None) => true,
                                            _ => false,
                                        };
                                        let label =
                                            tag.as_deref().unwrap_or(s.filter_all_tags).to_owned();
                                        let id = tag.as_ref().map_or_else(
                                            || "stack-tag-all".to_owned(),
                                            |tag| format!("stack-tag-value-{}", tag.to_lowercase()),
                                        );
                                        div()
                                            .id(id)
                                            .flex_shrink_0()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(theme.background.opacity(0.))
                                            .focusable()
                                            .tab_stop(true)
                                            .focus_visible(|style| style.border_color(focus_border))
                                            .cursor_pointer()
                                            .role(gpui_kit::Role::Button)
                                            .aria_label(label.clone())
                                            .aria_selected(is_active)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.active_tag =
                                                    if is_active { None } else { tag.clone() };
                                                this.stack_scroll
                                                    .scroll_to_item(0, ScrollStrategy::Top);
                                                cx.notify();
                                            }))
                                            .child(
                                                if is_active {
                                                    Tag::primary()
                                                } else {
                                                    Tag::secondary().outline()
                                                }
                                                .small()
                                                .child(label),
                                            )
                                    },
                                ),
                            ),
                        ))
                    }),
            )
        };

        let list_pane = v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .when_some(filter_row, |this, row| this.child(row))
            .child(if stacks.is_empty() {
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .p_3()
                    .child(
                        Empty::new()
                            .header(
                                EmptyHeader::new()
                                    .media(
                                        EmptyMedia::new()
                                            .child(Icon::new(IconName::LayoutDashboard).size_6()),
                                    )
                                    .title(EmptyTitle::new().child(if is_local {
                                        s.empty_local_stacks_title
                                    } else {
                                        s.empty_featured_stacks_title
                                    }))
                                    .description(
                                        EmptyDescription::new().child(s.empty_stacks_desc),
                                    ),
                            )
                            .content(
                                EmptyContent::new().child(
                                    Button::new("empty-create-stack")
                                        .primary()
                                        .small()
                                        .label(s.btn_create_first_stack)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.switch_sub_tab(StacksSubTab::Create, cx);
                                        })),
                                ),
                            ),
                    )
                    .into_any_element()
            } else {
                let selected_id = selected_stack.as_ref().map(|stack| stack.id.clone());
                let scroll = self.stack_scroll.clone();
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "stack-list-pane",
                            stacks.len(),
                            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                range
                                    .map(|index| {
                                        let stack = &stacks[index];
                                        let active = selected_id.as_ref() == Some(&stack.id);
                                        div()
                                            .px_3()
                                            .py_1()
                                            .child(this.render_stack_card(stack, active, cx))
                                    })
                                    .collect::<Vec<_>>()
                            }),
                        )
                        .track_scroll(&scroll)
                        .size_full(),
                    )
                    .vertical_scrollbar(&scroll)
                    .into_any_element()
            });

        let detail_pane = self.render_stack_detail(selected_stack, is_local, cx);

        h_resizable("stacks-split")
            .child(
                resizable_panel()
                    .size(px(304.))
                    .size_range(px(240.)..px(500.))
                    .child(list_pane),
            )
            .child(resizable_panel().child(detail_pane))
    }

    fn render_stack_card(
        &self,
        stack: &StackManifest,
        active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let stack_id = stack.id.clone();
        let metadata = format!(
            "{} {} · {}",
            stack.skills.len(),
            s.skills_unit,
            stack.description
        );

        InteractiveCard::new(format!("stack-card-{stack_id}"), v_flex(), active, cx)
            .accessible(stack.name.clone())
            .h(rems(3.75))
            .w_full()
            .overflow_hidden()
            .px_3()
            .justify_center()
            .gap_1()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected_stack_id = Some(stack_id.clone());
                cx.notify();
            }))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .font_semibold()
                            .truncate()
                            .child(stack.name.clone()),
                    )
                    .child(div().flex_shrink_0().child(if stack.verified {
                        StatusBadge::success(s.badge_verified).outline()
                    } else if stack.source == StackSource::Community {
                        StatusBadge::secondary(s.badge_community).outline()
                    } else {
                        StatusBadge::secondary(s.badge_local)
                    })),
            )
            .child(
                div()
                    .w_full()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .child(metadata),
            )
    }
}
