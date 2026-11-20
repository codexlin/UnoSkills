use std::{
    collections::{BTreeMap, HashSet},
    ops::Range,
};

use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex,
    input::Input,
    scroll::ScrollableElement,
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, prelude::FluentBuilder, px, uniform_list,
};

use super::DiscoverView;
use crate::components::badges::TokenBadge;

pub(super) struct AdoptGroup {
    key: String,
    name: String,
    indices: Vec<usize>,
}

pub(super) enum AdoptRow {
    Group(usize),
    Candidate(usize),
}

impl DiscoverView {
    pub(super) fn refilter_adopt(&mut self, cx: &Context<Self>) {
        let query = self
            .adopt_search_input
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let candidates = self.workspace.read(cx).adopt_candidates();
        let paths: HashSet<_> = candidates.iter().map(|candidate| &candidate.path).collect();
        self.selected_adopt_paths
            .retain(|path| paths.contains(path));
        let mut groups = BTreeMap::new();
        for (index, candidate) in candidates.iter().enumerate() {
            if query.is_empty()
                || candidate.name.to_lowercase().contains(&query)
                || candidate.agent_name.to_lowercase().contains(&query)
                || candidate
                    .path
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&query)
            {
                groups
                    .entry((&candidate.agent_name, &candidate.agent_key))
                    .or_insert_with(Vec::new)
                    .push(index);
            }
        }
        self.adopt_groups = groups
            .into_iter()
            .map(|((name, key), indices)| AdoptGroup {
                name: name.clone(),
                key: key.clone(),
                indices,
            })
            .collect();
        self.rebuild_adopt_rows();
    }

    fn rebuild_adopt_rows(&mut self) {
        self.adopt_rows.clear();
        for (index, group) in self.adopt_groups.iter().enumerate() {
            self.adopt_rows.push(AdoptRow::Group(index));
            if !self.collapsed_adopt_groups.contains(&group.key) {
                self.adopt_rows
                    .extend(group.indices.iter().copied().map(AdoptRow::Candidate));
            }
        }
    }

    fn confirm_adopt(&self, window: &mut gpui_kit::Window, cx: &mut Context<Self>) {
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let candidates: Vec<_> = ws
            .adopt_candidates()
            .iter()
            .filter(|candidate| self.selected_adopt_paths.contains(&candidate.path))
            .cloned()
            .collect();
        if candidates.is_empty() || ws.adopting_all() {
            return;
        }
        let selected = ws.lang().adopt_selected_count(candidates.len());
        let description = s.adopt_confirm_description;
        let workspace = self.workspace.downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let workspace = workspace.clone();
            let candidates = candidates.clone();
            alert
                .confirm()
                .centered()
                .title(selected.clone())
                .description(description)
                .ok_text(s.btn_adopt)
                .cancel_text(s.btn_cancel)
                .on_ok(move |_, window, cx| {
                    let _ = workspace.update(cx, |ws, cx| {
                        ws.adopt_candidates_async(candidates.clone(), window, cx)
                    });
                    true
                })
        });
    }

    pub(super) fn render_adopt_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let total = ws.adopt_candidates().len();
        let busy = ws.adopting_all();
        let count = self.adopt_rows.len();
        let selected = self.selected_adopt_paths.len();
        let selected_label = ws.lang().adopt_selected_count(selected);

        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                h_flex()
                    .px_5()
                    .py_2()
                    .min_h(px(48.))
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .max_w(px(360.))
                            .child(Input::new(&self.adopt_search_input).cleanable(true)),
                    )
                    .child(
                        Button::new("rescan-adopt")
                            .outline()
                            .icon(IconName::RefreshCw)
                            .label(s.btn_rescan)
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workspace.update(cx, |ws, cx| ws.reload_all(cx))
                            })),
                    ),
            )
            .child(if count == 0 {
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(
                        Empty::new().header(
                            EmptyHeader::new()
                                .media(
                                    EmptyMedia::new()
                                        .child(Icon::new(IconName::CircleCheck).size_6()),
                                )
                                .title(EmptyTitle::new().child(if total == 0 {
                                    s.empty_adopt_done_title
                                } else {
                                    s.empty_adopt_filter_title
                                }))
                                .description(EmptyDescription::new().child(if total == 0 {
                                    s.empty_adopt_done_desc
                                } else {
                                    s.empty_adopt_filter_desc
                                })),
                        ),
                    )
                    .into_any_element()
            } else {
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "adopt-vlist",
                            count,
                            cx.processor(|this, range: Range<usize>, _, cx| {
                                let ws = this.workspace.read(cx);
                                let s = ws.t();
                                let theme = cx.theme();
                                let border = theme.border;
                                let muted = theme.muted_foreground;
                                let busy = ws.adopting_all();
                                range
                                    .filter_map(|index| {
                                        let row = match *this.adopt_rows.get(index)? {
                                            AdoptRow::Group(group_index) => {
                                                let group = this.adopt_groups.get(group_index)?;
                                                let key = group.key.clone();
                                                let paths: Vec<_> = group
                                                    .indices
                                                    .iter()
                                                    .filter_map(|index| {
                                                        ws.adopt_candidates().get(*index)
                                                    })
                                                    .map(|candidate| candidate.path.clone())
                                                    .collect();
                                                let all_selected = paths.iter().all(|path| {
                                                    this.selected_adopt_paths.contains(path)
                                                });
                                                let collapsed =
                                                    this.collapsed_adopt_groups.contains(&key);
                                                h_flex()
                                                    .gap_2()
                                                    .bg(theme.secondary)
                                                    .px_2()
                                                    .child(
                                                        Button::new(format!("adopt-group-{key}"))
                                                            .ghost()
                                                            .icon(if collapsed {
                                                                IconName::ChevronRight
                                                            } else {
                                                                IconName::ChevronDown
                                                            })
                                                            .min_w_0()
                                                            .flex_shrink_1()
                                                            .label(format!(
                                                                "{} ({})",
                                                                group.name,
                                                                paths.len()
                                                            ))
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    if !this
                                                                        .collapsed_adopt_groups
                                                                        .remove(&key)
                                                                    {
                                                                        this.collapsed_adopt_groups
                                                                            .insert(key.clone());
                                                                    }
                                                                    this.rebuild_adopt_rows();
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    )
                                                    .child(div().flex_1())
                                                    .child(
                                                        Button::new(format!(
                                                            "adopt-select-group-{}",
                                                            group.key
                                                        ))
                                                        .ghost()
                                                        .small()
                                                        .label(if all_selected {
                                                            s.adopt_clear_selection
                                                        } else {
                                                            s.adopt_select_group
                                                        })
                                                        .disabled(busy)
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                if all_selected {
                                                                    for path in &paths {
                                                                        this.selected_adopt_paths
                                                                            .remove(path);
                                                                    }
                                                                } else {
                                                                    this.selected_adopt_paths
                                                                        .extend(
                                                                            paths.iter().cloned(),
                                                                        );
                                                                }
                                                                cx.notify();
                                                            },
                                                        )),
                                                    )
                                            }
                                            AdoptRow::Candidate(candidate_index) => {
                                                let candidate =
                                                    ws.adopt_candidates().get(candidate_index)?;
                                                let path = candidate.path.clone();
                                                let display_path =
                                                    path.to_string_lossy().into_owned();
                                                let hint = display_path.clone();
                                                h_flex()
                                                    .gap_2()
                                                    .px_2()
                                                    .border_b_1()
                                                    .border_color(border)
                                                    .child(
                                                        Checkbox::new(format!(
                                                            "adopt-candidate-{candidate_index}"
                                                        ))
                                                        .accessibility_label(candidate.name.clone())
                                                        .checked(
                                                            this.selected_adopt_paths
                                                                .contains(&path),
                                                        )
                                                        .disabled(busy)
                                                        .on_click(cx.listener(
                                                            move |this, checked: &bool, _, cx| {
                                                                if *checked {
                                                                    this.selected_adopt_paths
                                                                        .insert(path.clone());
                                                                } else {
                                                                    this.selected_adopt_paths
                                                                        .remove(&path);
                                                                }
                                                                cx.notify();
                                                            },
                                                        )),
                                                    )
                                                    .child(
                                                        div()
                                                            .w_1_3()
                                                            .min_w_0()
                                                            .text_sm()
                                                            .truncate()
                                                            .child(candidate.name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .id("adopt-path")
                                                            .flex_1()
                                                            .min_w_0()
                                                            .text_xs()
                                                            .text_color(muted)
                                                            .truncate()
                                                            .tooltip(move |window, cx| {
                                                                Tooltip::new(hint.clone())
                                                                    .build(window, cx)
                                                            })
                                                            .child(display_path),
                                                    )
                                                    .child(div().flex_shrink_0().child(
                                                        TokenBadge::tokens(
                                                            candidate.estimated_tokens,
                                                        ),
                                                    ))
                                            }
                                        };
                                        Some(div().w_full().px_5().child(row.h(px(40.)).w_full()))
                                    })
                                    .collect()
                            }),
                        )
                        .size_full()
                        .track_scroll(&self.adopt_scroll_handle),
                    )
                    .vertical_scrollbar(&self.adopt_scroll_handle)
                    .into_any_element()
            })
            .when(selected > 0, |pane| {
                pane.child(
                    h_flex()
                        .min_h(px(48.))
                        .px_5()
                        .gap_2()
                        .border_t_1()
                        .border_color(theme.border)
                        .child(div().flex_1().min_w_0().text_sm().child(selected_label))
                        .child(
                            Button::new("adopt-clear-selection")
                                .ghost()
                                .label(s.adopt_clear_selection)
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.selected_adopt_paths.clear();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("adopt-confirm-selection")
                                .primary()
                                .label(format!("{} ({selected})", s.btn_adopt))
                                .loading(busy)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_adopt(window, cx)
                                })),
                        ),
                )
            })
    }
}
