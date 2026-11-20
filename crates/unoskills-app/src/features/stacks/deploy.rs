use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, IndexPath, StyledExt, WindowExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    h_flex,
    searchable_list::{SearchableListItem, SearchableVec},
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};
use unoskills_core::{StackManifest, StackProjectDeploySummary};

use super::StacksView;
use crate::state::{WorkspaceEvent, WorkspaceState};

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

struct ProjectDeployForm {
    workspace: Entity<WorkspaceState>,
    stack: StackManifest,
    project: Entity<TargetSelect>,
    agent: Entity<TargetSelect>,
    running: bool,
    result: Option<Result<StackProjectDeploySummary, String>>,
    _subscriptions: [Subscription; 3],
}

impl StacksView {
    pub(super) fn open_project_deploy(
        &mut self,
        stack: StackManifest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workspace = self.workspace.clone();
        let form = cx.new(|cx| ProjectDeployForm::new(workspace.clone(), stack, window, cx));
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ws = workspace.read(cx);
        let projects = project_targets(ws);
        let agents = agent_targets(ws);
        let agent_index = (agents.len() == 1).then(IndexPath::default);
        let project = cx.new(|cx| {
            SelectState::new(SearchableVec::new(projects), None, window, cx).searchable(true)
        });
        let agent =
            cx.new(|cx| SelectState::new(SearchableVec::new(agents), agent_index, window, cx));
        let subscriptions = [
            cx.subscribe(&project, Self::target_changed),
            cx.subscribe(&agent, Self::target_changed),
            cx.subscribe_in(
                &workspace,
                window,
                |this, ws, event, window, cx| match event {
                    WorkspaceEvent::Reloaded => {
                        let projects = project_targets(ws.read(cx));
                        let agents = agent_targets(ws.read(cx));
                        this.project.update(cx, |state, cx| {
                            state.set_items(SearchableVec::new(projects), window, cx);
                        });
                        this.agent.update(cx, |state, cx| {
                            state.set_items(SearchableVec::new(agents), window, cx);
                        });
                        cx.notify();
                    }
                    WorkspaceEvent::LanguageChanged => cx.notify(),
                    _ => {}
                },
            ),
        ];
        Self {
            workspace,
            stack,
            project,
            agent,
            running: false,
            result: None,
            _subscriptions: subscriptions,
        }
    }

    fn target_changed(
        &mut self,
        _: Entity<TargetSelect>,
        _: &SelectEvent<SearchableVec<DeploymentTarget>>,
        cx: &mut Context<Self>,
    ) {
        self.result = None;
        cx.notify();
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
                    form.result = None;
                    cx.notify();
                });
            });
        });
    }

    fn deploy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(project), Some(agent)) = (
            self.project.read(cx).selected_value().cloned(),
            self.agent.read(cx).selected_value().cloned(),
        ) else {
            return;
        };
        if self.running {
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
                        .deploy_stack_to_project(&stack, &project, &agent)
                        .map_err(|error| format!("{error:#}"))
                })
                .await;
            let _ = workspace.update_in(cx, |ws, _, cx| {
                ws.reload_all(cx);
                ws.end_busy(token, cx);
            });
            let _ = form.update_in(cx, |form, _, cx| {
                form.running = false;
                form.result = Some(result);
                cx.notify();
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
        let mut deployed = 0;
        let mut pending = 0;
        let mut unavailable = 0;
        for item in &self.stack.skills {
            match item.deployable_in(ws.skills()) {
                Some(skill) => match (project_id, agent_key) {
                    (Some(project), Some(agent))
                        if skill.is_deployed_to_project(project, agent) =>
                    {
                        deployed += 1;
                    }
                    _ => pending += 1,
                },
                None => unavailable += 1,
            }
        }
        let has_agents = ws
            .agents()
            .iter()
            .any(|agent| agent.installed && agent.enabled);
        let muted = cx.theme().muted_foreground;
        let progress = project.is_some() && agent_key.is_some();
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
            .when_some(project, |form, project| {
                form.child(
                    div()
                        .min_h(px(16.))
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(muted)
                        .truncate()
                        .child(project.path.to_string_lossy().into_owned()),
                )
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
            .when(progress, |form| {
                form.child(
                    div()
                        .text_sm()
                        .font_medium()
                        .child(lang.stack_project_progress(
                            deployed,
                            self.stack.skills.len(),
                            pending,
                        )),
                )
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
                            .disabled(!progress || pending == 0)
                            .on_click(cx.listener(|form, _, window, cx| {
                                form.deploy(window, cx);
                            })),
                    ),
            )
    }
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
