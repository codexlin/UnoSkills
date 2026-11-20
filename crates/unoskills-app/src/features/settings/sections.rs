use gpui_kit::component::{
    ActiveTheme, IconName, Sizable, StyledExt, ThemeMode,
    button::{Button, ButtonVariants},
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    input::Input,
    select::Select,
    separator::Separator,
    switch::Switch,
    tab::{Tab, TabBar},
    v_flex,
};
use gpui_kit::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::FluentBuilder, px,
};
use unoskills_core::{AgentInfo, DeployMode};

use crate::components::badges::StatusBadge;
use crate::fonts::FONT_SIZE_CHOICES;

use super::SettingsView;

impl SettingsView {
    pub(super) fn render_general_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let is_dark = theme.is_dark();
        let deploy_mode = ws.deploy_mode();
        let root_dir = ws.manager().root_dir().to_path_buf();
        let root_path = root_dir.to_string_lossy().to_string();

        GroupBox::new().outline().title(s.settings_general).child(
            v_flex()
                .gap_4()
                .child(setting_row(
                    s.settings_language,
                    s.settings_language_desc,
                    theme.muted_foreground,
                    div()
                        .w(px(180.))
                        .flex_shrink_0()
                        .child(Select::new(&self.lang_select).small()),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_deploy_mode,
                    s.settings_deploy_mode_desc,
                    theme.muted_foreground,
                    div().flex_shrink_0().child(
                        TabBar::new("settings-deploy-mode")
                            .segmented()
                            .small()
                            .selected_index(if deploy_mode == DeployMode::Symlink {
                                0
                            } else {
                                1
                            })
                            .child(Tab::new().label(s.mode_symlink))
                            .child(Tab::new().label(s.mode_copy))
                            .on_click(cx.listener(|this, idx: &usize, window, cx| {
                                let mode = if *idx == 0 {
                                    DeployMode::Symlink
                                } else {
                                    DeployMode::Copy
                                };
                                this.workspace.update(cx, |ws, cx| {
                                    ws.set_deploy_mode(mode, window, cx);
                                });
                            })),
                    ),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_appearance,
                    s.settings_appearance_desc,
                    theme.muted_foreground,
                    div().flex_shrink_0().child(
                        TabBar::new("settings-theme-mode")
                            .segmented()
                            .small()
                            .selected_index(if is_dark { 0 } else { 1 })
                            .child(Tab::new().label(s.theme_dark))
                            .child(Tab::new().label(s.theme_light))
                            .on_click(cx.listener(|this, idx: &usize, window, cx| {
                                let mode = if *idx == 0 {
                                    ThemeMode::Dark
                                } else {
                                    ThemeMode::Light
                                };
                                this.workspace.update(cx, |ws, cx| {
                                    ws.set_theme_mode(mode, window, cx);
                                });
                            })),
                    ),
                ))
                .child(Separator::horizontal())
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .child(div().text_sm().font_medium().child(s.settings_root_dir))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .truncate()
                                        .child(root_path),
                                ),
                        )
                        .child(
                            h_flex()
                                .flex_shrink_0()
                                .items_center()
                                .gap_2()
                                .child(StatusBadge::secondary("SQLite WAL"))
                                .child(
                                    Button::new("reveal-root-dir")
                                        .outline()
                                        .xsmall()
                                        .icon(IconName::FolderOpen)
                                        .tooltip(s.tooltip_reveal_folder)
                                        .on_click(move |_, _, cx| {
                                            cx.reveal_path(&root_dir);
                                        }),
                                ),
                        ),
                ),
        )
    }

    pub(super) fn render_fonts_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let current_font_size = ws.font_settings().font_size;
        let size_idx = FONT_SIZE_CHOICES
            .iter()
            .position(|&v| (v - current_font_size).abs() < 0.5)
            .unwrap_or(1);

        GroupBox::new().outline().title(s.settings_fonts).child(
            v_flex()
                .gap_4()
                .child(setting_row(
                    s.settings_ui_font,
                    s.settings_ui_font_desc,
                    theme.muted_foreground,
                    div()
                        .w(px(240.))
                        .flex_shrink_0()
                        .child(Select::new(&self.ui_font_select).small()),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_mono_font,
                    s.settings_mono_font_desc,
                    theme.muted_foreground,
                    div()
                        .w(px(240.))
                        .flex_shrink_0()
                        .child(Select::new(&self.mono_font_select).small()),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_font_size,
                    s.settings_font_size_desc,
                    theme.muted_foreground,
                    div().flex_shrink_0().child(
                        TabBar::new("settings-font-size")
                            .segmented()
                            .small()
                            .selected_index(size_idx)
                            .children(
                                FONT_SIZE_CHOICES
                                    .iter()
                                    .map(|sz| Tab::new().label(format!("{}px", *sz as u32))),
                            )
                            .on_click(cx.listener(|this, idx: &usize, _, cx| {
                                if let Some(&sz) = FONT_SIZE_CHOICES.get(*idx) {
                                    this.workspace.update(cx, |ws, cx| ws.set_font_size(sz, cx));
                                }
                            })),
                    ),
                )),
        )
    }

    pub(super) fn render_network_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();

        GroupBox::new().outline().title(s.settings_network).child(
            v_flex()
                .gap_4()
                .child(setting_row(
                    s.settings_http_proxy,
                    s.settings_http_proxy_desc,
                    theme.muted_foreground,
                    div()
                        .w(px(280.))
                        .flex_shrink_0()
                        .child(Input::new(&self.http_proxy_input).small().cleanable(true)),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_github_token,
                    s.settings_github_token_desc,
                    theme.muted_foreground,
                    div().w(px(280.)).flex_shrink_0().child(
                        Input::new(&self.github_token_input)
                            .small()
                            .cleanable(true)
                            .mask_toggle(),
                    ),
                ))
                .child(
                    h_flex().justify_end().child(
                        Button::new("save-network-settings-btn")
                            .primary()
                            .small()
                            .icon(IconName::Check)
                            .label(s.btn_save)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let proxy = this.http_proxy_input.read(cx).value().to_string();
                                let token = this.github_token_input.read(cx).value().to_string();
                                this.workspace.update(cx, |ws, cx| {
                                    ws.save_network_settings(&proxy, &token, window, cx);
                                });
                            })),
                    ),
                ),
        )
    }

    pub(super) fn render_git_backup_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ws = self.workspace.read(cx);
        let lang = ws.lang();
        let s = lang.t();
        let backup_busy = ws.backup_busy();
        let summary = lang.backup_scope_summary(ws.skills().len(), ws.local_stacks().len());

        GroupBox::new()
            .outline()
            .title(s.settings_git_backup)
            .child(
                v_flex()
                    .gap_4()
                    .child(setting_row(
                        s.settings_git_backup_repo,
                        s.settings_git_backup_repo_desc,
                        theme.muted_foreground,
                        div()
                            .w(px(320.))
                            .flex_shrink_0()
                            .child(Input::new(&self.backup_repo_input).small().cleanable(true)),
                    ))
                    .child(Separator::horizontal())
                    .child(setting_row(
                        s.settings_git_backup_branch,
                        s.settings_git_backup_branch_desc,
                        theme.muted_foreground,
                        div()
                            .w(px(160.))
                            .flex_shrink_0()
                            .child(Input::new(&self.backup_branch_input).small()),
                    ))
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
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
                                    .child(summary),
                            )
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        Button::new("save-git-backup-btn")
                                            .outline()
                                            .small()
                                            .icon(IconName::Check)
                                            .label(s.btn_save)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                let repo = this
                                                    .backup_repo_input
                                                    .read(cx)
                                                    .value()
                                                    .to_string();
                                                let branch = this
                                                    .backup_branch_input
                                                    .read(cx)
                                                    .value()
                                                    .to_string();
                                                this.workspace.update(cx, |ws, cx| {
                                                    ws.save_git_backup_config(
                                                        &repo, &branch, window, cx,
                                                    );
                                                });
                                            })),
                                    )
                                    .child(
                                        Button::new("git-backup-pull-btn")
                                            .outline()
                                            .small()
                                            .icon(IconName::ArrowDown)
                                            .loading(backup_busy)
                                            .label(s.btn_backup_pull)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                let repo = this
                                                    .backup_repo_input
                                                    .read(cx)
                                                    .value()
                                                    .to_string();
                                                let branch = this
                                                    .backup_branch_input
                                                    .read(cx)
                                                    .value()
                                                    .to_string();
                                                this.workspace.update(cx, |ws, cx| {
                                                    let _ = ws
                                                        .manager()
                                                        .configure_git_backup(&repo, Some(&branch));
                                                    ws.git_backup_pull_async(window, cx);
                                                });
                                            })),
                                    )
                                    .child(
                                        Button::new("git-backup-push-btn")
                                            .primary()
                                            .small()
                                            .icon(IconName::ArrowUp)
                                            .loading(backup_busy)
                                            .label(s.btn_backup_push)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                let repo = this
                                                    .backup_repo_input
                                                    .read(cx)
                                                    .value()
                                                    .to_string();
                                                let branch = this
                                                    .backup_branch_input
                                                    .read(cx)
                                                    .value()
                                                    .to_string();
                                                this.workspace.update(cx, |ws, cx| {
                                                    let _ = ws
                                                        .manager()
                                                        .configure_git_backup(&repo, Some(&branch));
                                                    ws.git_backup_push_async(window, cx);
                                                });
                                            })),
                                    ),
                            ),
                    ),
            )
    }

    pub(super) fn render_updates_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let checking_updates = ws.checking_updates();

        GroupBox::new().outline().title(s.settings_updates).child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_4()
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
                                        .child(s.settings_current_version),
                                )
                                .child(StatusBadge::secondary(concat!(
                                    "v",
                                    env!("CARGO_PKG_VERSION")
                                ))),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(s.settings_updates_desc),
                        ),
                )
                .child(
                    div().flex_shrink_0().child(
                        Button::new("check-updates-btn")
                            .outline()
                            .small()
                            .icon(IconName::RefreshCw)
                            .loading(checking_updates)
                            .label(s.btn_check_updates)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.workspace
                                    .update(cx, |ws, cx| ws.check_for_updates_async(window, cx));
                            })),
                    ),
                ),
        )
    }

    pub(super) fn render_agents_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let agents: Vec<AgentInfo> = ws.agents().to_vec();

        GroupBox::new()
            .outline()
            .title(format!("{} ({})", s.settings_agents_registry, agents.len()))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(s.settings_agents_registry_desc),
                    )
                    // Inline Add Custom Agent bar
                    .child(
                        v_flex()
                            .p_3()
                            .gap_2()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.secondary.opacity(0.25))
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .child(s.settings_add_custom_agent),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div().w(px(180.)).flex_shrink_0().child(
                                            Input::new(&self.custom_agent_name_input).small(),
                                        ),
                                    )
                                    .child(
                                        div().flex_1().min_w_0().child(
                                            Input::new(&self.custom_agent_dir_input).small(),
                                        ),
                                    )
                                    .child(
                                        div().flex_shrink_0().child(
                                            Button::new("pick-custom-agent-dir")
                                                .outline()
                                                .small()
                                                .icon(IconName::FolderOpen)
                                                .tooltip(s.btn_browse_folder)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    let prompt = this
                                                        .workspace
                                                        .read(cx)
                                                        .t()
                                                        .prompt_pick_agent_dir;
                                                    let rx = cx.prompt_for_paths(
                                                        gpui_kit::PathPromptOptions {
                                                            files: false,
                                                            directories: true,
                                                            multiple: false,
                                                            prompt: Some(SharedString::from(
                                                                prompt,
                                                            )),
                                                        },
                                                    );
                                                    cx.spawn_in(window, async move |this, cx| {
                                                        if let Ok(Ok(Some(paths))) = rx.await
                                                            && let Some(dir) =
                                                                paths.into_iter().next()
                                                        {
                                                            let p =
                                                                dir.to_string_lossy().to_string();
                                                            let _ = this.update_in(
                                                                cx,
                                                                |view, window, cx| {
                                                                    view.custom_agent_dir_input
                                                                        .update(cx, |input, cx| {
                                                                            input.set_value(
                                                                                p, window, cx,
                                                                            );
                                                                        });
                                                                },
                                                            );
                                                        }
                                                    })
                                                    .detach();
                                                })),
                                        ),
                                    )
                                    .child(
                                        div().flex_shrink_0().child(
                                            Button::new("add-custom-agent-btn")
                                                .primary()
                                                .small()
                                                .icon(IconName::Plus)
                                                .label(s.btn_add_agent)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    let name = this
                                                        .custom_agent_name_input
                                                        .read(cx)
                                                        .value()
                                                        .to_string();
                                                    let dir = this
                                                        .custom_agent_dir_input
                                                        .read(cx)
                                                        .value()
                                                        .to_string();
                                                    let added =
                                                        this.workspace.update(cx, |ws, cx| {
                                                            ws.add_custom_agent(
                                                                &name, &dir, window, cx,
                                                            )
                                                        });
                                                    if added {
                                                        this.custom_agent_name_input.update(
                                                            cx,
                                                            |input, cx| {
                                                                input.set_value("", window, cx);
                                                            },
                                                        );
                                                        this.custom_agent_dir_input.update(
                                                            cx,
                                                            |input, cx| {
                                                                input.set_value("", window, cx);
                                                            },
                                                        );
                                                    }
                                                })),
                                        ),
                                    ),
                            ),
                    )
                    .children(agents.iter().map(|agent| self.render_agent_row(agent, cx))),
            )
    }

    fn render_agent_row(&self, agent: &AgentInfo, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let agent_key = agent.key.clone();
        let agent_key_delete = agent.key.clone();
        let enabled = agent.enabled;
        let is_custom = agent.is_custom;

        h_flex()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
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
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_medium()
                                    .truncate()
                                    .child(agent.display_name.clone()),
                            )
                            .child(if agent.installed {
                                StatusBadge::success(s.badge_detected).outline()
                            } else {
                                StatusBadge::secondary(s.badge_not_detected)
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(agent.skills_dir.to_string_lossy().to_string()),
                    ),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .when(is_custom, |row| {
                        row.child(
                            Button::new(SharedString::from(format!(
                                "del-custom-agent-{}",
                                agent.key
                            )))
                            .danger()
                            .ghost()
                            .xsmall()
                            .icon(IconName::Delete)
                            .tooltip(s.btn_delete)
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.workspace.update(cx, |ws, cx| {
                                        ws.remove_custom_agent(&agent_key_delete, window, cx)
                                    });
                                },
                            )),
                        )
                    })
                    .child(
                        Switch::new(SharedString::from(format!("agent-en-{}", agent.key)))
                            .small()
                            .checked(enabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.workspace.update(cx, |ws, cx| {
                                    ws.set_agent_enabled(&agent_key, !enabled, cx)
                                });
                            })),
                    ),
            )
            .into_any_element()
    }
}

fn setting_row(
    title: &'static str,
    description: &'static str,
    muted_fg: gpui_kit::Hsla,
    control: impl IntoElement,
) -> impl IntoElement {
    h_flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(div().text_sm().font_medium().child(title))
                .child(div().text_xs().text_color(muted_fg).child(description)),
        )
        .child(control)
}
