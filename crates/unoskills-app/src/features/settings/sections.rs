use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::{Transition, actions::Confirm, transition};
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    input::{Input, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    select::Select,
    separator::Separator,
    switch::Switch,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Focusable, InteractiveElement, IntoElement,
    ParentElement, PathPromptOptions, SharedString, StatefulInteractiveElement, StyleRefinement,
    Styled, WeakEntity, Window, div, prelude::FluentBuilder, px, rems,
};
use unoskills_core::{AgentInfo, DeployMode};

use crate::components::badges::StatusBadge;
use crate::components::segmented::SegmentedControl;
use crate::fonts::{AccentPalette, AppearancePreference, FONT_SIZE_CHOICES, RADIUS_CHOICES};
use crate::state::WorkspaceState;

use super::SettingsView;

const SETTINGS_FIELD_WIDTH: f32 = 200.;
const AGENT_ROW_HEIGHT_REM: f32 = 2.5;
const AGENT_GROUP_PADDING_REM: f32 = 1.0;
const AGENT_DISCLOSURE_GAP_REM: f32 = 0.75;

impl SettingsView {
    pub(super) fn render_general_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let deploy_mode = ws.deploy_mode();
        let root_dir = ws.manager().root_dir().to_path_buf();
        let root_path = root_dir.to_string_lossy().to_string();

