mod browser;
mod deploy;
mod detail;
mod editor;

use std::collections::HashSet;

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::{
    IconName, IndexPath,
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState, TextareaState},
    searchable_list::{SearchableListItem, SearchableVec},
    select::{SelectEvent, SelectState},
    v_flex,
};
use gpui_kit::{
    AppContext, Context, Entity, Focusable, IntoElement, ParentElement, Render, ScrollStrategy,
    SharedString, Styled, Subscription, UniformListScrollHandle, Window, div,
};
use unoskills_core::StackManifest;

use crate::components::page_header::{PageHeader, page_tabs};
use crate::state::{ProjectStackSelection, WorkspaceEvent, WorkspaceState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StacksSubTab {
    Featured,
    Mine,
    Create,
    Import,
}

#[derive(Clone)]
struct StackTagChoice {
    label: SharedString,
    tag: Option<String>,
}

impl SearchableListItem for StackTagChoice {
    type Value = Option<String>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.tag
    }
}

/// Stacks page: Featured community recipes, My Stacks, and a Create / Import
/// form. Owns its form inputs and the selected stack; list data is read from
/// `WorkspaceState` snapshots.
pub struct StacksView {
    pub(super) workspace: Entity<WorkspaceState>,
    pub(super) sub_tab: StacksSubTab,
    pub(super) selected_stack_id: Option<String>,
    pub(super) project_stack_target: Option<ProjectStackSelection>,
    pub(super) stack_search_input: Entity<InputState>,
    tag_filter: Entity<SelectState<SearchableVec<StackTagChoice>>>,
    pub(super) stack_scroll: UniformListScrollHandle,
    pub(super) stack_name_input: Entity<InputState>,
    pub(super) stack_author_input: Entity<InputState>,
    pub(super) stack_desc_input: Entity<InputState>,
    pub(super) stack_picker_search: Entity<InputState>,
    pub(super) stack_picker_scroll: UniformListScrollHandle,
    pub(super) stack_import_toml: Entity<TextareaState>,
    pub(super) stack_import_error: Option<String>,
    pub(super) stack_selected_skill_ids: HashSet<String>,
    _subscription: Subscription,
    _search_subscription: Subscription,
    _picker_subscription: Subscription,
    _tag_subscription: Subscription,
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
        let tags = stack_tag_options(workspace.read(cx));
        let tag_filter = cx.new(|cx| {
            SelectState::new(tags, Some(IndexPath::default()), window, cx).searchable(true)
        });
        let tag_subscription = cx.subscribe(&tag_filter, |this, _, _: &SelectEvent<_>, cx| {
            this.stack_scroll.scroll_to_item(0, ScrollStrategy::Top);
            cx.notify();
        });
        let stack_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_stack_name));
        let stack_author_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_stack_author));
        let stack_desc_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_stack_desc));
        let stack_picker_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(s.ph_search_skills));
        let picker_subscription = cx.subscribe_in(
            &stack_picker_search,
            window,
            |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.stack_picker_scroll
                        .scroll_to_item(0, gpui_kit::ScrollStrategy::Top);
                    cx.notify();
                }
            },
        );
        let stack_import_toml =
            cx.new(|cx| TextareaState::new(window, cx).placeholder(s.ph_stack_import));

        let subscription = cx.subscribe_in(
            &workspace,
            window,
            |this, _ws, event: &WorkspaceEvent, window, cx| match event {
                WorkspaceEvent::LanguageChanged => {
                    this.refresh_tag_filter(window, cx);
                    this.apply_language(window, cx);
                }
                WorkspaceEvent::Reloaded => {
                    this.refresh_tag_filter(window, cx);
                    cx.notify();
                }
                WorkspaceEvent::InstallStateChanged => cx.notify(),
                WorkspaceEvent::OpenMyStacks if this.sub_tab != StacksSubTab::Mine => {
                    this.switch_sub_tab(StacksSubTab::Mine, cx);
                }
                WorkspaceEvent::SelectProjectStack(selection) => {
                    this.switch_sub_tab(StacksSubTab::Mine, cx);
                    this.selected_stack_id = Some(selection.stack_id.clone());
                    this.project_stack_target = Some(selection.clone());
                    cx.notify();
                }
                _ => {}
            },
        );

        // Re-filter the merged stack list live as the search query changes.
        let search_subscription = cx.subscribe_in(
            &stack_search_input,
            window,
            |this, _input, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.stack_scroll
                        .scroll_to_item(0, gpui_kit::ScrollStrategy::Top);
                    cx.notify();
                }
            },
        );

        let selected_stack_id = workspace
            .read(cx)
            .local_stacks()
            .first()
            .map(|stack| stack.id.clone());

        Self {
            workspace,
            sub_tab: StacksSubTab::Mine,
            selected_stack_id,
            project_stack_target: None,
            stack_search_input,
            tag_filter,
            stack_scroll: UniformListScrollHandle::new(),
            stack_name_input,
            stack_author_input,
            stack_desc_input,
            stack_picker_search,
            stack_picker_scroll: UniformListScrollHandle::new(),
            stack_import_toml,
            stack_import_error: None,
            _picker_subscription: picker_subscription,
            stack_selected_skill_ids: HashSet::new(),
            _subscription: subscription,
            _search_subscription: search_subscription,
            _tag_subscription: tag_subscription,
        }
    }

    fn refresh_tag_filter(&self, window: &mut Window, cx: &mut Context<Self>) {
        let tags = stack_tag_options(self.workspace.read(cx));
        self.tag_filter.update(cx, |state, cx| {
            state.set_items(tags, window, cx);
            if state.selected_value().is_none() {
                state.set_selected_index(Some(IndexPath::default()), window, cx);
            }
        });
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
        self.stack_picker_search.update(cx, |state, cx| {
            state.set_placeholder(s.ph_search_skills, window, cx)
        });
        self.stack_import_toml
            .update(cx, |state, cx| state.set_placeholder(ph_import, window, cx));
        cx.notify();
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = match self.sub_tab {
            StacksSubTab::Create => &self.stack_picker_search,
            StacksSubTab::Import => return,
            _ => &self.stack_search_input,
        };
        let focus = input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    }

    pub(super) fn switch_sub_tab(&mut self, sub_tab: StacksSubTab, cx: &mut Context<Self>) {
        if self.sub_tab != sub_tab {
            self.stack_scroll
                .scroll_to_item(0, gpui_kit::ScrollStrategy::Top);
            self.sub_tab = sub_tab;
        }
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
            StacksSubTab::Create | StacksSubTab::Import => {}
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
    /// text and active tag.
    pub(super) fn merged_filtered_stacks(&self, cx: &Context<Self>) -> Vec<StackManifest> {
        let query = self
            .stack_search_input
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let active_tag = self
            .tag_filter
            .read(cx)
            .selected_value()
            .and_then(Option::as_ref);
        let mut merged = self.merged_all_stacks(cx);
        merged.retain(|stack| {
            if let Some(tag) = active_tag
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
}

fn stack_tag_options(ws: &WorkspaceState) -> SearchableVec<StackTagChoice> {
    let mut stack_ids = HashSet::new();
    let mut seen = HashSet::new();
    let mut tags = Vec::new();
    for stack in ws.featured_stacks().iter().chain(ws.community_stacks()) {
        if !stack_ids.insert(&stack.id) {
            continue;
        }
        for tag in &stack.tags {
            if seen.insert(tag.to_ascii_lowercase()) {
                tags.push(tag);
            }
        }
    }
    tags.sort_by_cached_key(|tag| tag.to_lowercase());
    SearchableVec::new(
        std::iter::once(StackTagChoice {
            label: ws.t().filter_all_tags.into(),
            tag: None,
        })
        .chain(tags.into_iter().map(|tag| StackTagChoice {
            label: tag.clone().into(),
            tag: Some(tag.clone()),
        }))
        .collect::<Vec<_>>(),
    )
}

impl Render for StacksView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.workspace.read(cx).t();
        let sub = self.sub_tab;
        let library_tabs = matches!(sub, StacksSubTab::Featured | StacksSubTab::Mine);
        let header = PageHeader::new(match sub {
            StacksSubTab::Create => s.stacks_create_title,
            StacksSubTab::Import => s.stacks_import_title,
            _ => s.stacks_title,
        });
        let header = if library_tabs {
            header.right_slot(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("stack-import-route")
                            .outline()
                            .label(s.stacks_import_title)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.switch_sub_tab(StacksSubTab::Import, cx)
                            })),
                    )
                    .child(
                        Button::new("stack-create-route")
                            .outline()
                            .icon(IconName::Plus)
                            .label(s.stacks_create_title)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.switch_sub_tab(StacksSubTab::Create, cx)
                            })),
                    ),
            )
        } else {
            header.right_slot(
                Button::new("stack-editor-back")
                    .ghost()
                    .label(s.btn_cancel)
                    .on_click(
                        cx.listener(|this, _, _, cx| this.switch_sub_tab(StacksSubTab::Mine, cx)),
                    ),
            )
        };

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(header)
            .children(library_tabs.then(|| {
                page_tabs(
                    "stacks-sub-tabs",
                    [
                        (AssetIcon::Layers, s.tab_my_stacks),
                        (AssetIcon::Users, s.tab_featured),
                    ],
                    usize::from(sub == StacksSubTab::Featured),
                    cx,
                )
                .on_click(cx.listener(|this, idx: &usize, _, cx| {
                    this.switch_sub_tab(
                        if *idx == 0 {
                            StacksSubTab::Mine
                        } else {
                            StacksSubTab::Featured
                        },
                        cx,
                    );
                }))
            }))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .overflow_hidden()
                    .child(match sub {
                        StacksSubTab::Featured => self.render_browser(false, cx).into_any_element(),
                        StacksSubTab::Mine => self.render_browser(true, cx).into_any_element(),
                        StacksSubTab::Create => self.render_create_stack(cx).into_any_element(),
                        StacksSubTab::Import => self.render_import_stack(cx).into_any_element(),
                    }),
            )
    }
}
