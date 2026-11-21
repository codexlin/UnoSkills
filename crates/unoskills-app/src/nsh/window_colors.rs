use std::cell::OnceCell;

use nsh_platform_linux::gtk4::{self as gtk, gdk, glib};
use serde::Deserialize;

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
            provider.load_from_string(&format!(
                "windowcontrols button, windowcontrols button image {{ color: {foreground}; }}
                 windowcontrols button:hover {{ background-color: {hover}; }}
                 windowcontrols button:active {{ background-color: {active}; }}"
            ));
        });
    });
}
