mod sections;

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::{
    ActiveTheme, IndexPath,
    button::{Button, ButtonVariants},
    h_flex,
    input::InputState,
    scroll::ScrollableElement,
    select::{SearchableVec, SelectEvent, SelectState},
    v_flex,
};
use gpui_kit::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};

use crate::components::page_header::PageHeader;
use crate::fonts::{FONT_AUTO, installed_font_families};
use crate::i18n::Lang;
use crate::state::{SettingsSection, WorkspaceEvent, WorkspaceState};

/// Settings page: language dropdown (zh-CN / en / ja), searchable UI & Mono
/// font pickers with language-aware Auto mode, base font size, deploy mode,
/// persisted appearance, network proxy / GitHub token, software updates,
/// central library root, and AI agent registry (including custom agents).
pub struct SettingsView {
    pub(super) workspace: Entity<WorkspaceState>,
    pub(super) active_section: SettingsSection,
    pub(super) lang_select: Entity<SelectState<Vec<SharedString>>>,
    pub(super) ui_font_select: Entity<SelectState<SearchableVec<SharedString>>>,
    pub(super) mono_font_select: Entity<SelectState<SearchableVec<SharedString>>>,
    pub(super) http_proxy_input: Entity<InputState>,
    pub(super) github_token_input: Entity<InputState>,
    pub(super) backup_repo_input: Entity<InputState>,
    pub(super) backup_branch_input: Entity<InputState>,
    installed_fonts: Vec<String>,
    pub(super) show_undetected_agents: bool,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(
        workspace: Entity<WorkspaceState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let installed_fonts = installed_font_families(cx);
        let (
            lang,
            ui_font_pref,
            mono_font_pref,
            saved_proxy,
            saved_token,
            saved_backup_repo,
            saved_backup_branch,
        ) = {
            let ws = workspace.read(cx);
            (
                ws.lang(),
                ws.font_settings().ui_font.clone(),
                ws.font_settings().mono_font.clone(),
                ws.manager().http_proxy().unwrap_or_default(),
                ws.manager().github_token().unwrap_or_default(),
                ws.manager().backup_repo_url().unwrap_or_default(),
                ws.manager().backup_branch(),
            )
        };
        let s = lang.t();

        let lang_items: Vec<SharedString> = Lang::ALL
            .iter()
            .map(|l| SharedString::from(l.display_name()))
            .collect();
        let lang_select = cx.new(|cx| {
            SelectState::new(
                lang_items,
                Some(IndexPath::default().row(lang.index())),
                window,
                cx,
            )
        });

        let (ui_items, ui_idx) =
            build_font_options(s.font_auto_by_lang, &installed_fonts, &ui_font_pref);
        let ui_font_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(ui_items),
                Some(IndexPath::default().row(ui_idx)),
                window,
                cx,
            )
            .searchable(true)
        });

        let (mono_items, mono_idx) =
            build_font_options(s.font_auto_mono, &installed_fonts, &mono_font_pref);
        let mono_font_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(mono_items),
                Some(IndexPath::default().row(mono_idx)),
                window,
                cx,
            )
            .searchable(true)
        });

        let http_proxy_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder(s.ph_http_proxy);
            if !saved_proxy.is_empty() {
                input.set_value(saved_proxy, window, cx);
            }
            input
        });
        let github_token_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder(s.ph_github_token);
            if !saved_token.is_empty() {
                input.set_value(saved_token, window, cx);
            }
            input
        });
        let backup_repo_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder(s.ph_git_backup_repo);
            if !saved_backup_repo.is_empty() {
                input.set_value(saved_backup_repo, window, cx);
            }
            input
        });
        let backup_branch_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("main");
            input.set_value(saved_backup_branch, window, cx);
            input
        });

        let sub_lang = cx.subscribe_in(
            &lang_select,
            window,
            |this, _state, event: &SelectEvent<Vec<SharedString>>, _window, cx| {
                let SelectEvent::Confirm(Some(chosen)) = event else {
                    return;
                };
                if let Some(new_lang) = Lang::from_display_name(chosen.as_ref()) {
                    this.workspace
                        .update(cx, |ws, cx| ws.set_language(new_lang, cx));
                }
            },
        );

        let sub_ui_font = cx.subscribe_in(
            &ui_font_select,
            window,
            |this, _state, event: &SelectEvent<SearchableVec<SharedString>>, _window, cx| {
                let SelectEvent::Confirm(Some(chosen)) = event else {
                    return;
                };
                let auto_label = this.workspace.read(cx).t().font_auto_by_lang;
                let value = if chosen.as_ref() == auto_label {
                    FONT_AUTO.to_string()
                } else {
                    chosen.to_string()
                };
                this.workspace
                    .update(cx, |ws, cx| ws.set_ui_font(value, cx));
            },
        );

        let sub_mono_font = cx.subscribe_in(
            &mono_font_select,
            window,
            |this, _state, event: &SelectEvent<SearchableVec<SharedString>>, _window, cx| {
                let SelectEvent::Confirm(Some(chosen)) = event else {
                    return;
                };
                let auto_label = this.workspace.read(cx).t().font_auto_mono;
                let value = if chosen.as_ref() == auto_label {
                    FONT_AUTO.to_string()
                } else {
                    chosen.to_string()
                };
                this.workspace
                    .update(cx, |ws, cx| ws.set_mono_font(value, cx));
            },
        );

        let sub_ws = cx.subscribe_in(
            &workspace,
            window,
            |this, ws, event: &WorkspaceEvent, window, cx| match event {
                WorkspaceEvent::LanguageChanged => {
                    let lang = ws.read(cx).lang();
                    let fs = ws.read(cx).font_settings().clone();
                    let s = lang.t();

                    this.lang_select.update(cx, |sel, cx| {
                        sel.set_selected_index(
                            Some(IndexPath::default().row(lang.index())),
                            window,
                            cx,
                        );
                    });

                    let (ui_items, ui_idx) =
                        build_font_options(s.font_auto_by_lang, &this.installed_fonts, &fs.ui_font);
                    this.ui_font_select.update(cx, |sel, cx| {
                        sel.set_items(SearchableVec::new(ui_items), window, cx);
                        sel.set_selected_index(Some(IndexPath::default().row(ui_idx)), window, cx);
                    });

                    let (mono_items, mono_idx) =
                        build_font_options(s.font_auto_mono, &this.installed_fonts, &fs.mono_font);
                    this.mono_font_select.update(cx, |sel, cx| {
                        sel.set_items(SearchableVec::new(mono_items), window, cx);
                        sel.set_selected_index(
                            Some(IndexPath::default().row(mono_idx)),
                            window,
                            cx,
                        );
                    });

                    this.http_proxy_input.update(cx, |input, cx| {
                        input.set_placeholder(s.ph_http_proxy, window, cx);
                    });
                    this.github_token_input.update(cx, |input, cx| {
                        input.set_placeholder(s.ph_github_token, window, cx);
                    });
                    this.backup_repo_input.update(cx, |input, cx| {
                        input.set_placeholder(s.ph_git_backup_repo, window, cx);
                    });
                    cx.notify();
                }
                WorkspaceEvent::NavigateToSettings(section) => {
                    this.active_section = *section;
                    cx.notify();
                }
                WorkspaceEvent::Reloaded | WorkspaceEvent::SettingsChanged => cx.notify(),
                _ => {}
            },
        );

        Self {
            workspace,
            active_section: SettingsSection::General,
            lang_select,
            ui_font_select,
            mono_font_select,
            http_proxy_input,
            github_token_input,
            backup_repo_input,
            backup_branch_input,
            installed_fonts,
            show_undetected_agents: false,
            _subscriptions: vec![sub_lang, sub_ui_font, sub_mono_font, sub_ws],
        }
    }
}

