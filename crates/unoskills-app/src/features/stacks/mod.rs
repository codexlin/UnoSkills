mod browser;
mod detail;
mod editor;

use std::collections::HashSet;

use gpui_kit::component::{
    Sizable,
    input::{InputEvent, InputState, TextareaState},
    tab::{Tab, TabBar},
    v_flex,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div,
};
use unoskills_core::StackManifest;

use crate::components::page_header::PageHeader;
use crate::state::{WorkspaceEvent, WorkspaceState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StacksSubTab {
    Featured,
    Mine,
    CreateImport,
}

/// Stacks page: Featured community recipes, My Stacks, and a Create / Import
/// form. Owns its form inputs and the selected stack; list data is read from
/// `WorkspaceState` snapshots.
pub struct StacksView {
    pub(super) workspace: Entity<WorkspaceState>,
    pub(super) sub_tab: StacksSubTab,
    pub(super) selected_stack_id: Option<String>,
    pub(super) stack_search_input: Entity<InputState>,
    pub(super) active_tag: Option<String>,
    pub(super) stack_name_input: Entity<InputState>,
    pub(super) stack_author_input: Entity<InputState>,
    pub(super) stack_desc_input: Entity<InputState>,
    pub(super) stack_import_toml: Entity<TextareaState>,
    pub(super) stack_selected_skill_ids: HashSet<String>,
    _subscription: Subscription,
    _search_subscription: Subscription,
}

impl StacksView {
    pub fn new(
        workspace: Entity<WorkspaceState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let s = workspace.read(cx).t();
        let stack_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_stack_search));
        let stack_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_stack_name));
        let stack_author_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_stack_author));
        let stack_desc_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_stack_desc));
        let stack_import_toml =
            cx.new(|cx| TextareaState::new(window, cx).placeholder(s.ph_stack_import));

        let subscription = cx.subscribe_in(
            &workspace,
            window,
            |this, _ws, event: &WorkspaceEvent, window, cx| match event {
                WorkspaceEvent::LanguageChanged => this.apply_language(window, cx),
                WorkspaceEvent::Reloaded => cx.notify(),
                _ => {}
            },
        );

        // Re-filter the merged stack list live as the search query changes.
        let search_subscription = cx.subscribe_in(
            &stack_search_input,
            window,
            |_this, _input, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );

        let selected_stack_id = workspace
            .read(cx)
            .featured_stacks()
            .first()
            .map(|stack| stack.id.clone());

        Self {
            workspace,
            sub_tab: StacksSubTab::Featured,
            selected_stack_id,
            stack_search_input,
            active_tag: None,
            stack_name_input,
            stack_author_input,
            stack_desc_input,
            stack_import_toml,
            stack_selected_skill_ids: HashSet::new(),
            _subscription: subscription,
            _search_subscription: search_subscription,
        }
    }

    fn apply_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.workspace.read(cx).t();
        let ph_search = s.ph_stack_search;
        let ph_name = s.ph_stack_name;
        let ph_author = s.ph_stack_author;
        let ph_desc = s.ph_stack_desc;
        let ph_import = s.ph_stack_import;
        self.stack_search_input
            .update(cx, |state, cx| state.set_placeholder(ph_search, window, cx));
        self.stack_name_input
            .update(cx, |state, cx| state.set_placeholder(ph_name, window, cx));
        self.stack_author_input
            .update(cx, |state, cx| state.set_placeholder(ph_author, window, cx));
        self.stack_desc_input
            .update(cx, |state, cx| state.set_placeholder(ph_desc, window, cx));
        self.stack_import_toml
            .update(cx, |state, cx| state.set_placeholder(ph_import, window, cx));
        cx.notify();
    }

    pub(super) fn switch_sub_tab(&mut self, sub_tab: StacksSubTab, cx: &mut Context<Self>) {
        self.sub_tab = sub_tab;
        match sub_tab {
            StacksSubTab::Featured => {
                self.selected_stack_id = self
                    .merged_filtered_stacks(cx)
                    .first()
                    .map(|stack| stack.id.clone());
            }
            StacksSubTab::Mine => {
                self.selected_stack_id = self
                    .workspace
                    .read(cx)
                    .local_stacks()
                    .first()
                    .map(|stack| stack.id.clone());
            }
            StacksSubTab::CreateImport => {}
        }
        cx.notify();
    }

    /// Featured + community stacks deduplicated by `id` (built-in featured
    /// stacks win on collision). Community stacks sort after built-in ones.
    pub(super) fn merged_all_stacks(&self, cx: &Context<Self>) -> Vec<StackManifest> {
        let ws = self.workspace.read(cx);
        let mut seen = HashSet::new();
        let mut merged = Vec::new();
        for stack in ws
            .featured_stacks()
            .iter()
            .chain(ws.community_stacks().iter())
        {
            if seen.insert(stack.id.clone()) {
                merged.push(stack.clone());
            }
        }
        merged
    }

    /// Deduplicated featured + community stacks filtered by the current search
    /// text and active tag pill.
    pub(super) fn merged_filtered_stacks(&self, cx: &Context<Self>) -> Vec<StackManifest> {
        let query = self
            .stack_search_input
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let active_tag = self.active_tag.clone();
        let mut merged = self.merged_all_stacks(cx);
        merged.retain(|stack| {
            if let Some(tag) = &active_tag
                && !stack.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
            {
                return false;
            }
            if query.is_empty() {
                return true;
            }
            let haystack = format!(
                "{} {} {} {} {}",
                stack.name,
                stack.id,
                stack.author,
                stack.description,
                stack.tags.join(" ")
            );
            haystack.to_lowercase().contains(&query)
        });
        merged
    }

    /// Distinct tags across the merged (pre-filter) stack list, capped for the
    /// pill row so the filter never overflows the panel.
    pub(super) fn available_tags(&self, cx: &Context<Self>) -> Vec<String> {
        let ws = self.workspace.read(cx);
        let mut seen = std::collections::HashSet::new();
        let mut tags = Vec::new();
        for stack in ws
            .featured_stacks()
            .iter()
            .chain(ws.community_stacks().iter())
        {
            for tag in &stack.tags {
                let key = tag.to_lowercase();
                if seen.insert(key) {
                    tags.push(tag.clone());
                }
                if tags.len() >= 10 {
                    return tags;
                }
            }
        }
        tags
    }
}

