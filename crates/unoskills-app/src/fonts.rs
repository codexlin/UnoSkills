use std::collections::BTreeSet;

use gpui_kit::component::Theme;
use gpui_kit::{App, SharedString, px};

use crate::i18n::Lang;

pub const FONT_AUTO: &str = "auto";
pub const DEFAULT_FONT_SIZE: f32 = 14.0;
pub const FONT_SIZE_CHOICES: [f32; 4] = [13.0, 14.0, 15.0, 16.0];

#[derive(Debug, Clone, PartialEq)]
pub struct FontSettings {
    /// `"auto"` to follow UI language, or an explicit installed font family name.
    pub ui_font: String,
    /// `"auto"` to pick the best installed monospace font, or an explicit family name.
    pub mono_font: String,
    pub font_size: f32,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            ui_font: FONT_AUTO.to_string(),
            mono_font: FONT_AUTO.to_string(),
            font_size: DEFAULT_FONT_SIZE,
        }
    }
}

/// Enumerates sorted, deduplicated system font family names via GPUI's `TextSystem`.
pub fn installed_font_families(cx: &App) -> Vec<String> {
    let mut set = BTreeSet::new();
    for name in cx.text_system().all_font_names() {
        let trimmed = name.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('.') {
            set.insert(trimmed.to_string());
        }
    }
    set.into_iter().collect()
}

fn pick_first_installed(candidates: &[&str], installed: &[String]) -> Option<SharedString> {
    for &cand in candidates {
        if let Some(found) = installed.iter().find(|f| f.eq_ignore_ascii_case(cand)) {
            return Some(SharedString::from(found.clone()));
        }
    }
    None
}

/// Resolves the UI font family. When `pref == "auto"`, picks the crispest installed
/// font matching the active UI language (`zh-CN`, `ja`, or `en`).
pub fn resolve_ui_font(pref: &str, lang: Lang, installed: &[String]) -> SharedString {
    if pref != FONT_AUTO && !pref.trim().is_empty() {
        return SharedString::from(pref.trim().to_string());
    }
    let candidates: &[&str] = match lang {
        Lang::ZhCn => &[
            "PingFang SC",
            "Hiragino Sans GB",
            "Microsoft YaHei UI",
            "Microsoft YaHei",
            "Noto Sans CJK SC",
            "Source Han Sans SC",
            "Noto Sans CJK HK",
            "WenQuanYi Micro Hei",
        ],
        Lang::Ja => &[
            "Hiragino Sans",
            "Hiragino Kaku Gothic ProN",
            "Yu Gothic UI",
            "Meiryo",
            "Noto Sans CJK JP",
            "Source Han Sans JP",
        ],
        Lang::En => &[],
    };
    pick_first_installed(candidates, installed)
        .unwrap_or_else(|| SharedString::from(".SystemUIFont"))
}

pub fn resolve_mono_font(pref: &str, installed: &[String]) -> SharedString {
    if pref != FONT_AUTO && !pref.trim().is_empty() {
        return SharedString::from(pref.trim().to_string());
    }
    let candidates = &[
        "Maple Mono NF CN",
        "JetBrains Mono",
        "SF Mono",
        "Cascadia Mono",
        "Ubuntu Sans Mono",
        "Ubuntu Mono",
        "DejaVu Sans Mono",
        "Menlo",
        "Consolas",
    ];
    pick_first_installed(candidates, installed)
        .unwrap_or_else(|| SharedString::from("DejaVu Sans Mono"))
}

/// Applies font families and base size globally via `Theme::update`, following
/// https://gpui-kit.com/docs/fonts/.
pub fn apply_fonts_to_theme(settings: &FontSettings, lang: Lang, cx: &mut App) {
    let installed = installed_font_families(cx);
    let ui_family = resolve_ui_font(&settings.ui_font, lang, &installed);
    let mono_family = resolve_mono_font(&settings.mono_font, &installed);
    let base_size = settings.font_size.clamp(12.0, 20.0);

    Theme::update(cx, |theme| {
        theme.font_family = ui_family;
        theme.mono_font_family = mono_family;
        theme.font_size = px(base_size);
        theme.mono_font_size = px((base_size - 1.0).max(11.0));
    });
    cx.refresh_windows();
}
