use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Side, Sizable, StyledExt, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
    spinner::Spinner,
    v_flex,
};
use gpui_kit::{
    AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, Window, actions, div, prelude::FluentBuilder, px,
};

use crate::assets::AppIcon;
use crate::components::badges::StatusBadge;
use crate::features::{DiscoverView, HomeView, LibraryView, SettingsView, StacksView};
use crate::state::{NavTab, SettingsSection, WorkspaceEvent, WorkspaceState};

actions!(unoskills, [FocusSearch, SaveSkill, CancelEdit]);

/// Desktop shell: owns the shared `WorkspaceState` entity, the active tab,
/// the collapsible sidebar, and the feature view entities. Navigation
/// flows through `WorkspaceEvent::NavigateTo`, so every tab switch (sidebar,
/// empty-state buttons, post-install jumps) funnels through one code path.
pub struct UnoSkillsShell {
    focus_handle: FocusHandle,
    workspace: Entity<WorkspaceState>,
    active_tab: NavTab,
    sidebar_collapsed: bool,
    home_view: Entity<HomeView>,
    library_view: Entity<LibraryView>,
    stacks_view: Entity<StacksView>,
    discover_view: Entity<DiscoverView>,
    settings_view: Entity<SettingsView>,
    _subscription: Subscription,
}

impl UnoSkillsShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspace = cx.new(|_| WorkspaceState::new());
        workspace.update(cx, |ws, cx| ws.apply_initial_appearance(window, cx));
        let appearance_subscription = cx.observe_window_appearance(window, |this, window, cx| {
            this.workspace
                .update(cx, |ws, cx| ws.sync_appearance(window, cx));
        });
        let library_view = cx.new(|cx| LibraryView::new(workspace.clone(), window, cx));
        let stacks_view = cx.new(|cx| StacksView::new(workspace.clone(), window, cx));
        let discover_view = cx.new(|cx| DiscoverView::new(workspace.clone(), window, cx));
        let settings_view = cx.new(|cx| SettingsView::new(workspace.clone(), window, cx));
        let home_view = cx.new(|cx| HomeView::new(workspace.clone(), window, cx));

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
                    WorkspaceEvent::InstallStateChanged
                    | WorkspaceEvent::NavigateToSettings(_)
                    | WorkspaceEvent::OpenMyStacks
                    | WorkspaceEvent::OpenLocalSkills
                    | WorkspaceEvent::OpenMarketBoard(_)
                    | WorkspaceEvent::MarketFirstPageChanged(_)
                    | WorkspaceEvent::OpenInstallDialog
                    | WorkspaceEvent::SelectLibrarySkill(_) => {}
                }
            },
        );

        let sidebar_collapsed = workspace
            .read(cx)
            .manager()
            .get_setting("sidebar_collapsed")
            .ok()
            .flatten()
            .is_some_and(|value| value == "true");

        Self {
            focus_handle: cx.focus_handle(),
            workspace,
            active_tab: NavTab::Home,
            sidebar_collapsed,
            home_view,
            library_view,
            stacks_view,
            discover_view,
            settings_view,
            _subscription: Subscription::join(subscription, appearance_subscription),
        }
    }
}