fn build_font_options(
    auto_label: &'static str,
    installed: &[String],
    selected_pref: &str,
) -> (Vec<SharedString>, usize) {
    let mut items = Vec::with_capacity(installed.len() + 1);
    items.push(SharedString::from(auto_label));
    let mut selected_idx = 0usize;

    for (i, font) in installed.iter().enumerate() {
        if selected_pref != FONT_AUTO && font.eq_ignore_ascii_case(selected_pref) {
            selected_idx = i + 1;
        }
        items.push(SharedString::from(font.clone()));
    }
    (items, selected_idx)
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.workspace.read(cx).t();
        let active = self.active_section;
        let theme = cx.theme();
        let border = theme.border;
        let primary = theme.primary;

        let sections = [
            (
                SettingsSection::General,
                s.settings_general,
                AssetIcon::SlidersHorizontal,
            ),
            (
                SettingsSection::Appearance,
                s.settings_appearance_tab,
                AssetIcon::Palette,
            ),
            (
                SettingsSection::SyncAndNetwork,
                s.settings_network,
                AssetIcon::Cloud,
            ),
            (
                SettingsSection::Agents,
                s.settings_agents_registry,
                AssetIcon::Bot,
            ),
            (
                SettingsSection::Updates,
                s.settings_updates_tab,
                AssetIcon::Info,
            ),
        ];
        let section_content = match active {
            SettingsSection::General => self.render_general_section(cx).into_any_element(),
            SettingsSection::Appearance => v_flex()
                .w_full()
                .gap_4()
                .child(self.render_appearance_section(cx))
                .child(self.render_fonts_section(cx))
                .into_any_element(),
            SettingsSection::SyncAndNetwork => v_flex()
                .w_full()
                .gap_4()
                .child(self.render_network_section(cx))
                .child(self.render_git_backup_section(cx))
                .into_any_element(),
            SettingsSection::Agents => self.render_agents_section(_window, cx).into_any_element(),
            SettingsSection::Updates => self.render_updates_section(cx).into_any_element(),
        };

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(PageHeader::new(s.settings_title))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .items_start()
                    .child(
                        v_flex()
                            .h_full()
                            .w(px(184.))
                            .flex_shrink_0()
                            .gap_1()
                            .p_3()
                            .border_r_1()
                            .border_color(border)
                            .children(sections.map(|(section, label, icon)| {
                                Button::new(format!("settings-{section:?}"))
                                    .ghost()
                                    .w_full()
                                    .icon(icon)
                                    .accessibility_label(label)
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_left()
                                            .truncate()
                                            .child(label),
                                    )
                                    .when(active == section, |button| {
                                        button.text_color(primary).bg(primary.opacity(0.12))
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.active_section = section;
                                        cx.notify();
                                    }))
                            })),
                    )
                    .child(
                        div()
                            .id(format!("settings-scroll-{active:?}"))
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .child(div().w_full().max_w(px(720.)).p_5().child(section_content)),
                    ),
            )
    }
}
