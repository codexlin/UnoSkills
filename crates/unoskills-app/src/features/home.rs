use std::cmp::Reverse;

use chrono::{DateTime, Days, Local, NaiveDate};
use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    chart::BarChart,
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    scroll::ScrollableElement,
    spinner::Spinner,
    tab::{Tab, TabBar},
    v_flex,
};
use gpui_kit::{
    AnyElement, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StyleRefinement, Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};
use unoskills_core::{Skill, source::skillssh_detail_url};

use crate::components::badges::format_install_count;
use crate::components::page_header::PageHeader;
use crate::state::{
    MarketBoard, MarketFirstPage, NavTab, SettingsSection, WorkspaceEvent, WorkspaceState,
};

const HOME_BOARDS: [MarketBoard; 2] = [MarketBoard::Trending, MarketBoard::AllTime];
const INTAKE_DAYS: usize = 7;

#[derive(Clone)]
struct IntakeDay {
    label: SharedString,
    count: u32,
}

struct LocalStats {
    deployed: usize,
    intake: Vec<IntakeDay>,
    recent: Vec<RecentSkill>,
}

struct RecentSkill {
    id: String,
    name: String,
    date: Option<SharedString>,
}

impl LocalStats {
    fn from_skills(skills: &[Skill]) -> Self {
        let today = Local::now().date_naive();
        let dates = skills.iter().filter_map(|skill| {
            DateTime::from_timestamp(skill.created_at, 0)
                .map(|time| time.with_timezone(&Local).date_naive())
        });
        let mut recent: Vec<_> = skills.iter().collect();
        recent.sort_by_key(|skill| Reverse(skill.created_at));
        Self {
            deployed: skills
                .iter()
                .filter(|skill| {
                    !skill.deployments.is_empty() || !skill.project_deployments.is_empty()
                })
                .count(),
            intake: intake_days(today, dates),
            recent: recent
                .into_iter()
                .take(3)
                .map(|skill| RecentSkill {
                    id: skill.id.clone(),
                    name: skill.name.clone(),
                    date: DateTime::from_timestamp(skill.created_at, 0).map(|time| {
                        time.with_timezone(&Local)
                            .format("%Y-%m-%d")
                            .to_string()
                            .into()
                    }),
                })
                .collect(),
        }
    }
}

fn intake_days(today: NaiveDate, dates: impl IntoIterator<Item = NaiveDate>) -> Vec<IntakeDay> {
    let mut counts = [0; INTAKE_DAYS];
    for date in dates {
        let days_ago = today.signed_duration_since(date).num_days();
        if let Ok(index) = usize::try_from(days_ago)
            && let Some(count) = counts.get_mut(index)
        {
            *count += 1;
        }
    }
    counts
        .into_iter()
        .enumerate()
        .rev()
        .filter_map(|(days_ago, count)| {
            today
                .checked_sub_days(Days::new(days_ago as u64))
                .map(|date| IntakeDay {
                    label: date.format("%m/%d").to_string().into(),
                    count,
                })
        })
        .collect()
}

pub struct HomeView {
    workspace: Entity<WorkspaceState>,
    snapshot: LocalStats,
    board: MarketBoard,
    _workspace_subscription: Subscription,
}

impl HomeView {
    pub fn new(
        workspace: Entity<WorkspaceState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let board = HOME_BOARDS[0];
        let snapshot = LocalStats::from_skills(workspace.read(cx).skills());
        let subscription =
            cx.subscribe_in(&workspace, window, |this, ws, event, _, cx| match event {
                WorkspaceEvent::Reloaded | WorkspaceEvent::NavigateTo(NavTab::Home) => {
                    this.snapshot = LocalStats::from_skills(ws.read(cx).skills());
                    cx.notify();
                }
                WorkspaceEvent::MarketFirstPageChanged(board) if *board == this.board => {
                    cx.notify()
                }
                WorkspaceEvent::LanguageChanged | WorkspaceEvent::SettingsChanged => cx.notify(),
                _ => {}
            });
        workspace.update(cx, |ws, cx| {
            ws.load_market_first_page(board, false, window, cx)
        });
        Self {
            workspace,
            snapshot,
            board,
            _workspace_subscription: subscription,
        }
    }

