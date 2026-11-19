mod sections;

use gpui_kit::component::{
    IndexPath, StyledExt,
    input::InputState,
    scroll::ScrollableElement,
    select::{SearchableVec, SelectEvent, SelectState},
    v_flex,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div,
};

use crate::fonts::{FONT_AUTO, installed_font_families};
use crate::i18n::Lang;
use crate::state::{WorkspaceEvent, WorkspaceState};

/// Settings page: language dropdown (zh-CN / en / ja), searchable UI & Mono
/// font pickers with language-aware Auto mode, base font size, deploy mode,
/// persisted appearance, network proxy / GitHub token, software updates,
/// central library root, and AI agent registry (including custom agents).
pub struct SettingsView {
    pub(super) workspace: Entity<WorkspaceState>,
    pub(super) lang_select: Entity<SelectState<Vec<SharedString>>>,
    pub(super) ui_font_select: Entity<SelectState<SearchableVec<SharedString>>>,
    pub(super) mono_font_select: Entity<SelectState<SearchableVec<SharedString>>>,
    pub(super) http_proxy_input: Entity<InputState>,
    pub(super) github_token_input: Entity<InputState>,
    pub(super) custom_agent_name_input: Entity<InputState>,
    pub(super) custom_agent_dir_input: Entity<InputState>,
    installed_fonts: Vec<String>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(
        workspace: Entity<WorkspaceState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let installed_fonts = installed_font_families(cx);
        let (lang, ui_font_pref, mono_font_pref, saved_proxy, saved_token) = {
            let ws = workspace.read(cx);
            (
                ws.lang(),
                ws.font_settings().ui_font.clone(),
                ws.font_settings().mono_font.clone(),
                ws.manager().http_proxy().unwrap_or_default(),
                ws.manager().github_token().unwrap_or_default(),
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
        let custom_agent_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_custom_agent_name));
        let custom_agent_dir_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_custom_agent_dir));

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
                    this.custom_agent_name_input.update(cx, |input, cx| {
                        input.set_placeholder(s.ph_custom_agent_name, window, cx);
                    });
                    this.custom_agent_dir_input.update(cx, |input, cx| {
                        input.set_placeholder(s.ph_custom_agent_dir, window, cx);
                    });
                    cx.notify();
                }
                WorkspaceEvent::Reloaded | WorkspaceEvent::SettingsChanged => cx.notify(),
                _ => {}
            },
        );

        Self {
            workspace,
            lang_select,
            ui_font_select,
            mono_font_select,
            http_proxy_input,
            github_token_input,
            custom_agent_name_input,
            custom_agent_dir_input,
            installed_fonts,
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

        div()
            .size_full()
            .min_h_0()
            .overflow_y_scrollbar()
            .id("settings-scroll")
            .child(
                v_flex()
                    .w_full()
                    .p_5()
                    .gap_5()
                    .child(div().text_lg().font_semibold().child(s.settings_title))
                    .child(self.render_general_section(cx))
                    .child(self.render_fonts_section(cx))
                    .child(self.render_network_section(cx))
                    .child(self.render_updates_section(cx))
                    .child(self.render_agents_section(cx)),
            )
    }
}
