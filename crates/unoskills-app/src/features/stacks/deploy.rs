use std::path::{Path, PathBuf};

use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, IndexPath, Sizable, StyledExt, WindowExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    h_flex,
    searchable_list::{SearchableListItem, SearchableVec},
    select::{Select, SelectEvent, SelectState},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Task, Window, div,
    prelude::FluentBuilder, px,
};
use unoskills_core::{StackManifest, StackProjectDeployStatus, StackProjectDeploySummary};

use super::StacksView;
use crate::state::{ProjectStackSelection, WorkspaceEvent, WorkspaceState};

#[derive(Clone)]
struct DeploymentTarget {
    id: String,
    label: SharedString,
    path: Option<SharedString>,
}

impl SearchableListItem for DeploymentTarget {
    type Value = String;

    fn title(&self) -> SharedString {
        self.path.as_ref().map_or_else(
            || self.label.clone(),
            |path| format!("{} · {path}", self.label).into(),
        )
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(
            div()
                .min_w_0()
                .truncate()
                .child(self.label.clone())
                .into_any_element(),
        )
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .min_w_0()
            .child(div().truncate().child(self.label.clone()))
            .when_some(self.path.clone(), |row, path| {
                row.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(path),
                )
            })
    }

    fn value(&self) -> &String {
        &self.id
    }
}

type TargetSelect = SelectState<SearchableVec<DeploymentTarget>>;

#[derive(Clone)]
struct ScopeChoice {
    path: PathBuf,
    label: SharedString,
}

impl SearchableListItem for ScopeChoice {
    type Value = PathBuf;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &PathBuf {
        &self.path
    }
}

type ScopeSelect = SelectState<SearchableVec<ScopeChoice>>;

struct ProjectDeployForm {
    workspace: Entity<WorkspaceState>,
    stack: StackManifest,
    project: Entity<TargetSelect>,
    agent: Entity<TargetSelect>,
    scope: Entity<ScopeSelect>,
    selection: Option<ProjectStackSelection>,
    modules: Vec<PathBuf>,
    module_task: Option<Task<()>>,
    module_error: Option<String>,
    running: bool,
    status: Option<Result<StackProjectDeployStatus, String>>,
    status_task: Option<Task<()>>,
    result: Option<Result<StackProjectDeploySummary, String>>,
    _subscriptions: [Subscription; 4],
}

impl StacksView {
    pub(super) fn open_project_deploy(
        &mut self,
        stack: StackManifest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workspace = self.workspace.clone();
        let selection = self
            .project_stack_target
            .as_ref()
            .filter(|selection| selection.stack_id == stack.id);
        let form =
            cx.new(|cx| ProjectDeployForm::new(workspace.clone(), stack, selection, window, cx));
        form.update(cx, |form, cx| form.project_changed(window, cx));
        window.open_dialog(cx, move |dialog, _, cx| {
            let running = form.read(cx).running;
            let content = form.clone();
            dialog
                .title(workspace.read(cx).t().stack_deploy_project)
                .w(px(464.))
                .overlay_closable(false)
                .keyboard(!running)
                .close_button(!running)
                .content(move |dialog_content, _, _| dialog_content.child(content.clone()))
        });
    }
}