impl Render for UnoSkillsShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (bg, fg, muted_fg, accent) = (
            theme.background,
            theme.foreground,
            theme.muted_foreground,
            theme.primary,
        );
        let s = self.workspace.read(cx).t();
        let adopt_count = self.workspace.read(cx).adopt_candidates().len();
        let busy_label = self.workspace.read(cx).busy_label().map(str::to_string);
        let narrow = window.viewport_size().width < px(1080.);
        let collapsed = narrow || self.sidebar_collapsed;
        let available_update_tag = self
            .workspace
            .read(cx)
            .available_update()
            .filter(|u| u.update_available)
            .and_then(|u| u.latest.as_ref())
            .map(|r| r.tag_name.clone());

        let active_view = match self.active_tab {
            NavTab::Home => self.home_view.clone().into_any_element(),
            NavTab::Library => self.library_view.clone().into_any_element(),
            NavTab::Stacks => self.stacks_view.clone().into_any_element(),
            NavTab::Discover => self.discover_view.clone().into_any_element(),
            NavTab::Settings => self.settings_view.clone().into_any_element(),
        };

        v_flex()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                match this.active_tab {
                    NavTab::Library => this
                        .library_view
                        .update(cx, |view, cx| view.focus_search(window, cx)),
                    NavTab::Stacks => this
                        .stacks_view
                        .update(cx, |view, cx| view.focus_search(window, cx)),
                    NavTab::Discover => this
                        .discover_view
                        .update(cx, |view, cx| view.focus_search(window, cx)),
                    NavTab::Home | NavTab::Settings => {
                        this.active_tab = NavTab::Library;
                        this.library_view
                            .update(cx, |view, cx| view.focus_search(window, cx));
                        cx.notify();
                    }
                }
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
                TitleBar::new().h(px(36.)).child(
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
                                .when(!narrow, |row| {
                                    let label = if collapsed {
                                        s.expand_sidebar
                                    } else {
                                        s.collapse_sidebar
                                    };
                                    row.child(
                                        Button::new("collapse-sidebar")
                                            .ghost()
                                            .small()
                                            .icon(IconName::PanelLeft)
                                            .tooltip(label)
                                            .accessibility_label(label)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.sidebar_collapsed = !this.sidebar_collapsed;
                                                let value = if this.sidebar_collapsed {
                                                    "true"
                                                } else {
                                                    "false"
                                                };
                                                this.workspace.update(cx, |ws, cx| {
                                                    if let Err(error) = ws
                                                        .manager()
                                                        .set_setting("sidebar_collapsed", value)
                                                    {
                                                        ws.notify_toast(
                                                            true,
                                                            error.to_string(),
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                });
                                                cx.notify();
                                            })),
                                    )
                                })
                                .when_some(available_update_tag, |row, tag| {
                                    row.child(
                                        Button::new("titlebar-update-btn")
                                            .primary()
                                            .xsmall()
                                            .icon(IconName::ArrowUp)
                                            .label(format!("{} {tag}", s.badge_update_available))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.workspace.update(cx, |ws, cx| {
                                                    ws.navigate_settings(
                                                        SettingsSection::Updates,
                                                        cx,
                                                    );
                                                });
                                            })),
                                    )
                                }),
                        )
                        .child(h_flex().min_w_0().items_center().gap_2().when_some(
                            busy_label,
                            |row, label| {
                                row.child(Spinner::new().xsmall()).child(
                                    div()
                                        .min_w_0()
                                        .text_xs()
                                        .text_color(muted_fg)
                                        .truncate()
                                        .child(label),
                                )
                            },
                        )),
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
                            .w(px(184.))
                            .collapsed(collapsed)
                            .child(
                                SidebarGroup::new("").child(
                                    SidebarMenu::new().children(
                                        [
                                            (NavTab::Home, s.nav_home, AssetIcon::House),
                                            (NavTab::Library, s.nav_library, AssetIcon::BookOpen),
                                            (
                                                NavTab::Stacks,
                                                s.nav_stacks,
                                                AssetIcon::LayoutDashboard,
                                            ),
                                            (NavTab::Discover, s.nav_discover, AssetIcon::Globe),
                                        ]
                                        .map(
                                            |(tab, label, icon)| {
                                                SidebarMenuItem::new(label)
                                                    .icon(icon)
                                                    .active(self.active_tab == tab)
                                                    .when(
                                                        tab == NavTab::Discover && adopt_count > 0,
                                                        |item| {
                                                            item.suffix(move |_, _| {
                                                                StatusBadge::warning(format!(
                                                                    "+{adopt_count}"
                                                                ))
                                                                .outline()
                                                            })
                                                        },
                                                    )
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.workspace.update(cx, |ws, cx| {
                                                            ws.navigate(tab, cx)
                                                        });
                                                    }))
                                            },
                                        ),
                                    ),
                                ),
                            )
                            .footer(
                                v_flex().w_full().items_center().child(
                                    sidebar_footer_item(
                                        "nav-settings",
                                        IconName::Settings,
                                        s.nav_settings,
                                        collapsed,
                                    )
                                    .when(self.active_tab == NavTab::Settings, |button| {
                                        button.text_color(accent).bg(accent.opacity(0.12))
                                    })
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.workspace.update(cx, |ws, cx| {
                                                ws.navigate(NavTab::Settings, cx)
                                            });
                                        },
                                    )),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .size_full()
                            .overflow_hidden()
                            .child(active_view),
                    ),
            )
    }
}

fn sidebar_footer_item(
    id: &'static str,
    icon: IconName,
    label: impl Into<SharedString>,
    collapsed: bool,
) -> Button {
    let label = label.into();
    let button = Button::new(id)
        .ghost()
        .small()
        .tooltip(label.clone())
        .accessibility_label(label.clone());

    if collapsed {
        button.icon(icon)
    } else {
        button.w_full().h_7().child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .child(Icon::new(icon).size_4().flex_shrink_0())
                .child(div().min_w_0().truncate().child(label)),
        )
    }
}
