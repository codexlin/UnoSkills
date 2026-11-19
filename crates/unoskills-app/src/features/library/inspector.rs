use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt, WindowExt,
    alert::Alert,
    button::{Button, ButtonVariant, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    input::Textarea,
    scroll::ScrollableElement,
    switch::Switch,
    text::TextView,
    v_flex,
};
use gpui_kit::{
    Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::FluentBuilder, px,
};

use crate::components::badges::StatusBadge;

use super::LibraryView;

impl LibraryView {
    pub(super) fn render_skill_inspector(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let lang = self.workspace.read(cx).lang();
        let s = lang.t();
        let Some(skill) = self.current_skill(cx).cloned() else {
            return v_flex().size_full().items_center().justify_center().child(
                Empty::new().header(
                    EmptyHeader::new()
                        .media(EmptyMedia::new().child(Icon::new(IconName::FileText).size_6()))
                        .title(EmptyTitle::new().child(s.empty_select_skill_title))
                        .description(EmptyDescription::new().child(s.empty_select_skill_desc)),
                ),
            );
        };

        let installed_agents: Vec<_> = self
            .workspace
            .read(cx)
            .agents()
            .iter()
            .filter(|a| a.installed && a.enabled)
            .cloned()
            .collect();
        let projects: Vec<_> = self.workspace.read(cx).projects().to_vec();
        let active_project = self
            .selected_project_id
            .as_ref()
            .and_then(|pid| projects.iter().find(|p| &p.id == pid))
            .cloned();
        let lint = self.selected_lint.clone();
        let editing = self.editing_markdown;
        let skill_id_for_delete = skill.id.clone();
        let skill_name_for_delete = skill.name.clone();
        let deploy_count_for_delete = skill.deployments.len() + skill.project_deployments.len();
        let can_update = skill.source_type != unoskills_core::SourceType::Adopted
            && skill.source_ref.as_deref().is_some_and(|r| !r.is_empty());
        let is_updating = self.workspace.read(cx).updating_skill_id() == Some(skill.id.as_str());
        let skill_id_for_update = skill.id.clone();
        let skill_name_for_update = skill.name.clone();
        let central_path_for_reveal = skill.central_path.clone();

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                h_flex()
                    .w_full()
                    .px_5()
                    .py_3()
                    .border_b_1()
                    .border_color(theme.border)
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
                                    .flex_wrap()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div().text_lg().font_semibold().child(skill.name.clone()),
                                    )
                                    .child(StatusBadge::secondary(skill.source_type.as_str()))
                                    .child(
                                        StatusBadge::info(format!(
                                            "~{} tokens",
                                            skill.estimated_tokens
                                        ))
                                        .outline(),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(skill.central_path.to_string_lossy().to_string()),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap_2()
                            .child(if editing {
                                Button::new("save-skill-md")
                                    .primary()
                                    .small()
                                    .icon(IconName::Check)
                                    .label(s.btn_save)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_markdown(window, cx);
                                    }))
                            } else {
                                Button::new("edit-skill-md")
                                    .outline()
                                    .small()
                                    .icon(IconName::FileText)
                                    .label(s.btn_edit_skill)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.editing_markdown = true;
                                        cx.notify();
                                    }))
                            })
                            .when(editing, |row| {
                                row.child(
                                    Button::new("cancel-edit-md")
                                        .ghost()
                                        .small()
                                        .label(s.btn_cancel)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.editing_markdown = false;
                                            this.reload_selected_details(false, window, cx);
                                            cx.notify();
                                        })),
                                )
                            })
                            .when(!editing && can_update, |row| {
                                row.child(
                                    Button::new("update-skill-btn")
                                        .outline()
                                        .small()
                                        .icon(IconName::RefreshCw)
                                        .loading(is_updating)
                                        .label(s.btn_update_skill)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.workspace.update(cx, |ws, cx| {
                                                ws.update_skill_async(
                                                    skill_id_for_update.clone(),
                                                    skill_name_for_update.clone(),
                                                    window,
                                                    cx,
                                                );
                                            });
                                        })),
                                )
                            })
                            .child(
                                Button::new("reveal-skill-dir")
                                    .ghost()
                                    .small()
                                    .icon(IconName::FolderOpen)
                                    .tooltip(s.tooltip_reveal_folder)
                                    .on_click(move |_, _, cx| {
                                        cx.reveal_path(&central_path_for_reveal);
                                    }),
                            )
                            .child(
                                Button::new("delete-skill")
                                    .danger()
                                    .outline()
                                    .small()
                                    .icon(IconName::Delete)
                                    .tooltip(s.tooltip_remove_skill)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let ws = this.workspace.clone();
                                        let skill_id = skill_id_for_delete.clone();
                                        let title = s.confirm_delete_skill_title;
                                        let desc = lang.confirm_delete_skill_desc(
                                            &skill_name_for_delete,
                                            deploy_count_for_delete,
                                        );
                                        let ok_label = s.btn_delete;
                                        let cancel_label = s.btn_cancel;

                                        window.open_alert_dialog(cx, move |alert, _, _| {
                                            let ws = ws.clone();
                                            let skill_id = skill_id.clone();
                                            alert
                                                .confirm()
                                                .centered()
                                                .title(title)
                                                .description(desc.clone())
                                                .ok_text(ok_label)
                                                .ok_variant(ButtonVariant::Danger)
                                                .cancel_text(cancel_label)
                                                .on_ok(move |_, window, cx| {
                                                    ws.update(cx, |ws, cx| {
                                                        ws.remove_skill(&skill_id, window, cx);
                                                    });
                                                    true
                                                })
                                        });
                                    })),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .px_5()
                    .py_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.secondary.opacity(0.4))
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(theme.muted_foreground)
                                            .child(s.deploy_to_agents),
                                    )
                                    .child({
                                        let is_global = active_project.is_none();
                                        let btn = Button::new("scope-global")
                                            .xsmall()
                                            .icon(IconName::Globe)
                                            .label(s.scope_global)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.selected_project_id = None;
                                                cx.notify();
                                            }));
                                        if is_global {
                                            btn.primary()
                                        } else {
                                            btn.outline()
                                        }
                                    })
                                    .children(projects.into_iter().map(|proj| {
                                        let is_active = active_project
                                            .as_ref()
                                            .is_some_and(|ap| ap.id == proj.id);
                                        let pid = proj.id.clone();
                                        let path_tip = proj.path.to_string_lossy().to_string();
                                        let btn = Button::new(SharedString::from(format!(
                                            "scope-project-{}",
                                            proj.id
                                        )))
                                        .xsmall()
                                        .icon(IconName::Folder)
                                        .label(proj.name)
                                        .tooltip(path_tip)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.selected_project_id = Some(pid.clone());
                                            cx.notify();
                                        }));
                                        if is_active {
                                            btn.primary()
                                        } else {
                                            btn.outline()
                                        }
                                    }))
                                    .child(
                                        Button::new("scope-add-project")
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::Plus)
                                            .label(s.btn_add_project)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.pick_and_attach_project(window, cx);
                                            })),
                                    ),
                            ),
                    )
                    .when_some(active_project.clone(), |bar, proj| {
                        let proj_path_str = proj.path.to_string_lossy().to_string();
                        let proj_path_reveal = proj.path.clone();
                        let proj_id_detach = proj.id.clone();
                        let proj_name_detach = proj.name.clone();
                        bar.child(
                            h_flex()
                                .w_full()
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
                                        .child(proj_path_str.clone()),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            Button::new("reveal-project-dir")
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::FolderOpen)
                                                .tooltip(s.tooltip_reveal_folder)
                                                .on_click(move |_, _, cx| {
                                                    cx.reveal_path(&proj_path_reveal);
                                                }),
                                        )
                                        .child(
                                            Button::new("detach-project-btn")
                                                .ghost()
                                                .danger()
                                                .xsmall()
                                                .icon(IconName::Delete)
                                                .label(s.btn_detach_project)
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        let ws = this.workspace.clone();
                                                        let pid = proj_id_detach.clone();
                                                        let title = s.confirm_detach_project_title;
                                                        let desc = lang
                                                            .confirm_detach_project_desc(
                                                                &proj_name_detach,
                                                                &proj_path_str,
                                                            );
                                                        let ok_label = s.btn_detach_project;
                                                        let cancel_label = s.btn_cancel;

                                                        window.open_alert_dialog(
                                                            cx,
                                                            move |alert, _, _| {
                                                                let ws = ws.clone();
                                                                let pid = pid.clone();
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
                                                                            ws.remove_project(
                                                                                &pid, window, cx,
                                                                            );
                                                                        });
                                                                        true
                                                                    })
                                                            },
                                                        );
                                                    },
                                                )),
                                        ),
                                ),
                        )
                    })
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .when(installed_agents.is_empty(), |row| {
                                row.child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(s.no_installed_agents),
                                )
                            })
                            .children(installed_agents.into_iter().map(|agent| {
                                let active_pid = active_project.as_ref().map(|p| p.id.clone());
                                let deployed = match active_pid.as_deref() {
                                    Some(pid) => skill.is_deployed_to_project(pid, &agent.key),
                                    None => skill.is_deployed_to(&agent.key),
                                };
                                let switch_id = match active_pid.as_deref() {
                                    Some(pid) => {
                                        format!("deploy-proj-{pid}-{}-{}", skill.id, agent.key)
                                    }
                                    None => format!("deploy-{}-{}", skill.id, agent.key),
                                };
                                let skill_id = skill.id.clone();
                                let agent_key = agent.key.clone();
                                h_flex()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(if deployed {
                                        theme.primary
                                    } else {
                                        theme.border
                                    })
                                    .bg(theme.background)
                                    .items_center()
                                    .child(
                                        Switch::new(SharedString::from(switch_id))
                                            .small()
                                            .checked(deployed)
                                            .label(agent.display_name.clone())
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                let pid = active_pid.clone();
                                                this.workspace.update(cx, |ws, cx| {
                                                    if let Some(pid) = pid.as_deref() {
                                                        ws.toggle_project_skill_deployment(
                                                            &skill_id, pid, &agent_key, deployed,
                                                            window, cx,
                                                        );
                                                    } else {
                                                        ws.toggle_skill_deployment(
                                                            &skill_id, &agent_key, deployed,
                                                            window, cx,
                                                        );
                                                    }
                                                });
                                            })),
                                    )
                            })),
                    ),
            )
            .when_some(lint.filter(|l| !l.warnings.is_empty()), |pane, report| {
                pane.child(
                    Alert::warning("skill-lint-alert", report.warnings.join(" · "))
                        .banner()
                        .small(),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .overflow_y_scrollbar()
                    .id("skill-md-pane")
                    .child(
                        v_flex()
                            .w_full()
                            .p_5()
                            .gap_4()
                            .when(!editing, |pane| {
                                pane.when_some(skill.description.clone(), |pane, desc| {
                                    pane.child(
                                        GroupBox::new()
                                            .outline()
                                            .title(s.section_description)
                                            .child(div().text_sm().child(desc)),
                                    )
                                })
                            })
                            .child(if editing {
                                Textarea::new(&self.markdown_editor)
                                    .h(px(460.))
                                    .into_any_element()
                            } else {
                                TextView::new(&self.markdown_preview_state)
                                    .selectable(true)
                                    .into_any_element()
                            }),
                    ),
            )
    }
}
