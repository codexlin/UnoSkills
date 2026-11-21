use std::collections::HashSet;
use std::ops::Range;
use std::path::PathBuf;

use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, IndexPath, Sizable, StyledExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    scroll::ScrollableElement,
    searchable_list::{SearchableListItem, SearchableVec},
    select::{Select, SelectEvent, SelectState},
    spinner::Spinner,
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Subscription, Task,
    UniformListScrollHandle, WeakEntity, Window, div, prelude::FluentBuilder, px, uniform_list,
};
use unoskills_core::{
    ProjectWorkspace,
    project_analysis::{ProjectModule, ProjectSkill, SkillLocation},
};

use super::LibraryView;
use crate::components::page_header::PageHeader;
use crate::state::{ProjectStackSelection, WorkspaceEvent, WorkspaceState};

#[derive(Clone)]
struct ProjectChoice<T> {
    value: T,
    label: SharedString,
}

impl<T: Clone + PartialEq> SearchableListItem for ProjectChoice<T> {
    type Value = T;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &T {
        &self.value
    }
}

type ProjectSelect<T> = SelectState<SearchableVec<ProjectChoice<T>>>;

pub(super) struct ProjectOverview {
    workspace: Entity<WorkspaceState>,
    library: WeakEntity<LibraryView>,
    project: ProjectWorkspace,
    modules: Option<Result<Vec<ProjectModule>, String>>,
    scope_select: Entity<ProjectSelect<PathBuf>>,
    agent_select: Entity<ProjectSelect<String>>,
    module_path: PathBuf,
    selected: HashSet<String>,
    agent: Option<String>,
    inventory: Option<Result<Vec<ProjectSkill>, String>>,
    scroll: UniformListScrollHandle,
    _scan: Option<Task<()>>,
    _inventory_scan: Option<Task<()>>,
    saving: bool,
    _subscriptions: [Subscription; 3],
}

impl LibraryView {
    pub(super) fn show_project(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let ws = self.workspace.read(cx);
        let Some(project) = ws
            .projects()
            .iter()
            .find(|project| project.id == id)
            .cloned()
        else {
            return;
        };
        self.set_scope(Some(id.to_owned()), cx);
        let library = cx.weak_entity();
        let overview =
            cx.new(|cx| ProjectOverview::new(self.workspace.clone(), library, project, window, cx));
        overview.update(cx, |overview, cx| overview.refresh(window, cx));
        self.project_overview = Some(overview);
        cx.notify();
    }
}

fn agent_options(workspace: &WorkspaceState) -> Vec<ProjectChoice<String>> {
    workspace
        .agents()
        .iter()
        .filter(|agent| agent.installed && agent.enabled)
        .map(|agent| ProjectChoice {
            value: agent.key.clone(),
            label: agent.display_name.clone().into(),
        })
        .collect()
}