    fn overview(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let metrics = [
            (s.nav_library, ws.skills().len(), AssetIcon::BookOpen),
            (s.home_deployed, self.snapshot.deployed, AssetIcon::Network),
            (s.home_agents, ws.installed_agent_count(), AssetIcon::Bot),
            (
                s.discover_adopt_tab,
                ws.adopt_candidates().len(),
                AssetIcon::ArchiveRestore,
            ),
        ];
        v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .min_h(px(32.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_lg()
                            .font_semibold()
                            .child(s.home_workspace),
                    )
                    .child(
                        Button::new("home-open-my-stacks")
                            .outline()
                            .small()
                            .flex_shrink_0()
                            .icon(AssetIcon::Layers)
                            .label(s.tab_my_stacks)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.workspace.update(cx, |ws, cx| ws.open_my_stacks(cx));
                            })),
                    ),
            )
            .child(
                GroupBox::new()
                    .id("home-overview")
                    .outline()
                    .content_style(
                        StyleRefinement::default()
                            .rounded(theme.radius_lg)
                            .p_1()
                            .gap_0(),
                    )
                    .children(
                        metrics
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .enumerate()
                            .map(|(row, items)| {
                                h_flex()
                                    .w_full()
                                    .when(row > 0, |line| {
                                        line.border_t_1().border_color(theme.border)
                                    })
                                    .children(items.iter().enumerate().map(
                                        |(column, &(label, count, icon))| {
                                            let index = row * 2 + column;
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .p_1()
                                                .when(column > 0, |cell| {
                                                    cell.border_l_1().border_color(theme.border)
                                                })
                                                .child(
                                                    Button::new(("home-metric", index))
                                                        .ghost()
                                                        .w_full()
                                                        .min_w_0()
                                                        .h(px(44.))
                                                        .accessibility_label(
                                                            ws.lang()
                                                                .label_with_count(label, count),
                                                        )
                                                        .child(
                                                            Icon::new(icon)
                                                                .size_4()
                                                                .text_color(muted)
                                                                .flex_shrink_0(),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .truncate()
                                                                .text_left()
                                                                .child(label),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_shrink_0()
                                                                .text_xl()
                                                                .font_semibold()
                                                                .child(count.to_string()),
                                                        )
                                                        .child(
                                                            Icon::new(IconName::ChevronRight)
                                                                .size_3()
                                                                .text_color(muted)
                                                                .flex_shrink_0(),
                                                        )
                                                        .on_click(cx.listener(
                                                            move |view, _, _, cx| {
                                                                view.workspace.update(
                                                                    cx,
                                                                    |ws, cx| match index {
                                                                        2 => ws.navigate_settings(
                                                                            SettingsSection::Agents,
                                                                            cx,
                                                                        ),
                                                                        3 => {
                                                                            ws.open_local_skills(cx)
                                                                        }
                                                                        _ => ws.navigate(
                                                                            NavTab::Library,
                                                                            cx,
                                                                        ),
                                                                    },
                                                                );
                                                            },
                                                        )),
                                                )
                                        },
                                    ))
                            }),
                    ),
            )
    }

    fn recent(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let s = self.workspace.read(cx).t();
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        v_flex()
            .w_full()
            .child(div().pb_2().font_semibold().child(s.home_recent))
            .when(self.snapshot.recent.is_empty(), |list| {
                list.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(s.empty_library_desc),
                )
            })
            .children(self.snapshot.recent.iter().map(|skill| {
                let id = skill.id.clone();
                div().border_t_1().border_color(theme.border).child(
                    Button::new(format!("home-recent-{id}"))
                        .ghost()
                        .w_full()
                        .min_w_0()
                        .h(px(32.))
                        .accessibility_label(skill.name.clone())
                        .child(
                            Icon::new(IconName::FileText)
                                .size_4()
                                .text_color(muted)
                                .flex_shrink_0(),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_left()
                                .truncate()
                                .child(skill.name.clone()),
                        )
                        .when_some(skill.date.clone(), |row, date| {
                            row.child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(date),
                            )
                        })
                        .child(
                            Icon::new(IconName::ChevronRight)
                                .size_3()
                                .text_color(muted)
                                .flex_shrink_0(),
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.workspace
                                .update(cx, |ws, cx| ws.open_library_skill(id.clone(), cx));
                        })),
                )
            }))
    }

    fn recent_projects(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let mut projects: Vec<_> = ws.projects().iter().collect();
        projects.sort_by_key(|project| Reverse(project.created_at));
        h_flex()
            .w_full()
            .gap_2()
            .flex_wrap()
            .child(div().text_sm().font_semibold().child(s.home_projects))
            .children(projects.into_iter().take(2).map(|project| {
                let id = project.id.clone();
                Button::new(format!("home-project-{id}"))
                    .outline()
                    .small()
                    .max_w(px(188.))
                    .label(project.name.clone())
                    .icon(IconName::FolderOpen)
                    .tooltip(project.path.to_string_lossy().into_owned())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.workspace
                            .update(cx, |ws, cx| ws.open_project(id.clone(), cx));
                    }))
            }))
            .child(
                Button::new("home-add-project")
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .label(s.btn_add_project)
                    .on_click(cx.listener(|view, _, window, cx| {
                        let workspace = view.workspace.downgrade();
                        view.workspace.update(cx, |ws, cx| {
                            ws.pick_and_attach_project(window, cx, move |id, _, cx| {
                                let _ = workspace.update(cx, |ws, cx| ws.open_project(id, cx));
                            });
                        });
                    })),
            )
    }

    fn activity(&self, compact: bool, cx: &Context<Self>) -> impl IntoElement + use<> {
        let s = self.workspace.read(cx).t();
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let accent = theme.primary;
        let maximum = self
            .snapshot
            .intake
            .iter()
            .map(|day| day.count)
            .max()
            .unwrap_or_default();
        let total: u32 = self.snapshot.intake.iter().map(|day| day.count).sum();
        let chart = if maximum > 0 {
            div()
                .w_full()
                .h(px(if compact { 104. } else { 128. }))
                .child(
                    BarChart::new(self.snapshot.intake.clone())
                        .id("home-intake-chart")
                        .band(|day| day.label.clone())
                        .value(|day| f64::from(day.count))
                        .value_axis(true)
                        .value_tick_count((maximum as usize + 1).min(5))
                        .value_tick_format(|value| {
                            if value.fract() == 0. {
                                format!("{value:.0}")
                            } else {
                                String::new()
                            }
                        })
                        .max_band_width(px(24.))
                        .fill(move |_, _, _, _| accent)
                        .name(s.nav_library)
                        .appear(false),
                )
                .into_any_element()
        } else {
            div()
                .w_full()
                .text_sm()
                .text_color(muted)
                .child(s.home_intake_empty)
                .into_any_element()
        };
        v_flex()
            .w_full()
            .gap_2()
            .when(compact, |section| section.gap_1())
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_semibold()
                            .child(s.home_intake),
                    )
                    .when(total > 0, |header| {
                        header.child(
                            div()
                                .flex_shrink_0()
                                .text_sm()
                                .font_semibold()
                                .child(format!("{} {total}", s.home_intake_total)),
                        )
                    }),
            )
            .child(chart)
            .when(maximum > 0, |section| {
                section.child(div().text_xs().text_color(muted).child(s.home_intake_scope))
            })
    }

    fn leaderboard(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let board = self.board;
        let page = ws.market_first_page(board);
        let loading = matches!(page, None | Some(MarketFirstPage::Loading));
        let selected = usize::from(board == MarketBoard::AllTime);
        let ranking: AnyElement = match page {
            Some(MarketFirstPage::Loaded(page)) if !page.items.is_empty() => v_flex()
                .w_full()
                .children(page.items.iter().take(5).enumerate().map(|(index, skill)| {
                    h_flex()
                        .w_full()
                        .min_h(px(48.))
                        .px_2()
                        .gap_3()
                        .border_t_1()
                        .border_color(theme.border)
                        .when(index == 0, |row| row.bg(theme.primary.opacity(0.06)))
                        .child(
                            h_flex()
                                .w(px(24.))
                                .flex_shrink_0()
                                .justify_center()
                                .text_xs()
                                .text_color(muted)
                                .child((index + 1).to_string())
                                .when(index == 0, |rank| {
                                    rank.text_color(theme.foreground).font_semibold()
                                }),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_medium()
                                        .truncate()
                                        .child(skill.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .truncate()
                                        .child(skill.source.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_sm()
                                .font_medium()
                                .child(format_install_count(skill.installs)),
                        )
                        .child(div().w(px(24.)).flex_shrink_0().when_some(
                            skillssh_detail_url(skill),
                            |action, url| {
                                action.child(
                                    Button::new(("home-market-external", index))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::ExternalLink)
                                        .tooltip(s.market_view_details)
                                        .accessibility_label(format!(
                                            "{} · {} · {}",
                                            s.market_view_details, skill.name, skill.source
                                        ))
                                        .on_click(move |_, _, cx| cx.open_url(url.as_str())),
                                )
                            },
                        ))
                }))
                .into_any_element(),
            Some(MarketFirstPage::Loaded(_)) => div()
                .py_4()
                .text_sm()
                .text_color(muted)
                .child(s.home_market_empty)
                .into_any_element(),
            Some(MarketFirstPage::Failed(error)) => {
                Alert::error("home-market-error", error.clone())
                    .banner()
                    .small()
                    .into_any_element()
            }
            _ => h_flex()
                .w_full()
                .gap_2()
                .py_4()
                .text_sm()
                .text_color(muted)
                .child(Spinner::new().small())
                .child(s.market_loading)
                .into_any_element(),
        };
        v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .min_h(px(32.))
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_lg()
                            .font_semibold()
                            .child(s.home_market_rank),
                    )
                    .child(
                        Button::new("home-market-refresh")
                            .ghost()
                            .small()
                            .flex_shrink_0()
                            .icon(IconName::RefreshCw)
                            .loading(loading)
                            .tooltip(s.home_refresh_market)
                            .accessibility_label(s.home_refresh_market)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.workspace.update(cx, |ws, cx| {
                                    ws.load_market_first_page(board, true, window, cx)
                                });
                            })),
                    ),
            )
            .child(
                GroupBox::new()
                    .id("home-market")
                    .outline()
                    .content_style(
                        StyleRefinement::default()
                            .bg(theme.group_box)
                            .rounded(theme.radius_lg)
                            .gap_0()
                            .p_3(),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .pb_3()
                            .justify_between()
                            .child(
                                TabBar::new("home-market-boards")
                                    .segmented()
                                    .selected_index(selected)
                                    .children(HOME_BOARDS.map(|board| Tab::new().label(board.label(s))))
                                    .on_click(cx.listener(|view, index: &usize, window, cx| {
                                        view.board = HOME_BOARDS[*index];
                                        view.workspace.update(cx, |ws, cx| {
                                            ws.load_market_first_page(view.board, false, window, cx)
                                        });
                                        cx.notify();
                                    })),
                            )
                            .when(matches!(page, Some(MarketFirstPage::Loaded(page)) if !page.items.is_empty()), |toolbar| {
                                toolbar.child(div().pr_10().text_xs().text_color(muted).child(s.home_market_installs))
                            }),
                    )
                    .child(ranking)
                    .child(
                        h_flex().justify_end().pt_2().border_t_1().border_color(theme.border).child(
                            Button::new("home-open-market")
                                .ghost()
                                .small()
                                .label(s.home_view_market)
                                .icon(IconName::ArrowRight)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.workspace
                                        .update(cx, |ws, cx| ws.open_market_board(board, cx));
                                })),
                        ),
                    ),
            )
    }
}

