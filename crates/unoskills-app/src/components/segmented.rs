use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::transition;
use gpui_kit::component::{ActiveTheme, Icon, h_flex};
use gpui_kit::{
    App, Bounds, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, TextRun, Window, div, point, prelude::FluentBuilder, px,
    rems, size,
};

use super::motion::control_transition;

const TRACK_HEIGHT_REM: f32 = 2.0;
const TRACK_PADDING: f32 = 2.0;
const SEGMENT_PADDING_X: f32 = 12.0;
/// Segment text uses `.text_sm()` (0.875rem).
const TEXT_REM: f32 = 0.875;

type SegmentHandler = dyn Fn(&usize, &mut Window, &mut App);

/// The selection indicator moves and resizes together. Repeated clicks
/// retarget it from its current bounds.
///
/// Shape the labels with the active UI font so the indicator fits every locale
/// and follows the configured font size.
#[derive(IntoElement)]
pub struct SegmentedControl {
    id: SharedString,
    labels: Vec<SharedString>,
    icons: Vec<IconName>,
    selected: usize,
    on_click: Option<Rc<SegmentHandler>>,
}

impl SegmentedControl {
    pub fn new<L: Into<SharedString>>(
        id: impl Into<SharedString>,
        labels: impl IntoIterator<Item = L>,
        selected: usize,
    ) -> Self {
        Self {
            id: id.into(),
            labels: labels.into_iter().map(Into::into).collect(),
            icons: Vec::new(),
            selected,
            on_click: None,
        }
    }

    pub fn icons(mut self, icons: impl IntoIterator<Item = IconName>) -> Self {
        self.icons = icons.into_iter().collect();
        self
    }

    pub fn on_click(mut self, listener: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(listener));
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_px = window.rem_size() * TEXT_REM;
        let track_height = window.rem_size() * TRACK_HEIGHT_REM;
        let font = window.text_style().font();
        let icon_space = window.rem_size() * (TEXT_REM + 0.25);

        let widths: Vec<f32> = self
            .labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let run = TextRun {
                    len: label.len(),
                    font: font.clone(),
                    ..Default::default()
                };
                let width = window
                    .text_system()
                    .shape_line(label.clone(), text_px, &[run], None)
                    .width();
                let icon_width = if index < self.icons.len() {
                    f32::from(icon_space)
                } else {
                    0.
                };
                f32::from(width).ceil() + icon_width + SEGMENT_PADDING_X * 2.0 + 2.0
            })
            .collect();
        let selected = self.selected.min(widths.len().saturating_sub(1));
        let target_x: f32 = widths.iter().take(selected).sum();
        let target_w = widths.get(selected).copied().unwrap_or(0.0);

        let indicator = transition(
            (self.id.clone(), "indicator"),
            Bounds::new(
                point(px(TRACK_PADDING + target_x), px(TRACK_PADDING)),
                size(px(target_w), track_height - px(TRACK_PADDING * 2.0)),
            ),
            control_transition(cx),
            window,
            cx,
        );

        let theme = cx.theme();
        let track_bg = theme.tab_bar_segmented;
        let indicator_bg = theme.tab_active;
        let indicator_border = theme.border;
        let active_fg = theme.tab_active_foreground;
        let idle_fg = theme.tab_foreground;
        let radius = theme.radius;
        let focus_border = theme.primary;
        let inner_radius = (f32::from(radius) - TRACK_PADDING).max(0.0);
        let total_width: f32 = widths.iter().sum::<f32>() + TRACK_PADDING * 2.0;

        let segments = self.labels.into_iter().enumerate().map(|(ix, label)| {
            let is_selected = ix == selected;
            let on_click = self.on_click.clone();
            let icon = self.icons.get(ix).copied();
            div()
                .id(SharedString::from(format!("{}-seg-{ix}", self.id)))
                .w(px(widths[ix]))
                .h_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .gap_1()
                .text_sm()
                .whitespace_nowrap()
                .text_color(if is_selected { active_fg } else { idle_fg })
                .rounded(px(inner_radius))
                .border_1()
                .border_color(focus_border.opacity(0.))
                .cursor_pointer()
                .focusable()
                .tab_stop(true)
                .focus_visible(|style| style.border_color(focus_border))
                .hover(|style| style.text_color(active_fg))
                .role(gpui_kit::Role::Tab)
                .aria_label(label.clone())
                .aria_selected(is_selected)
                .on_click(move |_, window, cx| {
                    if let Some(handler) = &on_click {
                        handler(&ix, window, cx);
                    }
                })
                .when_some(icon, |segment, icon| {
                    segment.child(Icon::new(icon).size(text_px).flex_shrink_0())
                })
                .child(label)
        });

        div()
            .id(self.id.clone())
            .role(gpui_kit::Role::TabList)
            .relative()
            .flex_shrink_0()
            .w(px(total_width))
            .h(rems(TRACK_HEIGHT_REM))
            .p(px(TRACK_PADDING))
            .rounded(radius)
            .bg(track_bg)
            .child(
                div()
                    .absolute()
                    .top(indicator.origin.y)
                    .left(indicator.origin.x)
                    .w(indicator.size.width)
                    .h(indicator.size.height)
                    .rounded(px(inner_radius))
                    .border_1()
                    .border_color(indicator_border)
                    .bg(indicator_bg),
            )
            .child(h_flex().relative().size_full().children(segments))
    }
}
