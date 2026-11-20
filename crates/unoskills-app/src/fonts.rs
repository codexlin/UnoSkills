use std::collections::BTreeSet;

use gpui_kit::component::Theme;
use gpui_kit::{App, Hsla, SharedString, px, rgb};

use crate::i18n::Lang;

pub const FONT_AUTO: &str = "auto";
pub const DEFAULT_FONT_SIZE: f32 = 14.0;
pub const FONT_SIZE_CHOICES: [f32; 4] = [13.0, 14.0, 15.0, 16.0];
pub const DEFAULT_CORNER_RADIUS: f32 = 8.0;
pub const RADIUS_CHOICES: [f32; 5] = [0.0, 4.0, 6.0, 8.0, 12.0];

/// Theme presets for surfaces, controls, selection, and typography contrast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccentPalette {
    Claude,
    Vercel,
    Supabase,
    Catppuccin,
    Ocean,
    #[default]
    UnoSkills,
}

impl AccentPalette {
    pub const ALL: [Self; 6] = [
        Self::Claude,
        Self::Vercel,
        Self::Supabase,
        Self::Catppuccin,
        Self::Ocean,
        Self::UnoSkills,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Vercel => "vercel",
            Self::Supabase => "supabase",
            Self::Catppuccin => "catppuccin",
            Self::Ocean => "ocean",
            Self::UnoSkills => "unoskills",
        }
    }

    pub fn from_code(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "claude" | "terracotta" | "amber" => Self::Claude,
            "vercel" | "mono" | "default" => Self::Vercel,
            "supabase" | "jade" | "emerald" => Self::Supabase,
            "catppuccin" | "indigo" | "rose" => Self::Catppuccin,
            "ocean" | "ocean-breeze" | "cobalt" => Self::Ocean,
            "unoskills" => Self::UnoSkills,
            _ => Self::default(),
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Claude => 0,
            Self::Vercel => 1,
            Self::Supabase => 2,
            Self::Catppuccin => 3,
            Self::Ocean => 4,
            Self::UnoSkills => 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontSettings {
    /// `"auto"` to follow UI language, or an explicit installed font family name.
    pub ui_font: String,
    /// `"auto"` to pick the best installed monospace font, or an explicit family name.
    pub mono_font: String,
    pub font_size: f32,
    pub palette: AccentPalette,
    pub corner_radius: f32,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            ui_font: FONT_AUTO.to_string(),
            mono_font: FONT_AUTO.to_string(),
            font_size: DEFAULT_FONT_SIZE,
            palette: AccentPalette::default(),
            corner_radius: DEFAULT_CORNER_RADIUS,
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
            "Inter",
            "Ubuntu",
            "Noto Sans",
            "DejaVu Sans",
        ],
        Lang::Ja => &[
            "Hiragino Sans",
            "Hiragino Kaku Gothic ProN",
            "Yu Gothic UI",
            "Meiryo",
            "Noto Sans CJK JP",
            "Source Han Sans JP",
            "Inter",
            "Ubuntu",
            "Noto Sans",
            "DejaVu Sans",
        ],
        Lang::En => &[
            "Inter",
            "SF Pro Text",
            "Segoe UI",
            "Ubuntu",
            "Ubuntu Sans",
            "Cantarell",
            "Noto Sans",
            "Noto Sans CJK SC",
            "DejaVu Sans",
        ],
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

#[inline]
fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Applies font families, base size, corner radius, and authentic tweakcn (shadcn/ui)
/// theme presets globally via `Theme::update`.
pub fn apply_fonts_to_theme(settings: &FontSettings, lang: Lang, cx: &mut App) {
    let installed = installed_font_families(cx);
    let ui_family = resolve_ui_font(&settings.ui_font, lang, &installed);
    let mono_family = resolve_mono_font(&settings.mono_font, &installed);
    // `settings.font_size` represents the UI body/control text size (`.text_sm()` = 0.875rem).
    // Since gpui-component binds `window.set_rem_size(cx.theme().font_size)`, we map
    // `ui_body_size` (default 14px) to `1rem` = `ui_body_size / 0.875` (default 16px),
    // so `.text_xs()` = 12px, `.text_sm()` = 14px, `.text_base()` = 16px,
    // `.text_lg()` = 18px, and `.text_xl()` = 20px.
    let ui_body_size = settings.font_size.clamp(12.0, 18.0);
    let rem_base = (ui_body_size / 0.875).round();
    let r = settings.corner_radius.clamp(0.0, 16.0);
    let r_lg = if r == 0.0 { 0.0 } else { (r + 2.0).min(16.0) };

    Theme::update(cx, |theme| {
        theme.font_family = ui_family;
        theme.mono_font_family = mono_family;
        theme.font_size = px(rem_base);
        theme.mono_font_size = px((ui_body_size - 1.0).max(12.0));
        theme.radius = px(r);
        theme.radius_lg = px(r_lg);

        if theme.is_dark() {
            let (
                l0_rail,
                l1_canvas,
                l2_card,
                l3_active,
                segmented_bg,
                border,
                input_border,
                fg,
                muted_fg,
                brand,
                brand_hover,
                brand_active,
                brand_fg,
                switch_bg,
                list_active_bg,
            ) = match settings.palette {
                AccentPalette::UnoSkills => (
                    c(0x181C23),
                    c(0x14171D),
                    c(0x1D222C),
                    c(0x262D3A),
                    c(0x151922),
                    c(0x343D4E),
                    c(0x465166),
                    c(0xE8EDF5),
                    c(0xA9B3C4),
                    c(0xA6BCFF),
                    c(0xB6C8FF),
                    c(0x95ADFA),
                    c(0x14223E),
                    c(0x465166),
                    c(0x2B354A),
                ),
                // tweakcn "Claude" preset (Anthropic warm stone & terracotta)
                AccentPalette::Claude => (
                    c(0x1F1E1D),
                    c(0x262624),
                    c(0x2E2E2B),
                    c(0x373632),
                    c(0x181816),
                    c(0x3E3E38),
                    c(0x52514A),
                    c(0xFAF9F5),
                    c(0xB7B5A9),
                    c(0xD97757),
                    c(0xE38668),
                    c(0xC96442),
                    c(0xFFFFFF),
                    c(0x46453F),
                    c(0x332722),
                ),
                // tweakcn "Vercel" preset (OLED monochrome & pure white primary)
                AccentPalette::Vercel => (
                    c(0x050505),
                    c(0x0A0A0A),
                    c(0x121212),
                    c(0x1F1F1F),
                    c(0x181818),
                    c(0x242424),
                    c(0x333333),
                    c(0xFFFFFF),
                    c(0xA4A4A4),
                    c(0xFFFFFF),
                    c(0xE5E5E5),
                    c(0xCCCCCC),
                    c(0x000000),
                    c(0x525252),
                    c(0x1A1A1A),
                ),
                // tweakcn "Supabase" preset (Deep carbon & emerald green)
                AccentPalette::Supabase => (
                    c(0x121212),
                    c(0x171717),
                    c(0x1F1F1F),
                    c(0x282828),
                    c(0x0E0E0E),
                    c(0x292929),
                    c(0x333333),
                    c(0xE2E8F0),
                    c(0xA2A2A2),
                    c(0x24B47E),
                    c(0x3ECF8E),
                    c(0x169C68),
                    c(0xFFFFFF),
                    c(0x383838),
                    c(0x162620),
                ),
                // tweakcn "Catppuccin" preset (Mocha Crust/Mantle/Base & Mauve)
                AccentPalette::Catppuccin => (
                    c(0x11111B),
                    c(0x181825),
                    c(0x1E1E2E),
                    c(0x292C3C),
                    c(0x0E0E16),
                    c(0x313244),
                    c(0x45475A),
                    c(0xCDD6F4),
                    c(0xA6ADC8),
                    c(0xCBA6F7),
                    c(0xD7B8F9),
                    c(0xB48EF0),
                    c(0x1E1E2E),
                    c(0x585B70),
                    c(0x26233A),
                ),
                // tweakcn "Ocean Breeze" preset (Slate navy & mint emerald)
                AccentPalette::Ocean => (
                    c(0x0B1120),
                    c(0x0F172A),
                    c(0x1E293B),
                    c(0x2D3748),
                    c(0x080C17),
                    c(0x334155),
                    c(0x475569),
                    c(0xE2E8F0),
                    c(0x94A3B8),
                    c(0x34D399),
                    c(0x4ADE80),
                    c(0x10B981),
                    c(0x0F172A),
                    c(0x475569),
                    c(0x162E38),
                ),
            };

            let success = c(0x2EA06B);
            let warning = c(0xD99B26);
            let danger = c(0xEF4444);
            let white = c(0xFFFFFF);

            theme.background = l1_canvas;
            theme.foreground = fg;
            theme.muted_foreground = muted_fg;
            theme.border = border;
            theme.input = input_border;

            theme.title_bar = l0_rail;
            theme.title_bar_border = border;
            theme.sidebar = l0_rail;
            theme.sidebar_border = border;
            theme.sidebar_foreground = fg;
            theme.sidebar_accent = l2_card;
            theme.sidebar_accent_foreground = fg;
            theme.sidebar_primary = brand;
            theme.sidebar_primary_foreground = brand_fg;

            theme.group_box = l2_card;
            theme.group_box_foreground = fg;
            theme.popover = l2_card;
            theme.popover_foreground = fg;
            theme.colors.list = l1_canvas;
            theme.list_hover = l3_active;
            theme.list_active = list_active_bg;
            theme.list_active_border = brand;

            theme.secondary = l2_card;
            theme.secondary_hover = l3_active;
            theme.secondary_active = border;
            theme.secondary_foreground = fg;
            theme.muted = segmented_bg;
            theme.accent = l3_active;
            theme.accent_foreground = fg;

            theme.tab_bar = segmented_bg;
            theme.tab_bar_segmented = segmented_bg;
            theme.tab_active = l2_card;
            theme.tab_active_foreground = fg;
            theme.tab_foreground = muted_fg;

            theme.primary = brand;
            theme.primary_hover = brand_hover;
            theme.primary_active = brand_active;
            theme.primary_foreground = brand_fg;
            theme.button_primary = brand;
            theme.button_primary_hover = brand_hover;
            theme.button_primary_active = brand_active;
            theme.button_primary_foreground = brand_fg;

            theme.button = l2_card;
            theme.button_hover = l3_active;
            theme.button_active = border;
            theme.button_foreground = fg;
            theme.button_secondary = l2_card;
            theme.button_secondary_hover = l3_active;
            theme.button_secondary_active = border;
            theme.button_secondary_foreground = fg;

            theme.success = success;
            theme.success_foreground = white;
            theme.warning = warning;
            theme.warning_foreground = c(0x14161A);
            theme.danger = danger;
            theme.danger_foreground = white;
            theme.info = brand;
            theme.info_foreground = brand_fg;

            theme.switch = switch_bg;
            theme.switch_thumb = brand_fg;
            theme.ring = brand;
            theme.selection = brand.opacity(0.3);
        } else {
            let (
                l0_rail,
                l1_canvas,
                l3_active,
                segmented_bg,
                border,
                fg,
                muted_fg,
                brand,
                brand_hover,
                brand_active,
                brand_fg,
                list_active_bg,
            ) = match settings.palette {
                AccentPalette::UnoSkills => (
                    c(0xF3F5F9),
                    c(0xFCFCFD),
                    c(0xEEF2FA),
                    c(0xEEF1F6),
                    c(0xDCE2EB),
                    c(0x182232),
                    c(0x526071),
                    c(0x315AC5),
                    c(0x294EAD),
                    c(0x234496),
                    c(0xFFFFFF),
                    c(0xE9EEFC),
                ),
                // tweakcn "Claude" Light
                AccentPalette::Claude => (
                    c(0xF5F4EE),
                    c(0xFAF9F5),
                    c(0xE9E6DC),
                    c(0xE6E2D6),
                    c(0xDAD9D4),
                    c(0x141413),
                    c(0x83827D),
                    c(0xC96442),
                    c(0xB85737),
                    c(0xA34B2E),
                    c(0xFFFFFF),
                    c(0xF5E8E2),
                ),
                // tweakcn "Vercel" Light
                AccentPalette::Vercel => (
                    c(0xF5F5F5),
                    c(0xFCFCFC),
                    c(0xEBEBEB),
                    c(0xE5E5E5),
                    c(0xE4E4E4),
                    c(0x000000),
                    c(0x525252),
                    c(0x000000),
                    c(0x262626),
                    c(0x404040),
                    c(0xFFFFFF),
                    c(0xF0F0F0),
                ),
                // tweakcn "Supabase" Light
                AccentPalette::Supabase => (
                    c(0xF3F4F3),
                    c(0xFCFCFC),
                    c(0xEDEDED),
                    c(0xE6E8E6),
                    c(0xDFDFDF),
                    c(0x171717),
                    c(0x707070),
                    c(0x169C68),
                    c(0x128356),
                    c(0x0E6B46),
                    c(0xFFFFFF),
                    c(0xE6F7F0),
                ),
                // tweakcn "Catppuccin" Light (Latte)
                AccentPalette::Catppuccin => (
                    c(0xE6E9EF),
                    c(0xEFF1F5),
                    c(0xCCD0DA),
                    c(0xDCE0E8),
                    c(0xBCC0CC),
                    c(0x4C4F69),
                    c(0x6C6F85),
                    c(0x8839EF),
                    c(0x7626DE),
                    c(0x641BC4),
                    c(0xFFFFFF),
                    c(0xEAE0FC),
                ),
                // tweakcn "Ocean Breeze" Light
                AccentPalette::Ocean => (
                    c(0xE0F2FE),
                    c(0xF0F8FF),
                    c(0xE0F2FE),
                    c(0xDCE7F3),
                    c(0xCBD5E1),
                    c(0x1E293B),
                    c(0x64748B),
                    c(0x16A34A),
                    c(0x15803D),
                    c(0x166534),
                    c(0xFFFFFF),
                    c(0xDCFCE7),
                ),
            };
            let l2_card = c(0xFFFFFF);
            let success = c(0x168A53);
            let warning = c(0xC47F17);
            let danger = c(0xD93838);
            let white = c(0xFFFFFF);

            theme.background = l1_canvas;
            theme.foreground = fg;
            theme.muted_foreground = muted_fg;
            theme.border = border;
            theme.input = border;

            theme.title_bar = l0_rail;
            theme.title_bar_border = border;
            theme.sidebar = l0_rail;
            theme.sidebar_border = border;
            theme.sidebar_foreground = fg;
            theme.sidebar_accent = l3_active;
            theme.sidebar_accent_foreground = fg;
            theme.sidebar_primary = brand;
            theme.sidebar_primary_foreground = brand_fg;

            theme.group_box = l2_card;
            theme.group_box_foreground = fg;
            theme.popover = l2_card;
            theme.popover_foreground = fg;
            theme.colors.list = l1_canvas;
            theme.list_hover = l3_active;
            theme.list_active = list_active_bg;
            theme.list_active_border = brand;

            theme.secondary = l3_active;
            theme.secondary_hover = border;
            theme.secondary_active = border;
            theme.secondary_foreground = fg;
            theme.muted = segmented_bg;
            theme.accent = l3_active;
            theme.accent_foreground = fg;

            theme.tab_bar = segmented_bg;
            theme.tab_bar_segmented = segmented_bg;
            theme.tab_active = l2_card;
            theme.tab_active_foreground = fg;
            theme.tab_foreground = muted_fg;

            theme.primary = brand;
            theme.primary_hover = brand_hover;
            theme.primary_active = brand_active;
            theme.primary_foreground = brand_fg;
            theme.button_primary = brand;
            theme.button_primary_hover = brand_hover;
            theme.button_primary_active = brand_active;
            theme.button_primary_foreground = brand_fg;

            theme.button = l2_card;
            theme.button_hover = l3_active;
            theme.button_active = border;
            theme.button_foreground = fg;
            theme.button_secondary = l3_active;
            theme.button_secondary_hover = border;
            theme.button_secondary_active = border;
            theme.button_secondary_foreground = fg;

            theme.success = success;
            theme.success_foreground = white;
            theme.warning = warning;
            theme.warning_foreground = white;
            theme.danger = danger;
            theme.danger_foreground = white;
            theme.info = brand;
            theme.info_foreground = brand_fg;

            theme.switch = border;
            theme.switch_thumb = brand_fg;
            theme.ring = brand;
            theme.selection = brand.opacity(0.25);
        }
    });
    cx.refresh_windows();
}
