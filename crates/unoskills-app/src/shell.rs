use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Side, Sizable, StyledExt, ThemeMode, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarMenu, SidebarMenuItem},
    spinner::Spinner,
    v_flex,
};
use gpui_kit::{
    AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, Window, actions, div, prelude::FluentBuilder,
};

use crate::assets::AppIcon;
use crate::components::badges::StatusBadge;
use crate::features::DiscoverView;
use crate::features::{LibraryView, SettingsView, StacksView};
use crate::state::{NavTab, WorkspaceEvent, WorkspaceState};

actions!(unoskills, [FocusSearch, SaveSkill, CancelEdit]);

/// Desktop shell: owns the shared `WorkspaceState` entity, the active tab,
/// the collapsible sidebar, and the four feature view entities. Navigation
/// flows through `WorkspaceEvent::NavigateTo`, so every tab switch (sidebar,
/// empty-state buttons, post-install jumps) funnels through one code path.
pub struct UnoSkillsShell {
    focus_handle: FocusHandle,
    workspace: Entity<WorkspaceState>,
    active_tab: NavTab,
    sidebar_collapsed: bool,
    library_view: Entity<LibraryView>,
    stacks_view: Entity<StacksView>,
    discover_view: Entity<DiscoverView>,
    settings_view: Entity<SettingsView>,
    _subscription: Subscription,
}

impl UnoSkillsShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspace = cx.new(|_| WorkspaceState::new());
        // Apply persisted Light/Dark mode and language-aware UI & mono fonts at startup.
        workspace.update(cx, |ws, cx| ws.apply_initial_appearance(window, cx));
        let library_view = cx.new(|cx| LibraryView::new(workspace.clone(), window, cx));
        let stacks_view = cx.new(|cx| StacksView::new(workspace.clone(), window, cx));
        let discover_view = cx.new(|cx| DiscoverView::new(workspace.clone(), window, cx));
        let settings_view = cx.new(|cx| SettingsView::new(workspace.clone(), window, cx));

        let subscription = cx.subscribe_in(
            &workspace,
            window,
            |this, _ws, event: &WorkspaceEvent, _window, cx| {
                match event {
                    WorkspaceEvent::NavigateTo(tab) => {
                        this.active_tab = *tab;
                        cx.notify();
                    }
                    // Counts, labels, theme, and the busy spinner all live here.
                    WorkspaceEvent::Reloaded
                    | WorkspaceEvent::SettingsChanged
                    | WorkspaceEvent::LanguageChanged
                    | WorkspaceEvent::BusyChanged => cx.notify(),
                    WorkspaceEvent::InstallFinished(_) => {}
                }
            },
        );

        Self {
            focus_handle: cx.focus_handle(),
            workspace,
            active_tab: NavTab::Library,
            sidebar_collapsed: false,
            library_view,
            stacks_view,
            discover_view,
            settings_view,
            _subscription: subscription,
        }
    }

    fn toast(
        &self,
        is_error: bool,
        message: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.workspace
            .update(cx, |ws, cx| ws.notify_toast(is_error, message, window, cx));
    }
}