impl ProjectOverview {
    fn new(
        workspace: Entity<WorkspaceState>,
        library: WeakEntity<LibraryView>,
        project: ProjectWorkspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let agents = agent_options(workspace.read(cx));
        let agent = agents.first().map(|choice| choice.value.clone());
        let initial = agent.as_ref().map(|_| IndexPath::default());
        let scope_select: Entity<ProjectSelect<PathBuf>> = cx.new(|cx| {
            SelectState::new(SearchableVec::new(Vec::new()), None, window, cx).searchable(true)
        });
        let agent_select =
            cx.new(|cx| SelectState::new(SearchableVec::new(agents), initial, window, cx));
        let subscriptions = [
            cx.subscribe_in(
                &scope_select,
                window,
                |view: &mut Self,
                 _,
                 event: &SelectEvent<SearchableVec<ProjectChoice<PathBuf>>>,
                 window,
                 cx| {
                    if let SelectEvent::Confirm(Some(path)) = event {
                        view.choose_module(path.clone(), cx);
                        view.scan_inventory(window, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &agent_select,
                window,
                |view: &mut Self, _, event: &SelectEvent<_>, window, cx| {
                    let SelectEvent::Confirm(agent) = event;
                    view.agent = agent.clone();
                    view.scan_inventory(window, cx);
                },
            ),
            cx.subscribe_in(
                &workspace,
                window,
                |view: &mut Self, ws, event, window, cx| match event {
                    WorkspaceEvent::Reloaded => {
                        let agents = agent_options(ws.read(cx));
                        let first_agent = agents.first().map(|item| item.value.clone());
                        view.agent = view.agent_select.update(cx, |state, cx| {
                            state.set_items(SearchableVec::new(agents), window, cx);
                            if let (None, Some(first_agent)) = (state.selected_value(), first_agent)
                            {
                                state.set_selected_value(&first_agent, window, cx);
                            }
                            state.selected_value().cloned()
                        });
                        view.scan_inventory(window, cx);
                    }
                    WorkspaceEvent::LanguageChanged => {
                        view.update_scope_options(window, cx);
                        cx.notify();
                    }
                    _ => {}
                },
            ),
        ];
        Self {
            workspace,
            library,
            project,
            modules: None,
            scope_select,
            agent_select,
            module_path: PathBuf::new(),
            selected: HashSet::new(),
            agent,
            inventory: None,
            scroll: UniformListScrollHandle::new(),
            _scan: None,
            _inventory_scan: None,
            saving: false,
            _subscriptions: subscriptions,
        }
    }

    fn update_scope_options(&self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.workspace.read(cx).t();
        let options: Vec<_> = self
            .modules
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .into_iter()
            .flatten()
            .map(|module| ProjectChoice {
                value: module.path.clone(),
                label: if module.path.as_os_str().is_empty() {
                    s.project_root.into()
                } else {
                    module.path.display().to_string().into()
                },
            })
            .collect();
        self.scope_select.update(cx, |state, cx| {
            state.set_items(SearchableVec::new(options), window, cx);
            state.set_selected_value(&self.module_path, window, cx);
        });
    }
    fn module(&self) -> Option<&ProjectModule> {
        self.modules
            .as_ref()?
            .as_ref()
            .ok()?
            .iter()
            .find(|module| module.path == self.module_path)
    }

    fn choose_module(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.module_path = path;
        self.selected = self
            .module()
            .into_iter()
            .flat_map(|module| &module.recommendations)
            .map(|item| item.skill.source.clone())
            .collect();
        cx.notify();
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let retain_selection = self.module().is_some();
        let active_module = self.module_path.clone();
        self.modules = None;
        let manager = self.workspace.read(cx).manager().clone();
        let id = self.project.id.clone();
        let background = cx.background_executor().clone();
        self._scan = Some(cx.spawn_in(window, async move |this, cx| {
            let result = background
                .spawn(async move {
                    manager
                        .analyze_project(&id)
                        .map_err(|error| format!("{error:#}"))
                })
                .await;
            let _ = this.update_in(cx, |view, window, cx| {
                let next_path = result
                    .as_ref()
                    .ok()
                    .and_then(|modules| {
                        modules
                            .iter()
                            .find(|module| module.path == view.module_path)
                            .or_else(|| modules.first())
                    })
                    .map(|module| module.path.clone());
                view.modules = Some(result);
                if let Some(path) = next_path {
                    if retain_selection && path == active_module {
                        let available: HashSet<_> = view
                            .module()
                            .into_iter()
                            .flat_map(|module| &module.recommendations)
                            .map(|item| item.skill.source.clone())
                            .collect();
                        view.selected.retain(|source| available.contains(source));
                        cx.notify();
                    } else {
                        view.choose_module(path, cx);
                    }
                } else {
                    view.selected.clear();
                    cx.notify();
                }
                view.update_scope_options(window, cx);
                view.scan_inventory(window, cx);
            });
        }));
    }

    fn scan_inventory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._inventory_scan = None;
        self.inventory = None;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
        let Some(agent) = self.agent.clone() else {
            return;
        };
        let manager = self.workspace.read(cx).manager().clone();
        let id = self.project.id.clone();
        let scope = self.module_path.clone();
        let background = cx.background_executor().clone();
        self._inventory_scan = Some(cx.spawn_in(window, async move |this, cx| {
            let result = background
                .spawn(async move {
                    manager
                        .inspect_project_skills_at(&id, &agent, &scope)
                        .map_err(|error| format!("{error:#}"))
                })
                .await;
            let _ = this.update_in(cx, |view, _, cx| {
                view.inventory = Some(result);
                cx.notify();
            });
        }));
    }

    fn save_stack(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let skills: Vec<_> = self
            .module()
            .into_iter()
            .flat_map(|module| &module.recommendations)
            .filter(|item| self.selected.contains(&item.skill.source))
            .map(|item| item.skill.clone())
            .collect();
        if skills.is_empty() || self.saving {
            return;
        }
        let name = if self.module_path.as_os_str().is_empty() {
            self.project.name.clone()
        } else {
            format!("{} · {}", self.project.name, self.module_path.display())
        };
        let manager = self.workspace.read(cx).manager().clone();
        let id = self.project.id.clone();
        let project_id = id.clone();
        let module_path = self.module_path.clone();
        let agent = self.agent.clone();
        let background = cx.background_executor().clone();
        self.saving = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = background
                .spawn(async move { manager.save_project_skill_stack(&id, name, skills) })
                .await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.saving = false;
                view.workspace.update(cx, |ws, cx| match result {
                    Ok(stack) => {
                        ws.reload_all(cx);
                        let message = ws
                            .lang()
                            .toast_stack_created(&stack.name, stack.skills.len());
                        ws.notify_toast(false, message, window, cx);
                        ws.open_project_stack(
                            ProjectStackSelection {
                                stack_id: stack.id,
                                project_id,
                                module_path,
                                agent_key: agent,
                            },
                            cx,
                        );
                    }
                    Err(error) => ws.notify_toast(true, format!("{error:#}"), window, cx),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn scan_panel(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let ws = self.workspace.read(cx);
        let lang = ws.lang();
        let s = ws.t();
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let modules = self
            .modules
            .as_ref()
            .and_then(|result| result.as_ref().ok());
        let inventory = self
            .inventory
            .as_ref()
            .and_then(|value| value.as_ref().ok());
        v_flex()
            .w_full()
            .min_w_0()
            .gap_3()
            .child(
                v_flex()
                    .gap_2()
                    .text_sm()
                    .when(self.module().is_some(), |details| {
                        details.child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_color(muted)
                                        .child(gpui_kit::text!(s.project_module)),
                                )
                                .when_else(
                                    modules.is_some_and(|modules| modules.len() > 1),
                                    |row| {
                                        row.child(
                                            div().w(px(176.)).min_w_0().child(
                                                Select::new(&self.scope_select)
                                                    .small()
                                                    .accessibility_label(s.project_module)
                                                    .menu_max_h(px(280.)),
                                            ),
                                        )
                                    },
                                    |row| {
                                        row.child(div().flex_1().min_w_0().truncate().child(
                                            if self.module_path.as_os_str().is_empty() {
                                                s.project_root.to_owned()
                                            } else {
                                                self.module_path.display().to_string()
                                            },
                                        ))
                                    },
                                ),
                        )
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_color(muted)
                                    .child(gpui_kit::text!(s.project_default_deploy_directory)),
                            )
                            .child(
                                div().flex_1().min_w_0().child(gpui_kit::text!(
                                    self.project.path.display().to_string()
                                )),
                            ),
                    ),
            )
            .child(
                div()
                    .text_base()
                    .font_semibold()
                    .child(s.project_technologies),
            )
            .when(self.modules.is_none(), |pane| {
                pane.child(Spinner::new().small())
            })
            .when_some(
                self.modules
                    .as_ref()
                    .and_then(|result| result.as_ref().err()),
                |pane, error| {
                    pane.child(Alert::error("project-scan-error", error.clone()).banner())
                },
            )
            .when(modules.is_some_and(Vec::is_empty), |pane| {
                pane.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(s.project_no_technologies),
                )
            })
            .when_some(self.module(), |pane, module| {
                pane.child(v_flex().gap_1().children(module.evidence.iter().map(
                    |(technology, path)| {
                        h_flex()
                            .min_w_0()
                            .gap_2()
                            .text_sm()
                            .child(
                                div()
                                    .font_medium()
                                    .flex_shrink_0()
                                    .child(technology.label()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(muted)
                                    .child(path.display().to_string()),
                            )
                            .when(module.added.contains(technology), |row| {
                                row.child(
                                    div()
                                        .flex_shrink_0()
                                        .text_xs()
                                        .text_color(cx.theme().primary)
                                        .child(s.project_new_technology),
                                )
                            })
                    },
                )))
                .child(
                    div()
                        .pt_3()
                        .border_t_1()
                        .border_color(border)
                        .font_semibold()
                        .child(s.project_recommendations),
                )
                .when(module.recommendations.is_empty(), |pane| {
                    pane.child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(s.project_no_recommendations),
                    )
                })
                .children(
                    module
                        .recommendations
                        .iter()
                        .enumerate()
                        .map(|(index, item)| {
                            let source = item.skill.source.clone();
                            let is_deployed = inventory.into_iter().flatten().any(|skill| {
                                skill.location == SkillLocation::ProjectDeployment
                                    && item.installed_slug.as_deref() == Some(skill.name.as_str())
                            });
                            let status = if is_deployed {
                                s.library_deployed
                            } else if item.installed_slug.is_some() {
                                s.already_in_library
                            } else {
                                s.project_install_required
                            };
                            v_flex()
                                .w_full()
                                .gap_1()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .border_1()
                                .border_color(border)
                                .child(
                                    h_flex()
                                        .w_full()
                                        .gap_2()
                                        .child(
                                            div().flex_1().min_w_0().child(
                                                Checkbox::new(("project-recommendation", index))
                                                    .label(item.skill.name.clone())
                                                    .checked(self.selected.contains(&source))
                                                    .on_click(cx.listener(
                                                        move |view, checked: &bool, _, cx| {
                                                            if *checked {
                                                                view.selected
                                                                    .insert(source.clone());
                                                            } else {
                                                                view.selected.remove(&source);
                                                            }
                                                            cx.notify();
                                                        },
                                                    )),
                                            ),
                                        )
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(status),
                                        ),
                                )
                                .child(div().text_xs().text_color(muted).child(
                                    lang.project_recommendation_reason(
                                        item.technology,
                                        item.purpose,
                                    ),
                                ))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .truncate()
                                        .child(item.skill.source.clone()),
                                )
                        }),
                )
            })
    }