impl Render for HomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.workspace.read(cx).t();
        let compact = window.viewport_size().height < px(680.);
        let local = v_flex()
            .w_full()
            .gap_5()
            .when(compact, |column| column.gap_4())
            .child(self.overview(cx))
            .child(self.recent_projects(cx))
            .child(self.recent(cx))
            .child(self.activity(compact, cx));
        let leaderboard = self.leaderboard(cx);
        let sections = if window.viewport_size().width < px(900.) {
            v_flex()
                .gap_5()
                .child(local)
                .child(leaderboard)
                .into_any_element()
        } else {
            h_flex()
                .w_full()
                .items_start()
                .gap_6()
                .child(div().flex_1().min_w_0().child(local))
                .child(div().w_2_5().min_w_0().flex_shrink_0().child(leaderboard))
                .into_any_element()
        };
        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                PageHeader::new(s.nav_home).right_slot(
                    Button::new("home-install")
                        .primary()
                        .icon(IconName::Plus)
                        .label(s.btn_install_from_link)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.workspace.update(cx, |ws, cx| ws.open_install(cx));
                        })),
                ),
            )
            .child(
                div()
                    .id("home-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scrollbar()
                    .child(v_flex().w_full().max_w(px(1380.)).p_5().child(sections)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intake_includes_today_and_six_previous_days() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 3).unwrap();
        let first_day = today - Days::new(6);
        let dates = [
            today + Days::new(1),
            today,
            today,
            first_day,
            today - Days::new(7),
        ];
        let intake = intake_days(today, dates);

        assert_eq!(
            intake.iter().map(|day| day.count).collect::<Vec<_>>(),
            [1, 0, 0, 0, 0, 0, 2]
        );
        assert_eq!(intake[0].label.as_ref(), "12/28");
        assert_eq!(intake[6].label.as_ref(), "01/03");
    }
}
