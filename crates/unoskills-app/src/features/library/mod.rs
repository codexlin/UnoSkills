mod inspector;
mod list;

use gpui_kit::component::{
    h_resizable,
    input::{InputEvent, InputState, TextareaState},
    resizable_panel,
    text::TextViewState,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Subscription,
    UniformListScrollHandle, Window, px,
};
use unoskills_core::SkillLintReport;

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
    pub(super) markdown_preview_state: Entity<TextViewState>,
    pub(super) markdown_editor: Entity<TextareaState>,
    pub(super) editing_markdown: bool,
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
            markdown_preview_state,
            markdown_editor,
            editing_markdown: false,
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
        if query.is_empty() {
            self.filtered_indices = (0..skills.len()).collect();
        } else {
            self.filtered_indices = skills
                .iter()
                .enumerate()
                .filter(|(_, skill)| {
                    skill.name.to_lowercase().contains(&query)
                        || skill.slug.to_lowercase().contains(&query)
                        || skill
                            .description
                            .as_deref()
                            .map(|d| d.to_lowercase().contains(&query))
                            .unwrap_or(false)
                        || skill.tags.iter().any(|t| t.to_lowercase().contains(&query))
                })
                .map(|(ix, _)| ix)
                .collect();
        }
    }

    pub(super) fn select_skill(
        &mut self,
        skill_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_skill_id = Some(skill_id);
        self.editing_markdown = false;
        self.reload_selected_details(false, window, cx);
        cx.notify();
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
        let left_pane = self.render_skill_list(cx);
        let right_pane = self.render_skill_inspector(cx);

        h_resizable("library-split")
            .child(
                resizable_panel()
                    .size(px(300.))
                    .size_range(px(220.)..px(480.))
                    .child(left_pane),
            )
            .child(resizable_panel().child(right_pane))
    }
}
