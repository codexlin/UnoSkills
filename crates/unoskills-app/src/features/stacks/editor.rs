use gpui_kit::component::{
    ActiveTheme, IconName, StyledExt, WindowExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    input::{Input, Textarea},
    scroll::ScrollableElement,
    v_flex,
};
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, div, px, uniform_list,
};

use super::{StacksSubTab, StacksView};

impl StacksView {
    pub(super) fn render_create_stack(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let s = self.workspace.read(cx).t();
        let color = cx.theme().muted_foreground;
        let query = self
            .stack_picker_search
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let skills = self.workspace.read(cx).skills();
        let mut selected_count = 0;
        let indices: Vec<_> = skills
            .iter()
            .enumerate()
            .filter_map(|(index, skill)| {
                if self.stack_selected_skill_ids.contains(&skill.id) {
                    selected_count += 1;
                }
                (query.is_empty() || skill.name.to_lowercase().contains(&query)).then_some(index)
            })
            .collect();
        let visible = indices.len();
        let title = |label| div().text_sm().font_semibold().child(label);
        div().id("create-stack-scroll").size_full().min_h_0().overflow_y_scrollbar()
            .child(v_flex().w_full().max_w(px(720.)).gap_3().p_5()
                .child(div().text_sm().text_color(color).child(s.create_stack_desc))
                .child(v_flex().gap_1().child(title(s.field_stack_name)).child(Input::new(&self.stack_name_input)))
                .child(v_flex().gap_1().child(title(s.field_stack_author)).child(Input::new(&self.stack_author_input)))
                .child(v_flex().gap_1().child(title(s.field_stack_desc)).child(Input::new(&self.stack_desc_input)))
                .child(v_flex().gap_2()
                    .child(div().text_sm().font_semibold().child(format!("{} ({selected_count})", s.field_select_skills)))
                    .child(Input::new(&self.stack_picker_search).cleanable(true))
                    .child(if visible == 0 {
                        div().text_sm().text_color(color).child(if skills.is_empty() { s.no_skills_to_bundle } else { s.empty_search_title }).into_any_element()
                    } else {
                        div().relative().h(px((visible.min(5) * 36) as f32)).overflow_hidden().child(
                            uniform_list("stack-skill-picker", visible, cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                range.filter_map(|i| {
                                    let skill = this.workspace.read(cx).skills().get(*indices.get(i)?)?;
                                    let id = skill.id.clone();
                                    let selected = this.stack_selected_skill_ids.contains(&id);
                                    Some(h_flex().h(px(36.)).px_2().child(
                                        Checkbox::new(format!("stack-picker-{id}")).label(skill.name.clone())
                                            .checked(selected)
                                            .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                                if *checked { this.stack_selected_skill_ids.insert(id.clone()); }
                                                else { this.stack_selected_skill_ids.remove(&id); }
                                                cx.notify();
                                            })),
                                    ))
                                }).collect()
                            })).size_full().track_scroll(&self.stack_picker_scroll),
                        ).vertical_scrollbar(&self.stack_picker_scroll).into_any_element()
                    }))
                .child(h_flex().justify_end().child(Button::new("save-new-stack-btn").primary().icon(IconName::Check)
                    .label(s.btn_save_stack).on_click(cx.listener(|this, _, window, cx| this.create_local_stack(window, cx))))))
    }

    pub(super) fn render_import_stack(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let s = self.workspace.read(cx).t();
        div()
            .id("import-stack-scroll")
            .size_full()
            .min_h_0()
            .overflow_y_scrollbar()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(720.))
                    .gap_3()
                    .p_5()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(s.import_stack_desc),
                    )
                    .child(Textarea::new(&self.stack_import_toml).h(px(280.)))
                    .children(
                        self.stack_import_error.as_ref().map(|error| {
                            Alert::error("stack-import-error", error.clone()).banner()
                        }),
                    )
                    .child(
                        h_flex().justify_end().child(
                            Button::new("import-stack-toml-btn")
                                .primary()
                                .label(s.stacks_preview_import)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.preview_stack_import(window, cx);
                                })),
                        ),
                    ),
            )
    }

    fn create_local_stack(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.stack_name_input.read(cx).value().trim().to_string();
        let author = self.stack_author_input.read(cx).value().trim().to_string();
        let description = self.stack_desc_input.read(cx).value().trim().to_string();
        let skill_ids: Vec<String> = self.stack_selected_skill_ids.iter().cloned().collect();

        let created = self.workspace.update(cx, |ws, cx| {
            ws.create_local_stack(&name, &author, &description, &skill_ids, window, cx)
        });
        if let Some(stack) = created {
            self.stack_selected_skill_ids.clear();
            self.sub_tab = StacksSubTab::Mine;
            self.selected_stack_id = Some(stack.id.clone());
            cx.notify();
        }
    }

    fn preview_stack_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.stack_import_toml.read(cx).value().to_string();
        let ws = self.workspace.read(cx);
        let s = ws.t();
        let manifest = if raw.trim().is_empty() {
            Err(s.toast_paste_toml.to_string())
        } else {
            unoskills_core::stacks::parse_stack_from_toml(&raw).map_err(|error| error.to_string())
        };
        let manifest = match manifest {
            Ok(manifest) => manifest,
            Err(error) => {
                self.stack_import_error = Some(error);
                cx.notify();
                return;
            }
        };
        self.stack_import_error = None;
        cx.notify();

        let mut description = format!("{} ({})", s.included_skills, manifest.skills.len());
        for member in manifest.skills.iter().take(5) {
            description.push('\n');
            description.push_str(&member.name);
        }
        if manifest.skills.len() > 5 {
            description.push_str("\n…");
        }
        if self
            .workspace
            .read(cx)
            .local_stacks()
            .iter()
            .any(|stack| stack.id == manifest.id)
        {
            description.push_str("\n\n");
            description.push_str(s.stacks_replace_import);
        }
        let view = cx.weak_entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            let raw = raw.clone();
            alert
                .confirm()
                .centered()
                .title(manifest.name.clone())
                .description(description.clone())
                .ok_text(s.btn_import_stack)
                .cancel_text(s.btn_cancel)
                .on_ok(move |_, window, cx| {
                    view.update(cx, |view, cx| view.import_stack_toml(&raw, window, cx))
                        .unwrap_or(false)
                })
        });
    }

    fn import_stack_toml(
        &mut self,
        raw: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let imported = self
            .workspace
            .update(cx, |ws, cx| ws.import_stack_toml(raw, window, cx));
        if let Some(stack) = imported {
            self.sub_tab = StacksSubTab::Mine;
            self.selected_stack_id = Some(stack.id.clone());
            self.stack_import_toml
                .update(cx, |field, cx| field.set_value("", window, cx));
            cx.notify();
            true
        } else {
            false
        }
    }
}
