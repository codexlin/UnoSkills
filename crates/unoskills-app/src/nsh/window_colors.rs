use std::cell::OnceCell;

use nsh_platform_linux::gtk4::{self as gtk, gdk, glib};
use serde::Deserialize;

const BUTTON_WIDTH: u32 = 32;
const BUTTON_SPACING: u32 = 4;
// Three buttons, two gaps and room for GTK's overlay margins.
pub(super) const CONTROLS_INSET: u32 = 3 * BUTTON_WIDTH + 2 * BUTTON_SPACING + 24;

thread_local! {
    static WINDOW_COLORS: OnceCell<gtk::CssProvider> = const { OnceCell::new() };
}

#[derive(Deserialize)]
pub(super) struct TitleBarColors {
    foreground: String,
    hover: String,
    active: String,
}

pub(super) fn apply(colors: TitleBarColors) {
    let TitleBarColors {
        foreground,
        hover,
        active,
    } = colors;
    if ![&foreground, &hover, &active].into_iter().all(|color| {
        color
            .strip_prefix('#')
            .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
    }) {
        return;
    }

    // NSH's GTK window controls are siblings of the themed content view.
    glib::idle_add_once(move || {
        let Some(display) = gdk::Display::default() else {
            return;
        };
        WINDOW_COLORS.with(|colors| {
            let provider = colors.get_or_init(|| {
                let provider = gtk::CssProvider::new();
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
                provider
            });
            // GTK themes give the same controls different sizes and can draw a
            // second hover backdrop on the image. Keep the overlay stable.
            provider.load_from_string(&format!(
                "overlay > windowcontrols {{ border-spacing: {BUTTON_SPACING}px; margin: 0; padding: 0; }}
                 overlay > windowcontrols > button {{
                     min-width: {BUTTON_WIDTH}px; min-height: 28px; margin: 0; padding: 0;
                     border: none; border-radius: 6px; box-shadow: none;
                     color: {foreground}; background: none;
                 }}
                 overlay > windowcontrols > button > image {{
                     min-width: 0; min-height: 0; margin: 0; padding: 0;
                     -gtk-icon-size: 16px; color: inherit; background: none; box-shadow: none;
                 }}
                 overlay > windowcontrols > button:hover {{ background-color: {hover}; }}
                 overlay > windowcontrols > button:active {{ background-color: {active}; }}"
            ));
        });
    });
}
