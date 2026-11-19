use gpui_kit::component::{Sizable, tag::Tag};
use gpui_kit::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Window, prelude::FluentBuilder,
};

/// Compact token-count chip (e.g. `~1.2k tok`) rendered on `Tag::secondary`.
#[derive(IntoElement)]
pub struct TokenBadge {
    label: SharedString,
}

impl TokenBadge {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
        }
    }

    /// Renders `~{count} tok`.
    pub fn tokens(count: u32) -> Self {
        Self::new(format!("~{count} tok"))
    }
}

impl RenderOnce for TokenBadge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        Tag::secondary().small().child(self.label)
    }
}

/// Semantic tone for [`StatusBadge`].
pub enum BadgeTone {
    Secondary,
    Info,
    Success,
    Warning,
}

/// Reusable status badge built on `gpui_kit::component::tag::Tag` — used for
/// verified/local, already-installed/will-install, and detected states.
#[derive(IntoElement)]
pub struct StatusBadge {
    tone: BadgeTone,
    label: SharedString,
    outline: bool,
}

impl StatusBadge {
    pub fn new(tone: BadgeTone, label: impl Into<SharedString>) -> Self {
        Self {
            tone,
            label: label.into(),
            outline: false,
        }
    }

    pub fn secondary(label: impl Into<SharedString>) -> Self {
        Self::new(BadgeTone::Secondary, label)
    }

    pub fn info(label: impl Into<SharedString>) -> Self {
        Self::new(BadgeTone::Info, label)
    }

    pub fn success(label: impl Into<SharedString>) -> Self {
        Self::new(BadgeTone::Success, label)
    }

    pub fn warning(label: impl Into<SharedString>) -> Self {
        Self::new(BadgeTone::Warning, label)
    }

    /// Use the outlined variant for quiet statuses inside dense list rows.
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }
}

impl RenderOnce for StatusBadge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let tag = match self.tone {
            BadgeTone::Secondary => Tag::secondary(),
            BadgeTone::Info => Tag::info(),
            BadgeTone::Success => Tag::success(),
            BadgeTone::Warning => Tag::warning(),
        };
        tag.when(self.outline, |tag| tag.outline())
            .small()
            .child(self.label)
    }
}
