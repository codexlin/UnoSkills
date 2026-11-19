use gpui_kit::component::{
    ActiveTheme, IconName, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariant, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyTitle},
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    scroll::ScrollableElement,
    v_flex,
};
use gpui_kit::{
    AnyElement, Context, IntoElement, ParentElement, Styled, div, prelude::FluentBuilder,
};
use unoskills_core::StackManifest;

use crate::components::badges::StatusBadge;

use super::StacksView;

impl StacksView {
    pub(super) fn render_stack_detail(
        &mut self,
        stack: Option<StackManifest>,
        is_local: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let Some(stack) = stack else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    Empty::new().header(
                        EmptyHeader::new()
                            .title(EmptyTitle::new().child(s.empty_select_stack_title))
                            .description(EmptyDescription::new().child(s.empty_select_stack_desc)),
                    ),
                )
                .into_any_element();
        };

        let stack_for_install = stack.clone();
        let stack_for_copy = stack.clone();
        let stack_for_share = stack.clone();
        let stack_id_for_delete = stack.id.clone();
        let stack_name_for_delete = stack.name.clone();
        let lang = self.workspace.read(cx).lang();

        div()
            .size_full()
            .min_h_0()
            .overflow_y_scrollbar()
            .id("stack-detail-pane")
            .child(
                v_flex()
                    .w_full()
                    .p_5()
                    .gap_4()
                    .child(
                        v_flex()
                            .w_full()
                            .gap_3()
                            .child(
                                v_flex()
                                    .w_full()
                                    .gap_1()
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .flex_wrap()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_semibold()
                                                    .child(stack.name.clone()),
                                            )
                                            .when(stack.verified, |row| {
                                                row.child(
                                                    StatusBadge::success(s.badge_verified)
                                                        .outline(),
                                                )
                                            }),
                                    )
                                    .child(
                                        div().text_xs().text_color(theme.muted_foreground).child(
                                            format!(
                                                "@{} · id: {}",
                                                if stack.author.is_empty() {
                                                    "community"
                                                } else {
                                                    &stack.author
                                                },
                                                stack.id
                                            ),
                                        ),
                                    )
                                    .child(div().mt_1().text_sm().child(stack.description.clone())),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        Button::new("install-stack-btn")
                                            .primary()
                                            .small()
                                            .icon(IconName::Plus)
                                            .label(s.btn_install_stack)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.workspace.update(cx, |ws, cx| {
                                                    ws.install_stack_async(
                                                        stack_for_install.clone(),
                                                        window,
                                                        cx,
                                                    )
                                                });
                                            })),
                                    )
                                    .child(
                                        Button::new("copy-stack-toml-btn")
                                            .outline()
                                            .small()
                                            .icon(IconName::Copy)
                                            .label(s.btn_copy_toml)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.workspace.update(cx, |ws, cx| {
                                                    ws.copy_stack_toml(&stack_for_copy, window, cx)
                                                });
                                            })),
                                    )
                                    .child(
                                        Button::new("share-stack-gist-btn")
                                            .outline()
                                            .small()
                                            .icon(IconName::ExternalLink)
                                            .label(s.btn_share_gist)
                                            .on_click(cx.listener(
                                                move |this, _, window, cx| {
                                                    this.workspace.update(cx, |ws, cx| {
                                                        ws.share_stack_to_gist(
                                                            stack_for_share.clone(),
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                },
                                            )),
                                    )
                                    .when(is_local, |row| {
                                        row.child(
                                            Button::new("delete-local-stack")
                                                .danger()
                                                .outline()
                                                .small()
                                                .icon(IconName::Delete)
                                                .tooltip(s.btn_delete)
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        let view_entity = cx.entity().clone();
                                                        let ws = this.workspace.clone();
                                                        let stack_id = stack_id_for_delete.clone();
                                                        let title = s.confirm_delete_stack_title;
                                                        let desc = lang.confirm_delete_stack_desc(
                                                            &stack_name_for_delete,
                                                        );
                                                        let ok_label = s.btn_delete;
                                                        let cancel_label = s.btn_cancel;

                                                        window.open_alert_dialog(
                                                            cx,
                                                            move |alert, _, _| {
                                                                let view_entity =
                                                                    view_entity.clone();
                                                                let ws = ws.clone();
                                                                let stack_id = stack_id.clone();
                                                                alert
                                                                    .confirm()
                                                                    .centered()
                                                                    .title(title)
                                                                    .description(desc.clone())
                                                                    .ok_text(ok_label)
                                                                    .ok_variant(
                                                                        ButtonVariant::Danger,
                                                                    )
                                                                    .cancel_text(cancel_label)
                                                                    .on_ok(move |_, window, cx| {
                                                                        ws.update(cx, |ws, cx| {
                                                                            ws.delete_local_stack(
                                                                                &stack_id, window,
                                                                                cx,
                                                                            );
                                                                        });
                                                                        view_entity.update(
                                                                            cx,
                                                                            |v, cx| {
                                                                                v.selected_stack_id =
                                                                                    None;
                                                                                cx.notify();
                                                                            },
                                                                        );
                                                                        true
                                                                    })
                                                            },
                                                        );
                                                    },
                                                )),
                                        )
                                    }),
                            ),
                    )
                    .when(!stack.recommended_agents.is_empty(), |this| {
                        this.child(
                            h_flex()
                                .flex_wrap()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_semibold()
                                        .text_color(theme.muted_foreground)
                                        .child(s.recommended_agents),
                                )
                                .children(
                                    stack
                                        .recommended_agents
                                        .iter()
                                        .map(|a| StatusBadge::secondary(a.clone())),
                                ),
                        )
                    })
                    .child(
                        GroupBox::new()
                            .outline()
                            .title(format!("{} ({})", s.included_skills, stack.skills.len()))
                            .child(v_flex().gap_2().children(stack.skills.iter().map(
                                |skill_ref| {
                                    let installed =
                                        self.workspace.read(cx).skills().iter().any(|local| {
                                            local.name.eq_ignore_ascii_case(&skill_ref.name)
                                                || local.slug.eq_ignore_ascii_case(&skill_ref.name)
                                        });
                                    v_flex()
                                        .p_3()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(theme.border)
                                        .bg(theme.secondary.opacity(0.3))
                                        .gap_1()
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
                                                        .child(skill_ref.name.clone()),
                                                )
                                                .child(div().flex_shrink_0().child(if installed {
                                                    StatusBadge::success(s.already_in_library)
                                                        .outline()
                                                } else {
                                                    StatusBadge::info(s.will_install).outline()
                                                })),
                                        )
                                        .when(!skill_ref.note.is_empty(), |box_el| {
                                            box_el.child(
                                                div().text_xs().child(skill_ref.note.clone()),
                                            )
                                        })
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .truncate()
                                                .child(skill_ref.source.clone()),
                                        )
                                },
                            ))),
                    ),
            )
            .into_any_element()
    }
}
