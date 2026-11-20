use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, StyledExt, WindowExt,
    button::{Button, ButtonVariant, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyTitle},
    h_flex,
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
    v_flex,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, div,
    prelude::FluentBuilder, px,
};
use unoskills_core::{Skill, StackManifest};

use super::StacksView;
use crate::components::badges::StatusBadge;

impl StacksView {
    pub(super) fn render_stack_detail(
        &self,
        stack: Option<StackManifest>,
        is_local: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ws = self.workspace.read(cx);
        let lang = ws.lang();
        let s = lang.t();
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
        let theme = cx.theme();
        let border = theme.border;
        let muted = theme.muted_foreground;
        let installing = ws.is_stack_installing(&stack.id);
        let blocking_member = stack
            .skills
            .iter()
            .any(|item| ws.is_source_installing(&item.source));
        let members: Vec<_> = stack
            .skills
            .iter()
            .map(|item| (item, installed_skill(ws.skills(), &item.name)))
            .collect();
        let existing = members.iter().filter(|(_, skill)| skill.is_some()).count();
        let to_install = members.len() - existing;
        let failures = ws.stack_install_failures(&stack.id);
        let saved = ws.local_stacks().iter().any(|local| local.id == stack.id);
        let summary = lang.stack_install_counts(to_install, existing);
        let can_submit = !blocking_member && (to_install > 0 || !saved);
        let install_label = if to_install > 0 {
            s.market_add_to_library
        } else {
            s.btn_save_stack
        };
        let install_stack = stack.clone();

        v_flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(theme.group_box)
            .child(
                div()
                    .id("stack-detail-pane")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .w_full()
                            .p_5()
                            .gap_4()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_start()
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap_2()
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .text_lg()
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
                                            .when(!stack.description.is_empty(), |pane| {
                                                pane.child(
                                                    div()
                                                        .text_sm()
                                                        .child(stack.description.clone()),
                                                )
                                            }),
                                    )
                                    .child(self.stack_actions(&stack, is_local, cx)),
                            )
                            .when(!stack.recommended_agents.is_empty(), |pane| {
                                pane.child(
                                    h_flex()
                                        .gap_2()
                                        .flex_wrap()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(s.recommended_agents)
                                        .child(stack.recommended_agents.join(", ")),
                                )
                            })
                            .child(div().text_sm().font_semibold().child(format!(
                                "{} ({})",
                                s.included_skills,
                                members.len()
                            )))
                            .child(v_flex().border_t_1().border_color(border).children(
                                members.into_iter().map(|(item, installed)| {
                                    let failure =
                                        failures.iter().find(|(name, _)| name == &item.name);
                                    let conflict = installed
                                        .and_then(|skill| skill.source_ref.as_deref())
                                        .is_some_and(|source| {
                                            !source.eq_ignore_ascii_case(item.source.trim())
                                        });
                                    let status = if conflict {
                                        s.market_same_name
                                    } else if installed.is_some() {
                                        s.already_in_library
                                    } else {
                                        s.will_install
                                    };
                                    v_flex()
                                        .min_h(px(40.))
                                        .justify_center()
                                        .py_2()
                                        .gap_1()
                                        .border_b_1()
                                        .border_color(border)
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .child(
                                                    v_flex()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_semibold()
                                                                .truncate()
                                                                .child(item.name.clone()),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(muted)
                                                                .truncate()
                                                                .child(item.source.clone()),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .text_xs()
                                                        .text_color(if conflict {
                                                            theme.warning
                                                        } else if installed.is_some() {
                                                            theme.success
                                                        } else {
                                                            muted
                                                        })
                                                        .child(status),
                                                ),
                                        )
                                        .when_some(failure, |row, (_, error)| {
                                            row.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.danger)
                                                    .child(error.clone()),
                                            )
                                        })
                                }),
                            )),
                    ),
            )
            .child(
                h_flex()
                    .min_h(px(56.))
                    .px_5()
                    .py_2()
                    .gap_2()
                    .border_t_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(muted)
                            .child(summary.clone()),
                    )
                    .child(
                        Button::new("install-stack-btn")
                            .primary()
                            .label(install_label)
                            .loading(installing)
                            .disabled(!installing && !can_submit)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let workspace = this.workspace.downgrade();
                                let stack = install_stack.clone();
                                let summary = summary.clone();
                                window.open_alert_dialog(cx, move |alert, _, _| {
                                    let workspace = workspace.clone();
                                    let stack = stack.clone();
                                    alert
                                        .confirm()
                                        .centered()
                                        .title(stack.name.clone())
                                        .description(summary.clone())
                                        .ok_text(install_label)
                                        .cancel_text(s.btn_cancel)
                                        .on_ok(move |_, window, cx| {
                                            let _ = workspace.update(cx, |ws, cx| {
                                                ws.install_stack_async(stack.clone(), window, cx)
                                            });
                                            true
                                        })
                                });
                            })),
                    ),
            )
            .into_any_element()
    }

    fn stack_actions(
        &self,
        stack: &StackManifest,
        is_local: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let lang = self.workspace.read(cx).lang();
        let s = lang.t();
        let ws = self.workspace.downgrade();
        let view = cx.weak_entity();
        let stack = stack.clone();
        Button::new("stack-actions")
            .ghost()
            .icon(IconName::Ellipsis)
            .accessibility_label(s.stack_actions)
            .tooltip(s.stack_actions)
            .dropdown_menu(move |menu, _, _| {
                let workspace = ws.clone();
                let to_copy = stack.clone();
                let share_ws = ws.clone();
                let to_share = stack.clone();
                let menu = menu
                    .item(
                        PopupMenuItem::new(s.btn_copy_toml).on_click(move |_, window, cx| {
                            let _ = workspace
                                .update(cx, |ws, cx| ws.copy_stack_toml(&to_copy, window, cx));
                        }),
                    )
                    .item(
                        PopupMenuItem::new(s.btn_share_gist).on_click(move |_, window, cx| {
                            let _ = share_ws.update(cx, |ws, cx| {
                                ws.share_stack_to_gist(to_share.clone(), window, cx)
                            });
                        }),
                    );
                if !is_local {
                    return menu;
                }
                let ws = ws.clone();
                let view = view.clone();
                let id = stack.id.clone();
                let description = lang.confirm_delete_stack_desc(&stack.name);
                menu.separator()
                    .item(
                        PopupMenuItem::new(s.btn_delete).on_click(move |_, window, cx| {
                            let workspace = ws.clone();
                            let view = view.clone();
                            let id = id.clone();
                            let description = description.clone();
                            window.open_alert_dialog(cx, move |alert, _, _| {
                                let workspace = workspace.clone();
                                let view = view.clone();
                                let id = id.clone();
                                alert
                                    .confirm()
                                    .centered()
                                    .title(s.confirm_delete_stack_title)
                                    .description(description.clone())
                                    .ok_text(s.btn_delete)
                                    .ok_variant(ButtonVariant::Danger)
                                    .cancel_text(s.btn_cancel)
                                    .on_ok(move |_, window, cx| {
                                        let _ = workspace.update(cx, |ws, cx| {
                                            ws.delete_local_stack(&id, window, cx)
                                        });
                                        let _ = view.update(cx, |view, cx| {
                                            view.selected_stack_id = None;
                                            cx.notify();
                                        });
                                        true
                                    })
                            });
                        }),
                    )
            })
    }
}

fn installed_skill<'a>(skills: &'a [Skill], key: &str) -> Option<&'a Skill> {
    let query = key.trim().to_lowercase();
    skills.iter().find(|skill| {
        skill.id.to_lowercase() == query
            || skill.slug.to_lowercase() == query
            || skill.name.to_lowercase() == query
    })
}