        card_group_box(s.settings_general, theme.group_box, theme.radius_lg).child(
            v_flex()
                .gap_4()
                .child(setting_row(
                    s.settings_language,
                    s.settings_language_desc,
                    theme.muted_foreground,
                    div().w(px(SETTINGS_FIELD_WIDTH)).flex_shrink_0().child(
                        Select::new(&self.lang_select).accessibility_label(s.settings_language),
                    ),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_deploy_mode,
                    s.settings_deploy_mode_desc,
                    theme.muted_foreground,
                    div().flex_shrink_0().child(
                        SegmentedControl::new(
                            "settings-deploy-mode",
                            [s.mode_symlink, s.mode_copy],
                            if deploy_mode == DeployMode::Symlink {
                                0
                            } else {
                                1
                            },
                        )
                        .on_click(cx.listener(
                            |this, idx: &usize, window, cx| {
                                let mode = if *idx == 0 {
                                    DeployMode::Symlink
                                } else {
                                    DeployMode::Copy
                                };
                                this.workspace.update(cx, |ws, cx| {
                                    ws.set_deploy_mode(mode, window, cx);
                                });
                            },
                        )),
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
                                        .font_family(theme.mono_font_family.clone())
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .truncate()
                                        .child(root_path),
                                ),
                        )
                        .child(
                            div().flex_shrink_0().child(
                                Button::new("reveal-root-dir")
                                    .outline()
                                    .icon(IconName::FolderOpen)
                                    .tooltip(s.tooltip_reveal_folder)
                                    .accessibility_label(s.tooltip_reveal_folder)
                                    .on_click(move |_, _, cx| {
                                        cx.reveal_path(&root_dir);
                                    }),
                            ),
                        ),
                ),
        )
    }

    pub(super) fn render_appearance_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let appearances = [
            (AppearancePreference::System, s.theme_system),
            (AppearancePreference::Light, s.theme_light),
            (AppearancePreference::Dark, s.theme_dark),
        ];
        let selected_appearance = appearances
            .iter()
            .position(|(mode, _)| *mode == ws.appearance())
            .unwrap_or_default();
        let active_palette = ws.font_settings().palette;
        let palette_items = [
            s.palette_claude,
            s.palette_vercel,
            s.palette_supabase,
            s.palette_catppuccin,
            s.palette_ocean,
            "UnoSkills",
        ];
        let palette_view = cx.weak_entity();
        let current_radius = ws.font_settings().corner_radius;
        let radius_idx = RADIUS_CHOICES
            .iter()
            .position(|&r| (r - current_radius).abs() < 0.5)
            .unwrap_or(3);
        card_group_box(s.settings_appearance, theme.group_box, theme.radius_lg).child(
            v_flex()
                .gap_4()
                .child(setting_row(
                    s.settings_appearance,
                    s.settings_appearance_desc,
                    theme.muted_foreground,
                    div().flex_shrink_0().child(
                        SegmentedControl::new(
                            "settings-theme-mode",
                            appearances.map(|(_, label)| label),
                            selected_appearance,
                        )
                        .icons([AssetIcon::Monitor, AssetIcon::Sun, AssetIcon::Moon])
                        .on_click(cx.listener(
                            move |this, idx: &usize, window, cx| {
                                this.workspace.update(cx, |ws, cx| {
                                    ws.set_appearance(appearances[*idx].0, window, cx);
                                });
                            },
                        )),
                    ),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_accent_palette,
                    s.settings_accent_palette_desc,
                    theme.muted_foreground,
                    Button::new("settings-accent-palette")
                        .outline()
                        .icon(IconName::ChevronDown)
                        .label(palette_items[active_palette.index()])
                        .dropdown_menu(move |mut menu, _, _| {
                            for (palette, label) in
                                AccentPalette::ALL.into_iter().zip(palette_items)
                            {
                                let view = palette_view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(label)
                                        .checked(palette == active_palette)
                                        .on_click(move |_, _, cx| {
                                            let _ = view.update(cx, |view, cx| {
                                                view.workspace.update(cx, |ws, cx| {
                                                    ws.set_accent_palette(palette, cx)
                                                });
                                            });
                                        }),
                                );
                            }
                            menu
                        }),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_corner_radius,
                    s.settings_corner_radius_desc,
                    theme.muted_foreground,
                    div().flex_shrink_0().child(
                        SegmentedControl::new(
                            "settings-corner-radius",
                            RADIUS_CHOICES.iter().map(|r| format!("{}px", *r as u32)),
                            radius_idx,
                        )
                        .on_click(cx.listener(
                            |this, idx: &usize, _, cx| {
                                if let Some(&r) = RADIUS_CHOICES.get(*idx) {
                                    this.workspace
                                        .update(cx, |ws, cx| ws.set_corner_radius(r, cx));
                                }
                            },
                        )),
                    ),
                )),
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

        card_group_box(s.settings_fonts, theme.group_box, theme.radius_lg).child(
            v_flex()
                .gap_4()
                .child(setting_row(
                    s.settings_ui_font,
                    s.settings_ui_font_desc,
                    theme.muted_foreground,
                    div().w(px(SETTINGS_FIELD_WIDTH)).flex_shrink_0().child(
                        Select::new(&self.ui_font_select).accessibility_label(s.settings_ui_font),
                    ),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_mono_font,
                    s.settings_mono_font_desc,
                    theme.muted_foreground,
                    div().w(px(SETTINGS_FIELD_WIDTH)).flex_shrink_0().child(
                        Select::new(&self.mono_font_select)
                            .accessibility_label(s.settings_mono_font),
                    ),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_font_size,
                    s.settings_font_size_desc,
                    theme.muted_foreground,
                    div().flex_shrink_0().child(
                        SegmentedControl::new(
                            "settings-font-size",
                            FONT_SIZE_CHOICES
                                .iter()
                                .map(|sz| format!("{}px", *sz as u32)),
                            size_idx,
                        )
                        .on_click(cx.listener(
                            |this, idx: &usize, _, cx| {
                                if let Some(&sz) = FONT_SIZE_CHOICES.get(*idx) {
                                    this.workspace.update(cx, |ws, cx| ws.set_font_size(sz, cx));
                                }
                            },
                        )),
                    ),
                )),
        )
    }

    pub(super) fn render_network_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();

        card_group_box(s.settings_network, theme.group_box, theme.radius_lg).child(
            v_flex()
                .gap_4()
                .child(setting_stacked_field(
                    s.settings_http_proxy,
                    s.settings_http_proxy_desc,
                    theme.muted_foreground,
                    Input::new(&self.http_proxy_input).cleanable(true),
                ))
                .child(Separator::horizontal())
                .child(setting_stacked_field(
                    s.settings_github_token,
                    s.settings_github_token_desc,
                    theme.muted_foreground,
                    Input::new(&self.github_token_input)
                        .cleanable(true)
                        .mask_toggle(),
                ))
                .child(
                    h_flex().justify_end().child(
                        Button::new("save-network-settings-btn")
                            .primary()
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

        card_group_box(s.settings_git_backup, theme.group_box, theme.radius_lg).child(
            v_flex()
                .gap_4()
                .child(setting_stacked_field(
                    s.settings_git_backup_repo,
                    s.settings_git_backup_repo_desc,
                    theme.muted_foreground,
                    Input::new(&self.backup_repo_input).cleanable(true),
                ))
                .child(Separator::horizontal())
                .child(setting_row(
                    s.settings_git_backup_branch,
                    s.settings_git_backup_branch_desc,
                    theme.muted_foreground,
                    div()
                        .w(px(SETTINGS_FIELD_WIDTH))
                        .flex_shrink_0()
                        .child(Input::new(&self.backup_branch_input)),
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
                                        .icon(IconName::Check)
                                        .label(s.btn_save)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            let repo =
                                                this.backup_repo_input.read(cx).value().to_string();
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
                                        .icon(IconName::ArrowDown)
                                        .loading(backup_busy)
                                        .label(s.btn_backup_pull)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            let repo =
                                                this.backup_repo_input.read(cx).value().to_string();
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
                                        .icon(IconName::ArrowUp)
                                        .loading(backup_busy)
                                        .label(s.btn_backup_push)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            let repo =
                                                this.backup_repo_input.read(cx).value().to_string();
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
        let downloading_update = ws.downloading_update();
        let available_update = ws.available_update().cloned();
        let downloaded_update = ws.downloaded_update().cloned();

        card_group_box(s.settings_updates, theme.group_box, theme.radius_lg).child(
            v_flex()
                .gap_3()
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
                                    .icon(IconName::RefreshCw)
                                    .loading(checking_updates)
                                    .label(s.btn_check_updates)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.workspace.update(cx, |ws, cx| {
                                            ws.check_for_updates_async(false, window, cx)
                                        });
                                    })),
                            ),
                        ),
                )
                .when_some(
                    available_update.and_then(|u| {
                        u.latest
                            .map(|r| (r, u.recommended_asset, u.checksums_asset.is_some()))
                    }),
                    |pane, (release, asset, _has_sums)| {
                        let release_url = release.html_url.clone();
                        let asset_label = asset
                            .as_ref()
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| release.tag_name.clone());
                        let install_btn_label = crate::state::platform_install_button_label(s);
                        let verified_download = downloaded_update
                            .as_ref()
                            .filter(|d| d.version == release.tag_name)
                            .cloned();

                        pane.child(Separator::horizontal()).child(
                            h_flex()
                                .w_full()
                                .flex_wrap()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .p_3()
                                .rounded(theme.radius)
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.secondary.opacity(0.4))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_1()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(StatusBadge::success(format!(
                                                    "{} {}",
                                                    s.badge_update_available, release.tag_name
                                                )))
                                                .when(verified_download.is_some(), |row| {
                                                    row.child(
                                                        StatusBadge::info(s.badge_sha256_verified)
                                                            .outline(),
                                                    )
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .truncate()
                                                .child(asset_label),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .flex_wrap()
                                        .gap_2()
                                        .when(!release_url.is_empty(), |row| {
                                            row.child(
                                                Button::new("open-release-page-btn")
                                                    .outline()
                                                    .icon(IconName::ExternalLink)
                                                    .label(s.btn_open_release)
                                                    .on_click(move |_, _, cx| {
                                                        cx.open_url(&release_url);
                                                    }),
                                            )
                                        })
                                        .child(if verified_download.is_some() {
                                            Button::new("apply-downloaded-update-btn")
                                                .primary()
                                                .icon(IconName::Check)
                                                .label(install_btn_label)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.workspace.update(cx, |ws, cx| {
                                                        ws.apply_downloaded_update(window, cx);
                                                    });
                                                }))
                                        } else {
                                            Button::new("download-verify-update-btn")
                                                .primary()
                                                .icon(IconName::ArrowDown)
                                                .loading(downloading_update)
                                                .label(s.btn_download_update)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.workspace.update(cx, |ws, cx| {
                                                        ws.prompt_download_update(window, cx);
                                                    });
                                                }))
                                        }),
                                ),
                        )
                    },
                ),
        )
    }

    pub(super) fn render_agents_section(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted_fg = cx.theme().muted_foreground;
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let (installed_agents, uninstalled_agents): (Vec<_>, Vec<_>) = ws
            .agents()
            .iter()
            .cloned()
            .partition(|agent| agent.installed);

        v_flex()
            .w_full()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .font_semibold()
                            .text_color(muted_fg)
                            .child(format!("{} ({})", s.badge_detected, installed_agents.len())),
                    )
                    .child(
                        Button::new("open-add-custom-agent")
                            .outline()
                            .flex_shrink_0()
                            .icon(IconName::Plus)
                            .label(s.settings_add_custom_agent)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_add_agent_dialog(window, cx);
                            })),
                    ),
            )
            .when(!installed_agents.is_empty(), |col| {
                col.child(self.render_agent_group(&installed_agents, true, cx))
            })
            .when(!uninstalled_agents.is_empty(), |col| {
                col.child(self.render_undetected_agents(&uninstalled_agents, window, cx))
            })
    }

    fn open_add_agent_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.workspace.downgrade();
        let s = self.workspace.read(cx).t();
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_custom_agent_name));
        let directory = cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_custom_agent_dir));
        let focus = name.read(cx).focus_handle(cx);
        window.open_dialog(cx, move |dialog, _, _| {
            let submit_name = name.clone();
            let submit_directory = directory.clone();
            let submit_workspace = workspace.clone();
            let browse_directory = directory.downgrade();
            let browse_workspace = workspace.clone();

            dialog
                .title(s.settings_add_custom_agent)
                .w(px(444.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            v_flex()
                                .gap_1()
                                .child(div().text_sm().child(s.custom_agent_name))
                                .child(Input::new(&name).aria_label(s.custom_agent_name)),
                        )
                        .child(
                            v_flex()
                                .gap_1()
                                .child(div().text_sm().child(s.custom_agent_directory))
                                .child(
                                    h_flex()
                                        .w_full()
                                        .gap_2()
                                        .child(
                                            div().flex_1().min_w_0().child(
                                                Input::new(&directory)
                                                    .aria_label(s.custom_agent_directory),
                                            ),
                                        )
                                        .child(
                                            Button::new("pick-custom-agent-dir")
                                                .outline()
                                                .flex_shrink_0()
                                                .icon(IconName::FolderOpen)
                                                .tooltip(s.btn_browse_folder)
                                                .accessibility_label(s.btn_browse_folder)
                                                .on_click(move |_, window, cx| {
                                                    pick_custom_agent_directory(
                                                        browse_directory.clone(),
                                                        browse_workspace.clone(),
                                                        s.prompt_pick_agent_dir,
                                                        window,
                                                        cx,
                                                    );
                                                }),
                                        ),
                                ),
                        ),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("cancel-custom-agent")
                                .outline()
                                .label(s.btn_cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("submit-custom-agent")
                                .primary()
                                .label(s.btn_add_agent)
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(
                                        Box::new(Confirm { secondary: false }),
                                        cx,
                                    );
                                }),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    let name = submit_name.read(cx).value();
                    let directory = submit_directory.read(cx).value();
                    submit_workspace
                        .update(cx, |ws, cx| {
                            ws.add_custom_agent(&name, &directory, window, cx)
                        })
                        .unwrap_or(false)
                })
        });
        window.focus(&focus, cx);
    }

    fn render_undetected_agents(
        &self,
        agents: &[AgentInfo],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let s = self.workspace.read(cx).t();
        let theme = cx.theme();
        let focus_border = theme.primary;
        let hover = theme.list_hover;
        let expanded = self.show_undetected_agents;
        let label = format!("{} ({})", s.badge_not_detected, agents.len());
        let motion = theme.motion_tokens();
        let reveal = transition(
            "undetected-agents-reveal",
            if expanded { 1.0 } else { 0.0 },
            Transition::new(motion.duration_normal).easing(motion.easing_move.clone()),
            window,
            cx,
        );

        // The outlined group has two 1px borders. Fixed-height rows let the
        // reveal lay out at its animated size even on the first frame.
        let full_height = window.rem_size()
            * (agents.len() as f32 * AGENT_ROW_HEIGHT_REM
                + 2.0 * AGENT_GROUP_PADDING_REM
                + AGENT_DISCLOSURE_GAP_REM)
            + px(2.0);

        Collapsible::new()
            .w_full()
            .open(reveal > 0.0)
            .child(
                h_flex()
                    .id("toggle-undetected-agents")
                    .w_full()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .rounded_sm()
                    .border_1()
                    .border_color(focus_border.opacity(0.))
                    .cursor_pointer()
                    .focusable()
                    .tab_stop(true)
                    .focus_visible(|style| style.border_color(focus_border))
                    .hover(|style| style.bg(hover))
                    .role(gpui_kit::Role::Button)
                    .aria_label(label.clone())
                    .aria_expanded(expanded)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_undetected_agents = !this.show_undetected_agents;
                        cx.notify();
                    }))
                    .child(
                        Icon::new(if expanded {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .small(),
                    )
                    .child(div().text_sm().font_semibold().child(label)),
            )
            .when(reveal > 0.0, |panel| {
                panel.content(
                    v_flex()
                        .h(full_height * reveal)
                        .w_full()
                        .overflow_hidden()
                        .child(
                            v_flex()
                                .flex_shrink_0()
                                .pt(rems(AGENT_DISCLOSURE_GAP_REM))
                                .child(self.render_agent_group(agents, expanded, cx)),
                        ),
                )
            })
    }

    fn render_agent_group(
        &self,
        agents: &[AgentInfo],
        interactive: bool,
        cx: &mut Context<Self>,
    ) -> GroupBox {
        GroupBox::new()
            .outline()
            .w_full()
            .content_style(StyleRefinement::default().p(rems(AGENT_GROUP_PADDING_REM)))
            .child(v_flex().w_full().children(
                agents.iter().enumerate().map(|(index, agent)| {
                    self.render_agent_row(agent, index != 0, interactive, cx)
                }),
            ))
    }

    fn render_agent_row(
        &self,
        agent: &AgentInfo,
        separated: bool,
        interactive: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let agent_key = agent.key.clone();
        let agent_key_delete = agent.key.clone();
        let enabled = agent.enabled;
        let is_custom = agent.is_custom;

        h_flex()
            .w_full()
            .when(separated, |row| row.border_t_1().border_color(theme.border))
            .h(rems(AGENT_ROW_HEIGHT_REM))
            .flex_shrink_0()
            .px_3()
            .py_1()
            .items_center()
            .gap_3()
            .child(
                div()
                    .w_1_3()
                    .min_w_0()
                    .text_sm()
                    .font_medium()
                    .truncate()
                    .child(agent.display_name.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .font_family(theme.mono_font_family.clone())
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .child(agent.skills_dir.to_string_lossy().into_owned()),
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
                            .disabled(!interactive)
                            .small()
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
                            .accessibility_label(agent.display_name.clone())
                            .disabled(!interactive)
                            .tab_stop(interactive)
                            .on_click(cx.listener(move |this, checked, _, cx| {
                                this.workspace.update(cx, |ws, cx| {
                                    ws.set_agent_enabled(&agent_key, *checked, cx)
                                });
                            })),
                    ),
            )
            .into_any_element()
    }
}