impl Render for StacksView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.workspace.read(cx).t();
        let sub = self.sub_tab;
        let selected_idx = match sub {
            StacksSubTab::Featured => 0,
            StacksSubTab::Mine => 1,
            StacksSubTab::CreateImport => 2,
        };
        let featured_count = {
            let ws = self.workspace.read(cx);
            let featured = ws.featured_stacks();
            let extra_community = ws
                .community_stacks()
                .iter()
                .filter(|c| !featured.iter().any(|f| f.id == c.id))
                .count();
            featured.len() + extra_community
        };
        let local_count = self.workspace.read(cx).local_stacks().len();

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                PageHeader::new(s.stacks_title)
                    .subtitle(s.stacks_subtitle)
                    .right_slot(
                        TabBar::new("stacks-sub-tabs")
                            .segmented()
                            .small()
                            .selected_index(selected_idx)
                            .child(
                                Tab::new().label(format!("{} ({featured_count})", s.tab_featured)),
                            )
                            .child(Tab::new().label(format!("{} ({local_count})", s.tab_my_stacks)))
                            .child(Tab::new().label(s.tab_create_share))
                            .on_click(cx.listener(|this, idx: &usize, _, cx| match *idx {
                                0 => this.switch_sub_tab(StacksSubTab::Featured, cx),
                                1 => this.switch_sub_tab(StacksSubTab::Mine, cx),
                                _ => this.switch_sub_tab(StacksSubTab::CreateImport, cx),
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .overflow_hidden()
                    .child(match sub {
                        StacksSubTab::Featured => self.render_browser(false, cx).into_any_element(),
                        StacksSubTab::Mine => self.render_browser(true, cx).into_any_element(),
                        StacksSubTab::CreateImport => {
                            self.render_create_import(cx).into_any_element()
                        }
                    }),
            )
    }
}
