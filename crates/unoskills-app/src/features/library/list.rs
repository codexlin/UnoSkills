use std::ops::Range;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, StyledExt,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
    v_flex,
};
use gpui_kit::{
    Context, IntoElement, ParentElement, Styled, div, prelude::FluentBuilder, px, uniform_list,
};

use crate::components::badges::TokenBadge;
use crate::components::interactive::InteractiveCard;
use crate::state::NavTab;

use super::LibraryView;

impl LibraryView {
    pub(super) fn render_library_toolbar(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let projects = ws.projects().to_vec();
        let selected_project = self.selected_project_id.clone();
        let scope = projects
            .iter()
            .find(|project| Some(&project.id) == selected_project.as_ref())
            .map_or_else(
                || s.scope_global.to_string(),
                |project| project.name.clone(),
            );
        let agents: Vec<_> = ws
            .agents()
            .iter()
            .filter(|agent| agent.installed && agent.enabled)
            .map(|agent| (agent.key.clone(), agent.display_name.clone()))
            .collect();
        let selected_agent = self.agent_filter.clone();
        let agent_label = agents
            .iter()
            .find(|(key, _)| Some(key) == selected_agent.as_ref())
            .map_or_else(
                || s.library_all_agents.to_string(),
                |(_, name)| name.clone(),
            );
        let scope_view = cx.weak_entity();
        let agent_view = scope_view.clone();
        let border = cx.theme().border;
        h_flex()
            .min_h(px(48.))
            .px_5()
            .py_2()
            .gap_2()
            .flex_wrap()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex_1()
                    .min_w(px(160.))
                    .max_w(px(280.))
                    .child(Input::new(&self.library_search).cleanable(true)),
            )
            .child(
                Button::new("library-scope")
                    .outline()
                    .max_w(px(224.))
                    .label(scope)
                    .icon(IconName::ChevronDown)
                    .tooltip(s.library_scope)
                    .dropdown_menu(move |mut menu, _, _| {
                        let view = scope_view.clone();
                        menu = menu.item(
                            PopupMenuItem::new(s.scope_global)
                                .checked(selected_project.is_none())
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |view, cx| view.set_scope(None, cx));
                                }),
                        );
                        for project in &projects {
                            let id = project.id.clone();
                            let view = scope_view.clone();
                            menu = menu.item(
                                PopupMenuItem::new(project.name.clone())
                                    .checked(Some(&id) == selected_project.as_ref())
                                    .on_click(move |_, _, cx| {
                                        let _ = view.update(cx, |view, cx| {
                                            view.set_scope(Some(id.clone()), cx)
                                        });
                                    }),
                            );
                        }
                        let view = scope_view.clone();
                        menu.separator()
                            .item(PopupMenuItem::new(s.btn_add_project).on_click(
                                move |_, window, cx| {
                                    let _ = view.update(cx, |view, cx| {
                                        view.pick_and_attach_project(window, cx)
                                    });
                                },
                            ))
                    }),
            )
            .child(
                Button::new("library-agent-filter")
                    .outline()
                    .max_w(px(224.))
                    .label(agent_label)
                    .icon(IconName::ChevronDown)
                    .tooltip(s.library_agent_filter)
                    .dropdown_menu(move |mut menu, _, _| {
                        let view = agent_view.clone();
                        menu = menu.item(
                            PopupMenuItem::new(s.library_all_agents)
                                .checked(selected_agent.is_none())
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |view, cx| {
                                        view.agent_filter = None;
                                        view.refilter(cx);
                                        cx.notify();
                                    });
                                }),
                        );
                        for (key, label) in &agents {
                            let key = key.clone();
                            let view = agent_view.clone();
                            menu = menu.item(
                                PopupMenuItem::new(label.clone())
                                    .checked(Some(&key) == selected_agent.as_ref())
                                    .on_click(move |_, _, cx| {
                                        let _ = view.update(cx, |view, cx| {
                                            view.agent_filter = Some(key.clone());
                                            view.refilter(cx);
                                            cx.notify();
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            )
            .child(div().flex_1())
            .child(
                Button::new("refresh-library")
                    .ghost()
                    .icon(IconName::RefreshCw)
                    .tooltip(s.tooltip_refresh)
                    .accessibility_label(s.tooltip_refresh)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.workspace.update(cx, |ws, cx| {
                            ws.reload_all(cx);
                            ws.notify_toast(false, ws.t().toast_refreshed, window, cx);
                        });
                    })),
            )
    }

    pub(super) fn render_skill_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let skills_empty = self.workspace.read(cx).skills().is_empty();
        let item_count = self.filtered_indices.len();
        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .when(item_count > 0, |pane| {
                pane.child(
                    h_flex()
                        .h_8()
                        .px_4()
                        .gap_2()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .border_b_1()
                        .border_color(theme.border)
                        .child(div().flex_1().min_w_0().child(s.library_skill_column))
                        .child(
                            div()
                                .w(px(76.))
                                .flex_shrink_0()
                                .text_right()
                                .truncate()
                                .child(s.library_deployment),
                        )
                        .child(
                            div()
                                .w(px(96.))
                                .flex_shrink_0()
                                .text_right()
                                .truncate()
                                .child(s.library_token_column),
                        ),
                )
            })
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
                                            .flex_wrap()
                                            .child(
                                                Button::new("empty-library-install")
                                                    .primary()
                                                    .label(s.btn_install)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.workspace.update(cx, |ws, cx| {
                                                            ws.open_install(cx)
                                                        })
                                                    })),
                                            )
                                            .child(
                                                Button::new("empty-go-discover")
                                                    .outline()
                                                    .label(s.btn_discover_adopt)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.workspace.update(cx, |ws, cx| {
                                                            ws.navigate(NavTab::Discover, cx)
                                                        })
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
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "library-skill-vlist",
                            item_count,
                            cx.processor(|this, range: Range<usize>, _, cx| {
                                let ws = this.workspace.read(cx);
                                let agents: Vec<_> = ws
                                    .agents()
                                    .iter()
                                    .filter(|agent| agent.installed && agent.enabled)
                                    .collect();
                                let muted = cx.theme().muted_foreground;
                                let border = cx.theme().border;
                                range
                                    .filter_map(|ix| {
                                        let skill =
                                            ws.skills().get(*this.filtered_indices.get(ix)?)?;
                                        let selected = this.selected_skill_id.as_deref()
                                            == Some(skill.id.as_str());
                                        let sid = skill.id.clone();
                                        let deployed = agents
                                            .iter()
                                            .filter(|agent| this.deployed(skill, &agent.key))
                                            .count();
                                        let source = skill
                                            .source_ref
                                            .as_deref()
                                            .unwrap_or_else(|| skill.source_type.as_str());
                                        Some(
                                            div().w_full().px_2().child(
                                                InteractiveCard::new(
                                                    format!("skill-row-{}", skill.id),
                                                    h_flex(),
                                                    selected,
                                                    cx,
                                                )
                                                .surface(
                                                    cx.theme().background,
                                                    cx.theme().list_hover,
                                                    border.opacity(0.),
                                                )
                                                .accessible(skill.name.clone())
                                                .h(px(48.))
                                                .w_full()
                                                .px_2()
                                                .gap_2()
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.select_skill(sid.clone(), window, cx)
                                                    },
                                                ))
                                                .child(
                                                    v_flex()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_semibold()
                                                                .truncate()
                                                                .child(skill.name.clone()),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(muted)
                                                                .truncate()
                                                                .child(source.to_string()),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(76.))
                                                        .flex_shrink_0()
                                                        .text_right()
                                                        .text_sm()
                                                        .child(format!(
                                                            "{deployed} / {}",
                                                            agents.len()
                                                        )),
                                                )
                                                .child(
                                                    h_flex()
                                                        .w(px(96.))
                                                        .flex_shrink_0()
                                                        .justify_end()
                                                        .child(TokenBadge::tokens(
                                                            skill.estimated_tokens,
                                                        )),
                                                ),
                                            ),
                                        )
                                    })
                                    .collect()
                            }),
                        )
                        .size_full()
                        .track_scroll(&self.library_scroll_handle),
                    )
                    .vertical_scrollbar(&self.library_scroll_handle)
                    .into_any_element()
            })
    }
}
