use gpui_kit::component::{
    ActiveTheme, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariant, ButtonVariants},
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    input::{Input, Textarea},
    scroll::ScrollableElement,
    v_flex,
};
use gpui_kit::{
    Context, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::FluentBuilder,
    px,
};

use super::{StacksSubTab, StacksView};

impl StacksView {
    pub(super) fn render_create_import(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let s = self.workspace.read(cx).t();
        let selected_ids = self.stack_selected_skill_ids.clone();
        let skill_count = self.workspace.read(cx).skills().len();

        div()
            .size_full()
            .min_h_0()
            .overflow_y_scrollbar()
            .id("create-import-stack-scroll")
            .child(
                h_flex()
                    .w_full()
                    .p_5()
                    .gap_5()
                    .items_start()
                    .child(
                        div().flex_1().min_w_0().child(
                            GroupBox::new()
                                .outline()
                                .title(s.create_stack_title)
                                .child(
                                    v_flex()
                                        .gap_3()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(s.create_stack_desc),
                                        )
                                        .child(
                                            v_flex()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_medium()
                                                        .child(s.field_stack_name),
                                                )
                                                .child(
                                                    Input::new(&self.stack_name_input).small(),
                                                ),
                                        )
                                        .child(
                                            v_flex()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_medium()
                                                        .child(s.field_stack_author),
                                                )
                                                .child(
                                                    Input::new(&self.stack_author_input).small(),
                                                ),
                                        )
                                        .child(
                                            v_flex()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_medium()
                                                        .child(s.field_stack_desc),
                                                )
                                                .child(
                                                    Input::new(&self.stack_desc_input).small(),
                                                ),
                                        )
                                        .child(
                                            v_flex()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_medium()
                                                        .child(format!(
                                                            "{} ({})",
                                                            s.field_select_skills,
                                                            selected_ids.len()
                                                        )),
                                                )
                                                .child(
                                                    h_flex()
                                                        .flex_wrap()
                                                        .gap_2()
                                                        .when(skill_count == 0, |row| {
                                                            row.child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(
                                                                        theme.muted_foreground,
                                                                    )
                                                                    .child(
                                                                        s.no_skills_to_bundle,
                                                                    ),
                                                            )
                                                        })
                                                        .children({
                                                            let skills =
                                                                self.workspace.read(cx).skills();
                                                            skills.iter().map(|skill| {
                                                                let checked = selected_ids
                                                                    .contains(&skill.id);
                                                                let skill_id = skill.id.clone();
                                                                Button::new(SharedString::from(
                                                                    format!(
                                                                        "pick-sk-{}",
                                                                        skill.id
                                                                    ),
                                                                ))
                                                                .xsmall()
                                                                .with_variant(if checked {
                                                                    ButtonVariant::Primary
                                                                } else {
                                                                    ButtonVariant::Secondary
                                                                })
                                                                .label(skill.name.clone())
                                                                .on_click(cx.listener(
                                                                    move |this, _, _, cx| {
                                                                        if !this
                                                                            .stack_selected_skill_ids
                                                                            .remove(&skill_id)
                                                                        {
                                                                            this.stack_selected_skill_ids
                                                                                .insert(
                                                                                    skill_id
                                                                                        .clone(),
                                                                                );
                                                                        }
                                                                        cx.notify();
                                                                    },
                                                                ))
                                                            })
                                                        }),
                                                ),
                                        )
                                        .child(
                                            Button::new("save-new-stack-btn")
                                                .primary()
                                                .small()
                                                .icon(IconName::Check)
                                                .label(s.btn_save_stack)
                                                .on_click(cx.listener(
                                                    |this, _, window, cx| {
                                                        this.create_local_stack(window, cx);
                                                    },
                                                )),
                                        ),
                                ),
                        ),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            GroupBox::new()
                                .outline()
                                .title(s.import_stack_title)
                                .child(
                                    v_flex()
                                        .gap_3()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(s.import_stack_desc),
                                        )
                                        .child(
                                            Textarea::new(&self.stack_import_toml).h(px(300.)),
                                        )
                                        .child(
                                            Button::new("import-stack-toml-btn")
                                                .primary()
                                                .small()
                                                .icon(IconName::Plus)
                                                .label(s.btn_import_stack)
                                                .on_click(cx.listener(
                                                    |this, _, window, cx| {
                                                        this.import_stack_toml(window, cx);
                                                    },
                                                )),
                                        ),
                                ),
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

    fn import_stack_toml(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.stack_import_toml.read(cx).value().to_string();
        let imported = self
            .workspace
            .update(cx, |ws, cx| ws.import_stack_toml(&raw, window, cx));
        if let Some(stack) = imported {
            self.sub_tab = StacksSubTab::Mine;
            self.selected_stack_id = Some(stack.id.clone());
            cx.notify();
        }
    }
}