impl ProjectDeployForm {
    fn new(
        workspace: Entity<WorkspaceState>,
        stack: StackManifest,
        selection: Option<&ProjectStackSelection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ws = workspace.read(cx);
        let projects = project_targets(ws);
        let project_index = selection
            .and_then(|selection| {
                projects
                    .iter()
                    .position(|project| project.id == selection.project_id)
            })
            .map(|index| IndexPath::default().row(index));
        let agents = agent_targets(ws);
        let root = ws.t().project_root;
        let agent_index = selection
            .and_then(|selection| selection.agent_key.as_deref())
            .and_then(|key| agents.iter().position(|agent| agent.id == key))
            .or_else(|| (agents.len() == 1).then_some(0))
            .map(|index| IndexPath::default().row(index));
        let project = cx.new(|cx| {
            SelectState::new(SearchableVec::new(projects), project_index, window, cx)
                .searchable(true)
        });
        let agent =
            cx.new(|cx| SelectState::new(SearchableVec::new(agents), agent_index, window, cx));
        let scope = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(scope_options(root, &[], None)),
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let subscriptions = [
            cx.subscribe_in::<_, SelectEvent<_>>(
                &project,
                window,
                |form: &mut Self, _, _, window, cx| {
                    form.project_changed(window, cx);
                },
            ),
            cx.subscribe_in::<_, SelectEvent<_>>(
                &agent,
                window,
                |form: &mut Self, _, _, window, cx| {
                    form.target_changed(window, cx);
                },
            ),
            cx.subscribe_in::<_, SelectEvent<_>>(
                &scope,
                window,
                |form: &mut Self, _, _, window, cx| {
                    form.target_changed(window, cx);
                },
            ),
            cx.subscribe_in(
                &workspace,
                window,
                |this, ws, event, window, cx| match event {
                    WorkspaceEvent::Reloaded => {
                        let previous = this.project.read(cx).selected_value().cloned();
                        let projects = project_targets(ws.read(cx));
                        let agents = agent_targets(ws.read(cx));
                        this.project.update(cx, |state, cx| {
                            state.set_items(SearchableVec::new(projects), window, cx);
                        });
                        this.agent.update(cx, |state, cx| {
                            state.set_items(SearchableVec::new(agents), window, cx);
                        });
                        if !this.running {
                            if this.project.read(cx).selected_value() != previous.as_ref() {
                                this.project_changed(window, cx);
                            } else {
                                this.check_status(window, cx);
                            }
                        }
                    }
                    WorkspaceEvent::LanguageChanged => this.update_scopes(window, cx),
                    WorkspaceEvent::InstallStateChanged => this.check_status(window, cx),
                    _ => {}
                },
            ),
        ];
        Self {
            workspace,
            stack,
            project,
            agent,
            scope,
            selection: selection.cloned(),
            modules: Vec::new(),
            module_task: None,
            module_error: None,
            running: false,
            status: None,
            status_task: None,
            result: None,
            _subscriptions: subscriptions,
        }
    }

    fn update_scopes(&self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.project.read(cx).selected_value();
        let suggested = self
            .selection
            .as_ref()
            .filter(|selection| Some(&selection.project_id) == project)
            .map(|selection| selection.module_path.as_path());
        let root = self.workspace.read(cx).t().project_root;
        let options = scope_options(root, &self.modules, suggested);
        self.scope.update(cx, |state, cx| {
            state.set_items(SearchableVec::new(options), window, cx);
        });
        cx.notify();
    }

    fn project_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.modules.clear();
        self.module_task = None;
        self.module_error = None;
        self.update_scopes(window, cx);
        self.scope.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::default()), window, cx);
        });
        self.target_changed(window, cx);
        let Some(project) = self.project.read(cx).selected_value().cloned() else {
            return;
        };
        let manager = self.workspace.read(cx).manager().clone();
        let background = cx.background_executor().clone();
        self.module_task = Some(cx.spawn_in(window, async move |form, cx| {
            let paths = background
                .spawn(async move { manager.project_module_paths(&project) })
                .await;
            let _ = form.update_in(cx, |form, window, cx| {
                match paths {
                    Ok(paths) => form.modules = paths,
                    Err(error) => form.module_error = Some(format!("{error:#}")),
                }
                form.module_task = None;
                form.update_scopes(window, cx);
            });
        }));
    }

    fn target_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.result = None;
        self.check_status(window, cx);
    }

    fn selected_target(&self, cx: &Context<Self>) -> Option<(String, String, PathBuf)> {
        Some((
            self.project.read(cx).selected_value()?.clone(),
            self.agent.read(cx).selected_value()?.clone(),
            self.scope.read(cx).selected_value()?.clone(),
        ))
    }

    fn check_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        self.status_task = None;
        self.status = None;
        cx.notify();
        let Some((project, agent, scope)) = self.selected_target(cx) else {
            return;
        };
        if self.workspace.read(cx).is_stack_installing(&self.stack.id) {
            return;
        }
        let manager = self.workspace.read(cx).manager().clone();
        let stack = self.stack.clone();
        let background = cx.background_executor().clone();
        self.status_task = Some(cx.spawn_in(window, async move |form, cx| {
            let status = background
                .spawn(async move {
                    manager
                        .stack_project_deploy_status_at(&stack, &project, &agent, &scope)
                        .map_err(|error| format!("{error:#}"))
                })
                .await;
            let _ = form.update_in(cx, |form, _, cx| {
                form.status = Some(status);
                cx.notify();
            });
        }));
    }

    fn attach_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let form = cx.weak_entity();
        self.workspace.update(cx, |ws, cx| {
            ws.pick_and_attach_project(window, cx, move |id, window, cx| {
                let _ = form.update(cx, |form, cx| {
                    let projects = project_targets(form.workspace.read(cx));
                    form.project.update(cx, |state, cx| {
                        state.set_items(SearchableVec::new(projects), window, cx);
                        state.set_selected_value(&id, window, cx);
                    });
                    form.project_changed(window, cx);
                });
            });
        });
    }

    fn deploy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, agent, scope)) = self.selected_target(cx) else {
            return;
        };
        if self.running
            || !self
                .status
                .as_ref()
                .is_some_and(|status| status.as_ref().is_ok_and(|status| status.pending > 0))
        {
            return;
        }
        self.running = true;
        self.result = None;
        let token = self
            .workspace
            .update(cx, |ws, cx| ws.begin_busy(ws.t().stack_deploy_busy, cx));
        let manager = self.workspace.read(cx).manager().clone();
        let workspace = self.workspace.downgrade();
        let stack = self.stack.clone();
        let background = cx.background_executor().clone();
        cx.notify();
        cx.spawn_in(window, async move |form, cx| {
            let result = background
                .spawn(async move {
                    manager
                        .deploy_stack_to_project_at(&stack, &project, &agent, &scope)
                        .map_err(|error| format!("{error:#}"))
                })
                .await;
            let _ = workspace.update_in(cx, |ws, _, cx| {
                ws.reload_all(cx);
                ws.end_busy(token, cx);
            });
            let _ = form.update_in(cx, |form, window, cx| {
                form.running = false;
                form.result = Some(result);
                form.check_status(window, cx);
            });
        })
        .detach();
    }
}