    fn inventory_panel(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let border = theme.border;
        let count = self
            .inventory
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map_or(0, Vec::len);
        v_flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .bg(theme.group_box)
            .child(
                h_flex()
                    .p_3()
                    .gap_2()
                    .flex_wrap()
                    .justify_between()
                    .border_b_1()
                    .border_color(border)
                    .child(div().font_semibold().child(s.project_inventory))
                    .child(
                        div().w(px(168.)).max_w_full().child(
                            Select::new(&self.agent_select)
                                .small()
                                .placeholder(s.stack_pick_agent)
                                .accessibility_label(s.stack_pick_agent)
                                .disabled(self.agent.is_none()),
                        ),
                    ),
            )
            .when(self.agent.is_none(), |pane| {
                pane.child(
                    div()
                        .p_3()
                        .text_sm()
                        .text_color(muted)
                        .child(s.stack_no_deploy_agents),
                )
            })
            .when(self.agent.is_some() && self.inventory.is_none(), |pane| {
                pane.child(div().p_3().child(Spinner::new().small()))
            })
            .when_some(
                self.inventory
                    .as_ref()
                    .and_then(|result| result.as_ref().err()),
                |pane, error| {
                    pane.child(
                        div()
                            .p_3()
                            .child(Alert::error("project-inventory-error", error.clone()).banner()),
                    )
                },
            )
            .when(
                self.inventory
                    .as_ref()
                    .is_some_and(|result| result.as_ref().is_ok_and(Vec::is_empty)),
                |pane| {
                    pane.child(
                        div()
                            .p_3()
                            .text_sm()
                            .text_color(muted)
                            .child(s.project_inventory_empty),
                    )
                },
            )
            .when(count > 0, |pane| {
                pane.child(
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .child(
                            uniform_list(
                                "project-inventory",
                                count,
                                cx.processor(move |view, range: Range<usize>, _, _| {
                                    let Some(Ok(inventory)) = &view.inventory else {
                                        return Vec::new();
                                    };
                                    range
                                        .filter_map(|index| {
                                            inventory.get(index).map(|skill| (index, skill))
                                        })
                                        .map(|(index, skill)| {
                                            let path = skill.path.display().to_string();
                                            let label = match skill.location {
                                                SkillLocation::ProjectDeployment => {
                                                    s.project_managed_skill
                                                }
                                                SkillLocation::ProjectChanged => {
                                                    s.project_changed_skill
                                                }
                                                SkillLocation::ProjectLocal => {
                                                    s.project_local_skill
                                                }
                                                SkillLocation::Global => s.scope_global,
                                            };
                                            v_flex()
                                                .w_full()
                                                .min_w_0()
                                                .h(px(48.))
                                                .px_3()
                                                .justify_center()
                                                .border_b_1()
                                                .border_color(border)
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .truncate()
                                                                .text_sm()
                                                                .font_medium()
                                                                .child(skill.name.clone()),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_shrink_0()
                                                                .text_xs()
                                                                .text_color(muted)
                                                                .child(label),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .id(("project-skill-path", index))
                                                        .text_xs()
                                                        .text_color(muted)
                                                        .truncate()
                                                        .tooltip({
                                                            let path = path.clone();
                                                            move |window, cx| {
                                                                Tooltip::new(path.clone())
                                                                    .build(window, cx)
                                                            }
                                                        })
                                                        .child(path),
                                                )
                                        })
                                        .collect()
                                }),
                            )
                            .size_full()
                            .track_scroll(&self.scroll),
                        )
                        .vertical_scrollbar(&self.scroll),
                )
            })
    }
}

