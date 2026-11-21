use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt, WindowExt,
    alert::Alert,
    button::{Button, ButtonVariant, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex,
    input::Textarea,
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
    spinner::Spinner,
    switch::Switch,
    text::TextView,
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{
    ClipboardItem, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use unoskills_core::{ProjectSkillPlacement, Skill};

use crate::components::page_header::page_tabs;

use super::LibraryView;

impl LibraryView {
    pub(super) fn render_skill_inspector(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let Some(skill) = self.current_skill(cx).cloned() else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    Empty::new().header(
                        EmptyHeader::new()
                            .media(EmptyMedia::new().child(Icon::new(IconName::FileText).size_6()))
                            .title(EmptyTitle::new().child(s.empty_select_skill_title))
                            .description(EmptyDescription::new().child(s.empty_select_skill_desc)),
                    ),
                )
                .into_any_element();
        };
        let editing = self.editing_markdown;
        let preview = self.preview_tab;
        let border = theme.border;
        let muted = theme.muted_foreground;
        let lint = self
            .selected_lint
            .clone()
            .filter(|report| !report.warnings.is_empty());
        let source = skill
            .source_ref
            .as_deref()
            .unwrap_or_else(|| skill.source_type.as_str());
        let meta = format!("{source} · ~{} tokens", skill.estimated_tokens);

        v_flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(theme.group_box)
            .child(
                h_flex()
                    .px_4()
                    .py_3()
                    .gap_2()
                    .items_center()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .truncate()
                                    .child(skill.name.clone()),
                            )
                            .child(
                                div()
                                    .id("library-skill-source")
                                    .text_xs()
                                    .text_color(muted)
                                    .truncate()
                                    .tooltip({
                                        let meta = meta.clone();
                                        move |window, cx| {
                                            Tooltip::new(meta.clone()).build(window, cx)
                                        }
                                    })
                                    .child(meta),
                            ),
                    )
                    .when(editing, |row| {
                        row.child(
                            Button::new("cancel-edit-md")
                                .ghost()
                                .label(s.btn_cancel)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.handle_cancel_edit_shortcut(window, cx)
                                })),
                        )
                        .child(
                            Button::new("save-skill-md")
                                .primary()
                                .label(s.btn_save)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.save_markdown(window, cx)
                                })),
                        )
                    })
                    .when(!editing, |row| {
                        row.when(preview, |row| {
                            row.child(
                                Button::new("edit-skill-md")
                                    .outline()
                                    .label(s.btn_edit_skill)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.editing_markdown = true;
                                        cx.notify();
                                    })),
                            )
                        })
                        .child(self.skill_actions(&skill, cx))
                    }),
            )
            .when(!editing, |pane| {
                pane.child(
                    page_tabs(
                        "skill-inspector-tabs",
                        [
                            (AssetIcon::Network, s.library_deployment),
                            (AssetIcon::FileText, s.library_preview),
                        ],
                        usize::from(preview),
                        cx,
                    )
                    .px_4()
                    .on_click(cx.listener(|view, index: &usize, _, cx| {
                        view.preview_tab = *index == 1;
                        cx.notify();
                    })),
                )
            })
            .child(if editing {
                div()
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .overflow_hidden()
                    .child(Textarea::new(&self.markdown_editor).size_full())
                    .into_any_element()
            } else {
                div()
                    .id("skill-inspector-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .w_full()
                            .p_4()
                            .gap_3()
                            .when_some(lint.filter(|_| preview), |pane, report| {
                                pane.child(
                                    Alert::warning("skill-lint-alert", report.warnings.join(" · "))
                                        .banner()
                                        .small(),
                                )
                            })
                            .child(if preview {
                                v_flex()
                                    .w_full()
                                    .max_w(px(720.))
                                    .gap_3()
                                    .when_some(
                                        skill
                                            .description
                                            .clone()
                                            .filter(|text| !text.trim().is_empty()),
                                        |pane, text| {
                                            pane.child(
                                                div()
                                                    .pb_3()
                                                    .border_b_1()
                                                    .border_color(border)
                                                    .text_sm()
                                                    .child(text),
                                            )
                                        },
                                    )
                                    .child(
                                        TextView::new(&self.markdown_preview_state)
                                            .selectable(true),
                                    )
                                    .into_any_element()
                            } else {
                                self.deployment_rows(&skill, cx).into_any_element()
                            }),
                    )
                    .into_any_element()
            })
            .into_any_element()
    }

    fn skill_actions(&self, skill: &Skill, cx: &Context<Self>) -> impl IntoElement + use<> {
        let ws = self.workspace.read(cx);
        let lang = ws.lang();
        let s = lang.t();
        let can_update = skill.source_type != unoskills_core::SourceType::Adopted
            && skill
                .source_ref
                .as_deref()
                .is_some_and(|reference| !reference.is_empty());
        let updating = ws.updating_skill_id() == Some(skill.id.as_str());
        let workspace = self.workspace.downgrade();
        let skill_id = skill.id.clone();
        let name = skill.name.clone();
        let path = skill.central_path.clone();
        let deployments = skill.deployments.len() + skill.project_deployments.len();

        Button::new("library-skill-actions")
            .ghost()
            .icon(IconName::Ellipsis)
            .tooltip(s.library_skill_actions)
            .accessibility_label(s.library_skill_actions)
            .dropdown_menu(move |mut menu, _, _| {
                if can_update {
                    let workspace = workspace.clone();
                    let id = skill_id.clone();
                    let name = name.clone();
                    menu = menu.item(
                        PopupMenuItem::new(s.btn_update_skill)
                            .disabled(updating)
                            .on_click(move |_, window, cx| {
                                let _ = workspace.update(cx, |ws, cx| {
                                    ws.update_skill_async(id.clone(), name.clone(), window, cx)
                                });
                            }),
                    );
                }
                let path_to_reveal = path.clone();
                let path_to_copy = path.to_string_lossy().into_owned();
                let workspace = workspace.clone();
                let id = skill_id.clone();
                let description = lang.confirm_delete_skill_desc(&name, deployments);
                menu.item(
                    PopupMenuItem::new(s.tooltip_reveal_folder)
                        .on_click(move |_, _, cx| cx.reveal_path(&path_to_reveal)),
                )
                .item(
                    PopupMenuItem::new(s.library_copy_path).on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(path_to_copy.clone()));
                    }),
                )
                .separator()
                .item(
                    PopupMenuItem::new(s.btn_delete).on_click(move |_, window, cx| {
                        let workspace = workspace.clone();
                        let id = id.clone();
                        let description = description.clone();
                        window.open_alert_dialog(cx, move |alert, _, _| {
                            let workspace = workspace.clone();
                            let id = id.clone();
                            alert
                                .confirm()
                                .centered()
                                .title(s.confirm_delete_skill_title)
                                .description(description.clone())
                                .ok_text(s.btn_delete)
                                .ok_variant(ButtonVariant::Danger)
                                .cancel_text(s.btn_cancel)
                                .on_ok(move |_, window, cx| {
                                    let _ = workspace
                                        .update(cx, |ws, cx| ws.remove_skill(&id, window, cx));
                                    true
                                })
                        });
                    }),
                )
            })
    }

    fn deployment_rows(&self, skill: &Skill, cx: &Context<Self>) -> impl IntoElement + use<> {
        let ws = self.workspace.read(cx);
        let lang = ws.lang();
        let s = lang.t();
        let theme = cx.theme();
        let border = theme.border;
        let muted = theme.muted_foreground;
        let project = ws
            .projects()
            .iter()
            .find(|project| Some(&project.id) == self.selected_project_id.as_ref());
        let scope = project.map_or_else(
            || s.scope_global.to_string(),
            |project| project.name.clone(),
        );
        let rows: Vec<_> = ws
            .agents()
            .iter()
            .filter(|agent| agent.installed && agent.enabled)
            .filter_map(|agent| {
                let skill_id = skill.id.clone();
                let agent_key = agent.key.clone();
                let row = h_flex()
                    .id(format!("deployment-row-{}", agent.key))
                    .min_h(px(36.))
                    .gap_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .truncate()
                            .child(agent.display_name.clone()),
                    );
                let Some(project) = self.selected_project_id.clone() else {
                    let deployed = skill.is_deployed_to(&agent.key);
                    let state_label = if deployed {
                        s.library_deployed
                    } else {
                        s.library_not_deployed
                    };
                    return Some(
                        row.child(
                            Switch::new(format!("deploy-{}-{}", skill.id, agent.key))
                                .flex_shrink_0()
                                .checked(deployed)
                                .accessibility_label(format!(
                                    "{} · {} · {scope}",
                                    s.library_deployment, agent.display_name,
                                ))
                                .tooltip(format!("{state_label} · {scope}"))
                                .on_change(cx.listener(move |view, checked: &bool, window, cx| {
                                    if *checked == deployed {
                                        return;
                                    }
                                    view.workspace.update(cx, |ws, cx| {
                                        ws.toggle_skill_deployment(
                                            &skill_id, &agent_key, deployed, window, cx,
                                        );
                                    });
                                })),
                        ),
                    );
                };

                use ProjectSkillPlacement::{Conflict, Local, Managed, Missing, Unavailable};
                let status = self.project_status(skill, &agent.key)?;
                let state_label = match status.placement {
                    Managed => s.library_project_deployed,
                    Local => s.library_project_local,
                    Conflict => s.library_project_conflict,
                    Unavailable => s.library_project_unavailable,
                    Missing if status.global_available => s.library_global_available,
                    Missing => s.library_not_deployed,
                };
                let label_color = match status.placement {
                    Conflict | Unavailable => theme.warning,
                    Managed | Local => theme.foreground,
                    Missing => muted,
                };
                let row = row.child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(label_color)
                        .child(gpui_kit::text!(state_label)),
                );
                Some(match status.placement {
                    Managed => row.child(
                        Switch::new(format!("project-deploy-{}-{}", skill.id, agent.key))
                            .flex_shrink_0()
                            .checked(true)
                            .accessibility_label(format!(
                                "{} · {}",
                                s.library_project_deployed, agent.display_name
                            ))
                            .tooltip(s.library_remove_project_deployment)
                            .on_change(cx.listener(move |view, checked: &bool, window, cx| {
                                if !checked {
                                    view.workspace.update(cx, |ws, cx| {
                                        ws.toggle_project_skill_deployment(
                                            &skill_id, &project, &agent_key, true, window, cx,
                                        );
                                    });
                                }
                            })),
                    ),
                    Missing | Conflict => {
                        let action = if status.placement == Conflict {
                            s.library_resolve_project_conflict
                        } else {
                            s.stack_deploy_project
                        };
                        row.child(
                            Button::new(format!("project-deploy-{}-{}", skill.id, agent.key))
                                .flex_shrink_0()
                                .small()
                                .ghost()
                                .label(action)
                                .accessibility_label(format!("{action} · {}", agent.display_name))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.workspace.update(cx, |ws, cx| {
                                        ws.toggle_project_skill_deployment(
                                            &skill_id, &project, &agent_key, false, window, cx,
                                        );
                                    });
                                })),
                        )
                    }
                    Local | Unavailable => row,
                })
            })
            .collect();

        v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_semibold()
                            .child(s.library_deployment_targets),
                    )
                    .child(
                        div()
                            .max_w(px(240.))
                            .min_w_0()
                            .text_xs()
                            .text_color(muted)
                            .truncate()
                            .child(scope),
                    ),
            )
            .when_some(project.cloned(), |panel, project| {
                let path = project.path.clone();
                let description = lang
                    .confirm_detach_project_desc(&project.name, &project.path.to_string_lossy());
                panel.child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .text_color(muted)
                                .truncate()
                                .child(path.to_string_lossy().into_owned()),
                        )
                        .child(
                            Button::new("reveal-project-dir")
                                .ghost()
                                .small()
                                .icon(IconName::FolderOpen)
                                .tooltip(s.tooltip_reveal_folder)
                                .accessibility_label(s.tooltip_reveal_folder)
                                .on_click(move |_, _, cx| cx.reveal_path(&path)),
                        )
                        .child(
                            Button::new("detach-project-btn")
                                .ghost()
                                .small()
                                .label(s.btn_detach_project)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    let workspace = view.workspace.downgrade();
                                    let description = description.clone();
                                    let id = project.id.clone();
                                    window.open_alert_dialog(cx, move |alert, _, _| {
                                        let workspace = workspace.clone();
                                        let id = id.clone();
                                        alert
                                            .confirm()
                                            .centered()
                                            .title(s.confirm_detach_project_title)
                                            .description(description.clone())
                                            .cancel_text(s.btn_cancel)
                                            .ok_text(s.btn_detach_project)
                                            .ok_variant(ButtonVariant::Danger)
                                            .on_ok(move |_, window, cx| {
                                                let _ = workspace.update(cx, |ws, cx| {
                                                    ws.remove_project(&id, window, cx)
                                                });
                                                true
                                            })
                                    });
                                })),
                        ),
                )
            })
            .when(
                !ws.agents()
                    .iter()
                    .any(|agent| agent.installed && agent.enabled),
                |panel| {
                    panel.child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(s.no_installed_agents),
                    )
                },
            )
            .when(self.selected_project_id.is_some(), |panel| {
                match &self.project_skill_statuses {
                    None => panel.child(
                        h_flex()
                            .gap_2()
                            .text_sm()
                            .text_color(muted)
                            .child(Spinner::new().small())
                            .child(s.stack_project_checking),
                    ),
                    Some(Err(error)) => {
                        panel.child(Alert::error("library-project-error", error.clone()).banner())
                    }
                    Some(Ok(_)) => panel,
                }
            })
            .child(v_flex().border_t_1().border_color(border).children(rows))
    }
}
