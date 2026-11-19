mod assets;
mod components;
mod features;
mod fonts;
mod i18n;
mod shell;
mod state;

use gpui_kit::{AppContext, Bounds, WindowBounds, WindowOptions, component::TitleBar, px, size};
use shell::UnoSkillsShell;

fn main() {
    gpui_kit::application()
        .with_assets(assets::UnoAssets)
        .run(|cx| {
            gpui_kit::init(cx);

            let bounds = Bounds::centered(None, size(px(1240.), px(780.)), cx);
            let mut titlebar = TitleBar::title_bar_options();
            titlebar.title = Some("UnoSkills".into());
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(titlebar),
                app_id: Some("dev.unoskills.UnoSkills".to_string()),
                ..Default::default()
            };

            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| UnoSkillsShell::new(window, cx))
            })
            .expect("Failed to open UnoSkills window");
        });
}
