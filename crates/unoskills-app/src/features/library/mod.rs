mod inspector;
mod list;
mod project;

use gpui_kit::component::{
    IconName, WindowExt,
    button::{Button, ButtonVariant},
    h_flex, h_resizable,
    input::{InputEvent, InputState, TextareaState},
    resizable_panel,
    text::TextViewState,
    v_flex,
};
use gpui_kit::{
    AppContext, Context, Entity, Focusable, IntoElement, ParentElement, Render, Styled,
    Subscription, UniformListScrollHandle, Window, div, prelude::FluentBuilder, px,
};
use unoskills_core::SkillLintReport;

use crate::components::page_header::PageHeader;
use crate::state::{WorkspaceEvent, WorkspaceState};

/// Library page: virtualized skill list with live search filtering, plus a
/// SKILL.md inspector (preview / quick edit) and per-agent deploy switches.
pub struct LibraryView {
    pub(super) workspace: Entity<WorkspaceState>,
    pub(super) library_search: Entity<InputState>,
    /// Indices into `WorkspaceState::skills()` matching the current search
    /// query — recomputed on `InputEvent::Change` / `WorkspaceEvent::Reloaded`
    /// so `render()` never clones the skill list.
    pub(super) filtered_indices: Vec<usize>,
    pub(super) library_scroll_handle: UniformListScrollHandle,
    pub(super) selected_skill_id: Option<String>,
    /// `None` = Global scope (`~/.claude/skills`, etc.), `Some(id)` = attached Project scope.
    pub(super) selected_project_id: Option<String>,
    project_overview: Option<Entity<project::ProjectOverview>>,
    pub(super) markdown_preview_state: Entity<TextViewState>,
    pub(super) markdown_editor: Entity<TextareaState>,
    pub(super) editing_markdown: bool,
    pub(super) preview_tab: bool,
    pub(super) agent_filter: Option<String>,
    loaded_markdown: String,
    pub(super) selected_lint: Option<SkillLintReport>,
    _subscriptions: Subscription,
}