impl Render for ProjectOverview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.workspace.read(cx).t();
        let border = cx.theme().border;
        let library = self.library.clone();
        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                PageHeader::new(self.project.name.clone()).right_slot(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("project-return-library")
                                .ghost()
                                .icon(IconName::ArrowLeft)
                                .label(s.nav_library)
                                .on_click(move |_, _, cx| {
                                    let _ = library.update(cx, |library, cx| {
                                        library.project_overview = None;
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("project-rescan")
                                .outline()
                                .icon(IconName::RefreshCw)
                                .label(s.project_scan)
                                .loading(self.modules.is_none())
                                .on_click(
                                    cx.listener(|view, _, window, cx| view.refresh(window, cx)),
                                ),
                        ),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .overflow_hidden()
                    .child(
                        div()
                            .id("project-recommendation-scroll")
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .child(div().p_4().child(self.scan_panel(cx))),
                    )
                    .child(
                        div()
                            .w_2_5()
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .border_l_1()
                            .border_color(border)
                            .child(self.inventory_panel(cx)),
                    ),
            )
            .child(
                h_flex()
                    .min_h(px(56.))
                    .px_5()
                    .gap_2()
                    .border_t_1()
                    .border_color(border)
                    .child(
                        div().flex_1().min_w_0().text_sm().child(
                            self.workspace
                                .read(cx)
                                .lang()
                                .project_selection_count(self.selected.len()),
                        ),
                    )
                    .child(
                        Button::new("project-save-recommendations")
                            .primary()
                            .flex_shrink_0()
                            .icon(IconName::Plus)
                            .label(s.project_save_stack)
                            .loading(self.saving)
                            .disabled(self.selected.is_empty() || self.modules.is_none())
                            .on_click(
                                cx.listener(|view, _, window, cx| view.save_stack(window, cx)),
                            ),
                    ),
            )
    }
}