impl Render for ProjectDeployForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let lang = ws.lang();
        let s = lang.t();
        let project_id = self.project.read(cx).selected_value();
        let agent_key = self.agent.read(cx).selected_value();
        let project = ws
            .projects()
            .iter()
            .find(|project| Some(&project.id) == project_id);
        let scope = self.scope.read(cx).selected_value();
        let selection = self
            .selection
            .as_ref()
            .filter(|selection| Some(&selection.project_id) == project_id);
        let has_subproject = self.modules.iter().any(|path| !path.as_os_str().is_empty())
            || selection.is_some_and(|selection| !selection.module_path.as_os_str().is_empty());
        let directory = project
            .zip(scope)
            .map(|(project, scope)| project.path.join(scope).display().to_string());
        let installing = ws.is_stack_installing(&self.stack.id);
        let status = self.status.as_ref().and_then(|status| status.as_ref().ok());
        let pending = status.map_or(0, |status| status.pending);
        let unavailable = status.map_or(0, |status| status.unavailable);
        let conflicts = status.map_or(0, |status| status.conflicts);
        let status_error = self
            .status
            .as_ref()
            .and_then(|status| status.as_ref().err());
        let has_agents = ws
            .agents()
            .iter()
            .any(|agent| agent.installed && agent.enabled);
        let muted = cx.theme().muted_foreground;
        let has_target = project.is_some() && agent_key.is_some() && scope.is_some();
        let checking = has_target && self.status.is_none() && !installing;
        let progress = match status {
            Some(status) => {
                lang.stack_project_progress(status.deployed, self.stack.skills.len(), pending)
            }
            None if installing => s.stack_project_installing.to_owned(),
            None if checking => s.stack_project_checking.to_owned(),
            None => String::new(),
        };
        let result = self.result.as_ref().map(|result| match result {
            Ok(summary) => {
                let message = lang.stack_project_result(summary);
                if let Some((name, error)) = summary.failed.first() {
                    let reason = if error.contains("TARGET_CONFLICT") {
                        s.stack_project_target_conflict
                    } else {
                        error.as_str()
                    };
                    Alert::error(
                        "project-deploy-result",
                        format!("{message}\n{name} · {reason}"),
                    )
                } else {
                    Alert::success("project-deploy-result", message)
                }
            }
            Err(error) => Alert::error("project-deploy-result", error.clone()),
        });

        v_flex()
            .w_full()
            .min_w_0()
            .gap_3()
            .child(
                div()
                    .min_h(px(20.))
                    .flex_shrink_0()
                    .font_semibold()
                    .truncate()
                    .child(self.stack.name.clone()),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div().flex_1().min_w_0().child(
                            Select::new(&self.project)
                                .placeholder(s.stack_pick_project)
                                .accessibility_label(s.stack_pick_project)
                                .disabled(self.running || ws.projects().is_empty()),
                        ),
                    )
                    .child(
                        Button::new("stack-attach-project")
                            .outline()
                            .flex_shrink_0()
                            .icon(IconName::FolderOpen)
                            .label(s.btn_add_project)
                            .disabled(self.running)
                            .on_click(cx.listener(|form, _, window, cx| {
                                form.attach_project(window, cx);
                            })),
                    ),
            )
            .when_some(selection, |form, selection| {
                let path = if selection.module_path.as_os_str().is_empty() {
                    s.project_root.to_owned()
                } else {
                    selection.module_path.display().to_string()
                };
                form.child(
                    h_flex()
                        .gap_2()
                        .text_xs()
                        .text_color(muted)
                        .child(div().flex_shrink_0().child(s.project_module))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(gpui_kit::text!(path)),
                        ),
                )
            })
            .when_some(directory, |form, directory| {
                form.child(
                    v_flex()
                        .gap_1()
                        .child(
                            h_flex()
                                .gap_2()
                                .justify_between()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_medium()
                                        .child(s.stack_project_directory),
                                )
                                .when(has_subproject, |row| {
                                    row.child(
                                        div().w(px(200.)).max_w_full().min_w_0().child(
                                            Select::new(&self.scope)
                                                .small()
                                                .accessibility_label(s.stack_project_directory)
                                                .disabled(self.running)
                                                .menu_max_h(px(240.)),
                                        ),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .id("stack-project-directory-path")
                                .text_xs()
                                .text_color(muted)
                                .truncate()
                                .tooltip({
                                    let directory = directory.clone();
                                    move |window, cx| {
                                        Tooltip::new(directory.clone()).build(window, cx)
                                    }
                                })
                                .child(gpui_kit::text!(directory)),
                        ),
                )
            })
            .when_some(self.module_error.as_ref(), |form, error| {
                form.child(Alert::warning("stack-project-module-error", error.clone()).banner())
            })
            .child(
                Select::new(&self.agent)
                    .placeholder(s.stack_pick_agent)
                    .accessibility_label(s.stack_pick_agent)
                    .disabled(self.running || !has_agents),
            )
            .when(!has_agents, |form| {
                form.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(s.stack_no_deploy_agents),
                )
            })
            .when(has_target || installing, |form| {
                form.child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .font_medium()
                                .child(gpui_kit::text!(progress)),
                        )
                        .child(
                            Button::new("refresh-project-deploy-status")
                                .ghost()
                                .flex_shrink_0()
                                .icon(IconName::RefreshCw)
                                .tooltip(s.stack_project_refresh)
                                .accessibility_label(s.stack_project_refresh)
                                .loading(checking || installing)
                                .disabled(self.running || checking || installing)
                                .on_click(cx.listener(|form, _, window, cx| {
                                    form.result = None;
                                    form.check_status(window, cx);
                                })),
                        ),
                )
            })
            .when_some(status_error, |form, error| {
                form.child(Alert::error("stack-project-check-error", error.clone()).banner())
            })
            .when(unavailable > 0, |form| {
                form.child(
                    Alert::warning(
                        "stack-project-unavailable",
                        lang.stack_project_unavailable(unavailable),
                    )
                    .banner(),
                )
            })
            .when(conflicts > 0, |form| {
                form.child(
                    Alert::warning(
                        "stack-project-conflicts",
                        lang.stack_project_conflicts(conflicts),
                    )
                    .banner(),
                )
            })
            .when_some(result, |form, result| form.child(result.banner()))
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("close-stack-project-deploy")
                            .outline()
                            .label(if self.result.is_some() {
                                s.btn_close
                            } else {
                                s.btn_cancel
                            })
                            .disabled(self.running)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("apply-stack-to-project")
                            .primary()
                            .label(s.stack_deploy_apply)
                            .loading(self.running)
                            .disabled(!has_target || installing || pending == 0)
                            .on_click(cx.listener(|form, _, window, cx| {
                                form.deploy(window, cx);
                            })),
                    ),
            )
    }
}

fn scope_options(root: &str, modules: &[PathBuf], scanned: Option<&Path>) -> Vec<ScopeChoice> {
    let mut paths = modules.to_vec();
    paths.extend(scanned.map(Path::to_path_buf));
    paths.push(PathBuf::new());
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .map(|path| {
            let label = if path.as_os_str().is_empty() {
                root.into()
            } else {
                path.display().to_string().into()
            };
            ScopeChoice { path, label }
        })
        .collect()
}

fn project_targets(workspace: &WorkspaceState) -> Vec<DeploymentTarget> {
    workspace
        .projects()
        .iter()
        .map(|project| DeploymentTarget {
            id: project.id.clone(),
            label: project.name.clone().into(),
            path: Some(project.path.to_string_lossy().into_owned().into()),
        })
        .collect()
}

fn agent_targets(workspace: &WorkspaceState) -> Vec<DeploymentTarget> {
    workspace
        .agents()
        .iter()
        .filter(|agent| agent.installed && agent.enabled)
        .map(|agent| DeploymentTarget {
            id: agent.key.clone(),
            label: agent.display_name.clone().into(),
            path: None,
        })
        .collect()
}