impl LibraryView {
    pub fn new(
        workspace: Entity<WorkspaceState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let s = workspace.read(cx).t();
        let library_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_search_skills));
        let markdown_preview_state = cx.new(|cx| TextViewState::markdown("", cx));
        let markdown_editor =
            cx.new(|cx| TextareaState::new(window, cx).placeholder(s.ph_skill_md));

        let workspace_sub = cx.subscribe_in(
            &workspace,
            window,
            |this, _ws, event: &WorkspaceEvent, window, cx| match event {
                WorkspaceEvent::Reloaded => this.on_data_reloaded(window, cx),
                WorkspaceEvent::SettingsChanged => cx.notify(),
                WorkspaceEvent::SelectLibrarySkill(id) => {
                    this.project_overview = None;
                    this.select_skill(id.clone(), window, cx);
                }
                WorkspaceEvent::OpenProject(id) => this.show_project(id, window, cx),
                WorkspaceEvent::LanguageChanged => this.apply_language(window, cx),
                _ => {}
            },
        );
        let search_sub = cx.subscribe_in(
            &library_search,
            window,
            |this, _input, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.refilter(cx);
                    cx.notify();
                }
            },
        );

        let mut view = Self {
            workspace,
            library_search,
            filtered_indices: Vec::new(),
            library_scroll_handle: UniformListScrollHandle::new(),
            selected_skill_id: None,
            selected_project_id: None,
            project_overview: None,
            markdown_preview_state,
            markdown_editor,
            editing_markdown: false,
            preview_tab: false,
            agent_filter: None,
            loaded_markdown: String::new(),
            selected_lint: None,
            _subscriptions: Subscription::join(workspace_sub, search_sub),
        };
        view.on_data_reloaded(window, cx);
        view
    }

    fn on_data_reloaded(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prev_selected = self.selected_skill_id.clone();
        let last_added = self
            .workspace
            .update(cx, |ws, _cx| ws.take_last_added_skill_id());
        if let Some(skill_id) = last_added {
            self.selected_skill_id = Some(skill_id);
            self.editing_markdown = false;
        } else {
            let still_exists = {
                let ws = self.workspace.read(cx);
                self.selected_skill_id
                    .as_ref()
                    .map(|id| ws.skills().iter().any(|s| &s.id == id))
                    .unwrap_or(false)
            };
            if !still_exists {
                self.selected_skill_id = self
                    .workspace
                    .read(cx)
                    .skills()
                    .first()
                    .map(|s| s.id.clone());
                self.editing_markdown = false;
            }
        }
        if let Some(proj_id) = self.selected_project_id.as_ref() {
            let still_attached = self
                .workspace
                .read(cx)
                .projects()
                .iter()
                .any(|p| &p.id == proj_id);
            if !still_attached {
                self.selected_project_id = None;
                self.project_overview = None;
            }
        }
        if self.agent_filter.as_ref().is_some_and(|key| {
            !self
                .workspace
                .read(cx)
                .agents()
                .iter()
                .any(|agent| &agent.key == key && agent.installed && agent.enabled)
        }) {
            self.agent_filter = None;
        }
        self.refilter(cx);
        let preserve_editor = self.editing_markdown && self.selected_skill_id == prev_selected;
        self.reload_selected_details(preserve_editor, window, cx);
        cx.notify();
    }

    fn apply_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.workspace.read(cx).t();
        let ph_search = s.ph_search_skills;
        let ph_markdown = s.ph_skill_md;
        self.library_search
            .update(cx, |state, cx| state.set_placeholder(ph_search, window, cx));
        self.markdown_editor.update(cx, |state, cx| {
            state.set_placeholder(ph_markdown, window, cx)
        });
        cx.notify();
    }

    pub(super) fn refilter(&mut self, cx: &Context<Self>) {
        let query = self.library_search.read(cx).value().trim().to_lowercase();
        let skills = self.workspace.read(cx).skills();
        self.filtered_indices = skills
            .iter()
            .enumerate()
            .filter(|(_, skill)| {
                let matches_search = query.is_empty()
                    || skill.name.to_lowercase().contains(&query)
                    || skill.slug.to_lowercase().contains(&query)
                    || skill
                        .description
                        .as_deref()
                        .is_some_and(|d| d.to_lowercase().contains(&query))
                    || skill.tags.iter().any(|t| t.to_lowercase().contains(&query));
                matches_search
                    && self
                        .agent_filter
                        .as_deref()
                        .is_none_or(|agent| self.deployed(skill, agent))
            })
            .map(|(ix, _)| ix)
            .collect();
    }

    pub(super) fn deployed(&self, skill: &unoskills_core::Skill, agent: &str) -> bool {
        match self.selected_project_id.as_deref() {
            Some(project) => skill.is_deployed_to_project(project, agent),
            None => skill.is_deployed_to(agent),
        }
    }

    pub(super) fn set_scope(&mut self, project: Option<String>, cx: &mut Context<Self>) {
        self.selected_project_id = project;
        self.refilter(cx);
        cx.notify();
    }

    fn confirm_discard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if !self.editing_markdown
            || self.markdown_editor.read(cx).value().as_ref() == self.loaded_markdown
        {
            then(self, window, cx);
            return;
        }
        let s = self.workspace.read(cx).t();
        let view = cx.weak_entity();
        let then = std::rc::Rc::new(then);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            let then = then.clone();
            alert
                .confirm()
                .centered()
                .title(s.library_discard_title)
                .ok_text(s.library_discard_action)
                .ok_variant(ButtonVariant::Danger)
                .cancel_text(s.btn_cancel)
                .on_ok(move |_, window, cx| {
                    let _ = view.update(cx, |view, cx| then(view, window, cx));
                    true
                })
        });
    }

    pub(super) fn select_skill(
        &mut self,
        skill_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_skill_id.as_deref() == Some(skill_id.as_str()) {
            return;
        }
        self.confirm_discard(window, cx, move |view, window, cx| {
            view.selected_skill_id = Some(skill_id.clone());
            view.editing_markdown = false;
            view.reload_selected_details(false, window, cx);
            cx.notify();
        });
    }

    /// Reloads the selected skill's markdown into the retained preview /
    /// editor states. Preserves unsaved edits in `markdown_editor` when
    /// `preserve_editor` is true.
    pub(super) fn reload_selected_details(
        &mut self,
        preserve_editor: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(skill_id) = self.selected_skill_id.clone() else {
            self.selected_lint = None;
            self.markdown_preview_state
                .update(cx, |state, cx| state.set_text("", cx));
            return;
        };
        let manager = self.workspace.read(cx).manager().clone();
        match manager.read_skill_markdown(&skill_id) {
            Ok((_entry, markdown)) => {
                let body = unoskills_core::parser::strip_frontmatter(&markdown).to_string();
                self.markdown_preview_state
                    .update(cx, |state, cx| state.set_text(&body, cx));
                if !preserve_editor {
                    self.loaded_markdown.clone_from(&markdown);
                    self.markdown_editor.update(cx, |editor, cx| {
                        editor.set_value(markdown, window, cx);
                    });
                }
            }
            Err(_) => {
                self.markdown_preview_state
                    .update(cx, |state, cx| state.set_text("", cx));
            }
        }
        self.selected_lint = manager.lint_skill(&skill_id).ok();
    }

    pub(super) fn save_markdown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(skill_id) = self.selected_skill_id.clone() else {
            return;
        };
        let content = self.markdown_editor.read(cx).value().to_string();
        let saved = self.workspace.update(cx, |ws, cx| {
            ws.save_skill_markdown(&skill_id, &content, window, cx)
        });
        if saved {
            self.editing_markdown = false;
        }
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.library_search.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    }

    pub fn handle_save_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing_markdown {
            self.save_markdown(window, cx);
        }
    }

    pub fn handle_cancel_edit_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing_markdown {
            self.confirm_discard(window, cx, |view, window, cx| {
                view.editing_markdown = false;
                view.reload_selected_details(false, window, cx);
                cx.notify();
            });
        }
    }

    pub(super) fn pick_and_attach_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.weak_entity();
        self.workspace.update(cx, |ws, cx| {
            ws.pick_and_attach_project(window, cx, move |id, _, cx| {
                let _ = view.update(cx, |view, cx| {
                    view.set_scope(Some(id), cx);
                });
            });
        });
    }

    pub(super) fn current_skill<'a>(
        &self,
        cx: &'a Context<Self>,
    ) -> Option<&'a unoskills_core::Skill> {
        let ws = self.workspace.read(cx);
        let id = self.selected_skill_id.as_ref()?;
        ws.skills().iter().find(|s| &s.id == id)
    }
}

