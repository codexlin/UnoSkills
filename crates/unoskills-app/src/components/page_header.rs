use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, StyledExt, h_flex,
    tab::{Tab, TabBar},
};
use gpui_kit::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, prelude::FluentBuilder, px,
};

/// Shared desktop page title and action row.
#[derive(IntoElement)]
pub struct PageHeader {
    title: SharedString,
    right: Option<AnyElement>,
}

impl PageHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            right: None,
        }
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
            .min_h(px(56.))
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_xl()
                    .font_semibold()
                    .truncate()
                    .child(self.title),
            )
            .when_some(self.right, |header, right| {
                header.child(div().flex_shrink_0().child(right))
            })
    }
}

/// Underlined tabs are navigation within a page or inspector.
pub fn page_tabs(
    id: impl Into<ElementId>,
    tabs: impl IntoIterator<Item = (IconName, impl Into<SharedString>)>,
    selected: usize,
    cx: &App,
) -> TabBar {
    TabBar::new(id)
        .underline()
        .w_full()
        .min_h(px(36.))
        .px_5()
        .border_b_1()
        .border_color(cx.theme().border)
        .selected_index(selected)
        .children(
            tabs.into_iter()
                .map(|(icon, label)| Tab::new().label(label).prefix(Icon::new(icon).size_3p5())),
        )
}