fn pick_custom_agent_directory(
    directory: WeakEntity<InputState>,
    workspace: WeakEntity<WorkspaceState>,
    prompt: &'static str,
    window: &mut Window,
    cx: &mut App,
) {
    let selection = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(prompt.into()),
    });
    window
        .spawn(cx, async move |cx| {
            let result = selection.await;
            let _ = cx.update(|window, cx| match result {
                Ok(Ok(Some(paths))) => {
                    if let Some(path) = paths.into_iter().next() {
                        let _ = directory.update(cx, |input, cx| {
                            input.set_value(path.to_string_lossy().into_owned(), window, cx);
                        });
                    }
                }
                Ok(Err(error)) => {
                    let _ = workspace.update(cx, |ws, cx| {
                        ws.notify_toast(true, error.to_string(), window, cx);
                    });
                }
                _ => {}
            });
        })
        .detach();
}

fn card_group_box(
    title: &'static str,
    card_bg: gpui_kit::Hsla,
    radius_lg: gpui_kit::Pixels,
) -> GroupBox {
    GroupBox::new().outline().title(title).content_style(
        gpui_kit::StyleRefinement::default()
            .bg(card_bg)
            .rounded(radius_lg),
    )
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

fn setting_stacked_field(
    title: &'static str,
    description: &'static str,
    muted_fg: gpui_kit::Hsla,
    control: impl IntoElement,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2()
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .child(div().text_sm().font_medium().child(title))
                .child(div().text_xs().text_color(muted_fg).child(description)),
        )
        .child(div().w_full().child(control))
}