impl Render for UnoSkillsShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (bg, fg, muted_fg, is_dark) = {
            let theme = cx.theme();
            (
                theme.background,
                theme.foreground,
                theme.muted_foreground,
                theme.is_dark(),
            )
        };
        let s = self.workspace.read(cx).t();
        let current_lang = self.workspace.read(cx).lang();
        let installed_agent_count = self.workspace.read(cx).installed_agent_count();
        let skill_count = self.workspace.read(cx).skills().len();
        let stack_count = {
            let ws = self.workspace.read(cx);
            let featured = ws.featured_stacks();
            let extra_community = ws
                .community_stacks()
                .iter()
                .filter(|c| !featured.iter().any(|f| f.id == c.id))
                .count();
            featured.len() + extra_community + ws.local_stacks().len()
        };
        let adopt_count = self.workspace.read(cx).adopt_candidates().len();
        let busy_label = self.workspace.read(cx).busy_label().map(str::to_string);

        let active_view = match self.active_tab {
            NavTab::Library => self.library_view.clone().into_any_element(),
            NavTab::Stacks => self.stacks_view.clone().into_any_element(),
            NavTab::Discover => self.discover_view.clone().into_any_element(),
            NavTab::Settings => self.settings_view.clone().into_any_element(),
        };

        v_flex()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.active_tab = NavTab::Library;
                this.library_view
                    .update(cx, |view, cx| view.focus_search(window, cx));
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SaveSkill, window, cx| {
                if this.active_tab == NavTab::Library {
                    this.library_view
                        .update(cx, |view, cx| view.handle_save_shortcut(window, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &CancelEdit, window, cx| {
                if this.active_tab == NavTab::Library {
                    this.library_view
                        .update(cx, |view, cx| view.handle_cancel_edit_shortcut(window, cx));
                }
            }))
            .size_full()
            .bg(bg)
            .text_color(fg)
            .child(
                TitleBar::new().child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .pr_2()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(Icon::new(AppIcon::UnoMark).size_4())
                                .child(div().text_sm().font_semibold().child("UnoSkills"))
                                .child(StatusBadge::secondary(concat!(
                                    "v",
                                    env!("CARGO_PKG_VERSION")
                                ))),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .when_some(busy_label, |row, label| {
                                    row.child(
                                        h_flex()
                                            .items_center()
                                            .gap_1()
                                            .child(Spinner::new().xsmall())
                                            .child(
                                                div().text_xs().text_color(muted_fg).child(label),
                                            ),
                                    )
                                })
                                .child(
                                    Button::new("quick-toggle-lang")
                                        .ghost()
                                        .xsmall()
                                        .label(current_lang.short_label())
                                        .tooltip(s.tooltip_lang)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let next = current_lang.next();
                                            this.workspace
                                                .update(cx, |ws, cx| ws.set_language(next, cx));
                                        })),
                                )
                                .child(
                                    Button::new("refresh-all")
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::RefreshCw)
                                        .tooltip(s.tooltip_refresh)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.workspace.update(cx, |ws, cx| ws.reload_all(cx));
                                            let msg = this.workspace.read(cx).t().toast_refreshed;
                                            this.toast(false, msg, window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("toggle-theme")
                                        .ghost()
                                        .xsmall()
                                        .icon(if is_dark {
                                            IconName::Sun
                                        } else {
                                            IconName::Moon
                                        })
                                        .tooltip(s.tooltip_theme)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            let next = if is_dark {
                                                ThemeMode::Light
                                            } else {
                                                ThemeMode::Dark
                                            };
                                            this.workspace.update(cx, |ws, cx| {
                                                ws.set_theme_mode(next, window, cx)
                                            });
                                        })),
                                ),
                        ),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(
                        Sidebar::new("main-sidebar")
                            .side(Side::Left)
                            .collapsible(true)
                            .collapsed(self.sidebar_collapsed)
                            .child(
                                SidebarGroup::new(s.workspace_group).child(
                                    SidebarMenu::new()
                                        .child(
                                            SidebarMenuItem::new(s.nav_library)
                                                .icon(IconName::BookOpen)
                                                .active(self.active_tab == NavTab::Library)
                                                .suffix(move |_, _| {
                                                    StatusBadge::secondary(format!("{skill_count}"))
                                                })
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.workspace.update(cx, |ws, cx| {
                                                        ws.navigate(NavTab::Library, cx)
                                                    });
                                                })),
                                        )
                                        .child(
                                            SidebarMenuItem::new(s.nav_stacks)
                                                .icon(IconName::LayoutDashboard)
                                                .active(self.active_tab == NavTab::Stacks)
                                                .suffix(move |_, _| {
                                                    StatusBadge::secondary(format!("{stack_count}"))
                                                })
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.workspace.update(cx, |ws, cx| {
                                                        ws.navigate(NavTab::Stacks, cx)
                                                    });
                                                })),
                                        )
                                        .child(
                                            SidebarMenuItem::new(s.nav_discover)
                                                .icon(IconName::Globe)
                                                .active(self.active_tab == NavTab::Discover)
                                                .when(adopt_count > 0, |item| {
                                                    item.suffix(move |_, _| {
                                                        StatusBadge::warning(format!(
                                                            "+{adopt_count}"
                                                        ))
                                                        .outline()
                                                    })
                                                })
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.workspace.update(cx, |ws, cx| {
                                                        ws.navigate(NavTab::Discover, cx)
                                                    });
                                                })),
                                        )
                                        .child(
                                            SidebarMenuItem::new(s.nav_settings)
                                                .icon(IconName::Settings)
                                                .active(self.active_tab == NavTab::Settings)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.workspace.update(cx, |ws, cx| {
                                                        ws.navigate(NavTab::Settings, cx)
                                                    });
                                                })),
                                        ),
                                ),
                            )
                            .footer(
                                SidebarFooter::new().child(
                                    h_flex()
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .when(!self.sidebar_collapsed, |row| {
                                            row.child(div().text_xs().text_color(muted_fg).child(
                                                format!(
                                                    "{installed_agent_count} {}",
                                                    s.agents_detected_suffix
                                                ),
                                            ))
                                        })
                                        .child(
                                            Button::new("collapse-sidebar")
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::PanelLeft)
                                                .tooltip(s.workspace_group)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.sidebar_collapsed =
                                                        !this.sidebar_collapsed;
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .size_full()
                            .overflow_hidden()
                            .child(active_view),
                    ),
            )
    }
}
