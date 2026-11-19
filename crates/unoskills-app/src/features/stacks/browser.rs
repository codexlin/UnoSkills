use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex, h_resizable, resizable_panel,
    scroll::ScrollableElement,
    v_flex,
};
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use unoskills_core::StackManifest;

use crate::components::badges::StatusBadge;

use super::{StacksSubTab, StacksView};

impl StacksView {
    pub(super) fn render_browser(
        &mut self,
        is_local: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let stacks: &[StackManifest] = if is_local {
            self.workspace.read(cx).local_stacks()
        } else {
            self.workspace.read(cx).featured_stacks()
        };
        let selected_stack = stacks
            .iter()
            .find(|stack| self.selected_stack_id.as_deref() == Some(stack.id.as_str()))
            .or_else(|| stacks.first())
            .cloned();

        let list_pane = div()
            .size_full()
            .min_h_0()
            .overflow_y_scrollbar()
            .id("stack-list-pane")
            .child(
                v_flex()
                    .w_full()
                    .p_3()
                    .gap_2()
                    .when(stacks.is_empty(), |this| {
                        this.child(
                            Empty::new()
                                .header(
                                    EmptyHeader::new()
                                        .media(
                                            EmptyMedia::new().child(
                                                Icon::new(IconName::LayoutDashboard).size_6(),
                                            ),
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
                                                this.switch_sub_tab(StacksSubTab::CreateImport, cx);
                                            })),
                                    ),
                                ),
                        )
                    })
                    .children(stacks.iter().map(|stack| {
                        let active = selected_stack
                            .as_ref()
                            .is_some_and(|selected| selected.id == stack.id);
                        let stack_id = stack.id.clone();
                        v_flex()
                            .id(SharedString::from(format!("stack-card-{}", stack.id)))
                            .role(gpui_kit::Role::ListItem)
                            .aria_label(stack.name.clone())
                            .aria_selected(active)
                            .p_3()
                            .rounded_md()
                            .border_1()
                            .border_color(if active { theme.primary } else { theme.border })
                            .bg(if active {
                                theme.accent
                            } else {
                                theme.background
                            })
                            .cursor_pointer()
                            .gap_2()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selected_stack_id = Some(stack_id.clone());
                                cx.notify();
                            }))
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
                                            .font_semibold()
                                            .truncate()
                                            .child(stack.name.clone()),
                                    )
                                    .child(div().flex_shrink_0().child(if stack.verified {
                                        StatusBadge::success(s.badge_verified).outline()
                                    } else {
                                        StatusBadge::secondary(s.badge_local)
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(stack.description.clone()),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .truncate()
                                            .child(format!(
                                                "@{} · {} {}",
                                                if stack.author.is_empty() {
                                                    "community"
                                                } else {
                                                    &stack.author
                                                },
                                                stack.skills.len(),
                                                s.skills_unit
                                            )),
                                    )
                                    .child(
                                        h_flex().flex_shrink_0().gap_1().children(
                                            stack.tags.iter().take(3).map(|t| {
                                                StatusBadge::secondary(t.clone()).outline()
                                            }),
                                        ),
                                    ),
                            )
                    })),
            );

        let detail_pane = self.render_stack_detail(selected_stack, is_local, cx);

        h_resizable("stacks-split")
            .child(
                resizable_panel()
                    .size(px(320.))
                    .size_range(px(240.)..px(500.))
                    .child(list_pane),
            )
            .child(resizable_panel().child(detail_pane))
    }
}
