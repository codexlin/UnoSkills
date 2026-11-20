use gpui_kit::component::{ActiveTheme, Sizable, tag::Tag};
use gpui_kit::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder,
};

pub fn format_install_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Formats an estimated token count into a compact string (`~940` or `~5.5k`).
pub fn format_tokens_short(count: u32) -> String {
    if count >= 1000 {
        format!("~{:.1}k", count as f32 / 1000.0)
    } else {
        format!("~{count}")
    }
}

/// Quiet monospace token-count indicator (e.g. `~1.2k` or `~940`) that stays
/// neutral unless the skill exceeds the 4,000-token budget threshold.
#[derive(IntoElement)]
pub struct TokenBadge {
    label: SharedString,
    over_budget: bool,
}

impl TokenBadge {
    /// Renders compact `~1.2k` or `~940` in monospace; highlights in warning
    /// tone when `count > 4000`.
    pub fn tokens(count: u32) -> Self {
        Self {
            label: SharedString::from(format_tokens_short(count)),
            over_budget: count > 4000,
        }
    }
}

impl RenderOnce for TokenBadge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = if self.over_budget {
            theme.warning
        } else {
            theme.muted_foreground
        };
        div()
            .font_family(theme.mono_font_family.clone())
            .text_xs()
            .text_color(color)
            .child(self.label)
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
