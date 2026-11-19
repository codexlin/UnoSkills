use gpui_kit::component::{ActiveTheme, StyledExt, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder,
};

/// Unified page header: title + optional subtitle on the left and an optional
/// right slot (usually a segmented `TabBar`).
///
/// The title/subtitle column carries `.flex_1().min_w_0()` and the right slot
/// `.flex_shrink_0()`, so variable-length text never overflows horizontally
/// at the default 1240x780 window.
#[derive(IntoElement)]
pub struct PageHeader {
    title: SharedString,
    subtitle: Option<SharedString>,
    right: Option<AnyElement>,
}

impl PageHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            right: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn right_slot(mut self, right: impl IntoElement) -> Self {
        self.right = Some(right.into_any_element());
        self
    }
}

impl RenderOnce for PageHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .w_full()
            .px_5()
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(div().text_lg().font_semibold().child(self.title))
                    .when_some(self.subtitle, |header, subtitle| {
                        header.child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(subtitle),
                        )
                    }),
            )
            .when_some(self.right, |header, right| {
                header.child(div().flex_shrink_0().child(right))
            })
    }
}