impl Render for LibraryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(project) = &self.project_overview {
            return project.clone().into_any_element();
        }
        let s = self.workspace.read(cx).t();
        let toolbar = self.render_library_toolbar(cx);
        let left_pane = self.render_skill_list(cx);
        let right_pane = self.render_skill_inspector(cx);

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                PageHeader::new(s.nav_library).right_slot(
                    h_flex()
                        .gap_2()
                        .when_some(self.selected_project_id.clone(), |row, id| {
                            row.child(
                                Button::new("open-project-overview")
                                    .outline()
                                    .icon(IconName::FolderOpen)
                                    .label(s.project_overview)
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.show_project(&id, window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("lib-add-skill")
                                .outline()
                                .icon(IconName::Plus)
                                .label(s.btn_install)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.workspace.update(cx, |ws, cx| ws.open_install(cx));
                                })),
                        ),
                ),
            )
            .child(toolbar)
            .child(
                div().flex_1().min_h_0().overflow_hidden().child(
                    h_resizable("library-split")
                        .child(
                            resizable_panel()
                                .size(px(408.))
                                .size_range(px(330.)..px(560.))
                                .child(left_pane),
                        )
                        .child(
                            resizable_panel()
                                .size_range(px(384.)..px(f32::MAX))
                                .child(right_pane),
                        ),
                ),
            )
            .into_any_element()
    }
}
