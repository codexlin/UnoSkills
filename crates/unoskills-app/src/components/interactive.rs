use gpui_kit::base::{TransitionId, transition};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    AnyElement, App, ClickEvent, Div, ElementId, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, SharedString, Stateful, StatefulInteractiveElement,
    StyleRefinement, Styled, Window, prelude::FluentBuilder,
};

use super::motion::control_transition;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// Selectable list/grid card with smooth hover and selection transitions.
///
/// The fill and border follow the target state (`rest` → `hover` → `selected`)
/// through keyed Base transitions, so quickly moving the pointer across a list
/// retargets from the current color instead of snapping.
#[derive(IntoElement)]
pub struct InteractiveCard {
    id: SharedString,
    base: Stateful<Div>,
    selected: bool,
    on_click: Option<ClickHandler>,
    rest: Hsla,
    hover: Hsla,
    active: Hsla,
    border: Hsla,
    border_active: Hsla,
    radius: Pixels,
}

impl InteractiveCard {
    /// `content` is the flex container (`h_flex()` / `v_flex()`) to decorate.
    pub fn new(id: impl Into<SharedString>, content: Div, selected: bool, cx: &App) -> Self {
        let id: SharedString = id.into();
        let theme = cx.theme();
        Self {
            base: content.id(id.clone()),
            id,
            selected,
            on_click: None,
            rest: theme.group_box,
            hover: theme.list_hover,
            active: theme.list_active,
            border: theme.border,
            border_active: theme.primary,
            radius: theme.radius_lg,
        }
    }

    pub fn surface(mut self, background: Hsla, hover_background: Hsla, border: Hsla) -> Self {
        self.rest = background;
        self.hover = hover_background;
        self.border = border;
        self
    }

    pub fn on_click(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(listener));
        self
    }

    /// Exposes the card to assistive technology as a selectable list item.
    pub fn accessible(mut self, label: impl Into<SharedString>) -> Self {
        let selected = self.selected;
        self.base = self
            .base
            .role(gpui_kit::Role::ListItem)
            .aria_label(label)
            .aria_selected(selected);
        self
    }
}

impl ParentElement for InteractiveCard {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl Styled for InteractiveCard {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for InteractiveCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let clickable = self.on_click.is_some();
        let hover_state = window.use_keyed_state(
            ElementId::from(TransitionId::from((self.id.clone(), "hover"))),
            cx,
            |_, _| false,
        );
        let hovered = *hover_state.read(cx);

        let (bg_target, border_target) = if self.selected {
            (self.active, self.border_active)
        } else if hovered {
            (self.hover, self.border)
        } else {
            (self.rest, self.border)
        };
        let policy = control_transition(cx);
        let bg = transition(
            (self.id.clone(), "bg"),
            bg_target,
            policy.clone(),
            window,
            cx,
        );
        let border = transition(
            (self.id.clone(), "border"),
            border_target,
            policy,
            window,
            cx,
        );
        let active = self.active;
        let focus_border = self.border_active;

        self.base
            .rounded(self.radius)
            .border_1()
            .border_color(border)
            .bg(bg)
            .when(clickable, |card| {
                card.cursor_pointer()
                    .focusable()
                    .tab_stop(true)
                    .focus_visible(|style| style.border_color(focus_border))
                    .active(|style| style.bg(active))
            })
            .on_hover(move |is_hovered, window, cx| {
                hover_state.update(cx, |state, cx| {
                    if *state != *is_hovered {
                        *state = *is_hovered;
                        cx.notify();
                        window.refresh();
                    }
                });
            })
            .when_some(self.on_click, |card, listener| {
                card.on_click(move |event, window, cx| listener(event, window, cx))
            })
    }
}
