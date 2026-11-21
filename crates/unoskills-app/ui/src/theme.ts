import { colorScheme, setTheme } from "nsh";
import { Scrollbar } from "nsh/base";
import unoSkills from "../themes/unoskills.json";
import type { Locale, Theme } from "./shared";

export interface Preferences {
  language: Locale;
  theme: "system" | "light" | "dark";
  accent_palette: string;
  font_size: string;
  corner_radius: string;
  ui_font: string;
  mono_font: string;
  sidebar_collapsed: string;
  deploy_mode: "symlink" | "copy";
  proxy: string;
  token: string;
  root_dir: string;
  version: string;
}

export const defaults: Preferences = {
  language: "zh-CN", theme: "system", accent_palette: "unoskills", font_size: "14",
  corner_radius: "8", ui_font: "auto", mono_font: "auto", sidebar_collapsed: "false",
  deploy_mode: "symlink", proxy: "", token: "", root_dir: "", version: "",
};

export const palettes = ["claude", "vercel", "supabase", "catppuccin", "ocean", "unoskills"] as const;
type Palette = typeof palettes[number];

type Colors = Pick<Theme, "background" | "surface" | "foreground" | "muted" | "border" | "primary" | "onPrimary" | "accent"
  | "sidebar" | "destructive"> & {
  chrome: NonNullable<Theme["chrome"]>;
};

function unoSkillsColors(scheme: "light" | "dark"): Colors {
  const tokens = unoSkills.cssVars[scheme];
  const mix = (first: string, weight: number, second: string) =>
    `color-mix(in oklab, ${first} ${weight}%, ${second})`;
  return {
    background: tokens.background, surface: tokens.card, foreground: tokens.foreground,
    muted: tokens["muted-foreground"], border: tokens.border, primary: tokens.primary,
    // The supplied white label has 2.8:1 contrast on the dark orange.
    onPrimary: scheme === "dark" ? tokens.background : tokens["primary-foreground"],
    accent: tokens.accent,
    destructive: { background: tokens.destructive, foreground: tokens["destructive-foreground"] },
    sidebar: {
      foreground: tokens["sidebar-foreground"], selected: tokens["sidebar-accent"],
      onSelected: tokens["sidebar-accent-foreground"], border: tokens["sidebar-border"], ring: tokens["sidebar-ring"],
    },
    chrome: {
      rail: tokens.sidebar, hover: mix(tokens.accent, 56, tokens.card), segmented: tokens.muted,
      primaryHover: mix(tokens.primary, 92, tokens.foreground),
      primaryActive: mix(tokens.primary, 87, scheme === "dark" ? tokens.background : tokens.foreground),
      inputBorder: tokens.input,
    },
  };
}

const colors: Record<Palette, { light: Colors; dark: Colors }> = {
  unoskills: { light: unoSkillsColors("light"), dark: unoSkillsColors("dark") },
  claude: {
    dark: {
      background: "#262624", surface: "#2e2e2b", foreground: "#faf9f5", muted: "#b7b5a9",
      border: "#3e3e38", primary: "#d97757", onPrimary: "#141413", accent: "#332722",
      chrome: { rail: "#1f1e1d", hover: "#373632",
        segmented: "#181816", primaryHover: "#e38668", primaryActive: "#c96442", inputBorder: "#52514a" },
    },
    light: {
      background: "#faf9f5", surface: "#ffffff", foreground: "#141413", muted: "#65645e",
      border: "#dad9d4", primary: "#a34b2e", onPrimary: "#ffffff", accent: "#f5e8e2",
      chrome: { rail: "#f5f4ee", hover: "#e9e6dc",
        segmented: "#e6e2d6", primaryHover: "#944125", primaryActive: "#81371f", inputBorder: "#dad9d4" },
    },
  },
  vercel: {
    dark: {
      background: "#0a0a0a", surface: "#121212", foreground: "#ffffff", muted: "#a4a4a4",
      border: "#242424", primary: "#ffffff", onPrimary: "#000000", accent: "#1a1a1a",
      chrome: { rail: "#050505", hover: "#1f1f1f",
        segmented: "#181818", primaryHover: "#e5e5e5", primaryActive: "#cccccc", inputBorder: "#333333" },
    },
    light: {
      background: "#fcfcfc", surface: "#ffffff", foreground: "#000000", muted: "#525252",
      border: "#e4e4e4", primary: "#000000", onPrimary: "#ffffff", accent: "#f0f0f0",
      chrome: { rail: "#f5f5f5", hover: "#ebebeb",
        segmented: "#e5e5e5", primaryHover: "#262626", primaryActive: "#404040", inputBorder: "#e4e4e4" },
    },
  },
  supabase: {
    dark: {
      background: "#171717", surface: "#1f1f1f", foreground: "#e2e8f0", muted: "#a2a2a2",
      border: "#292929", primary: "#24b47e", onPrimary: "#081911", accent: "#162620",
      chrome: { rail: "#121212", hover: "#282828",
        segmented: "#0e0e0e", primaryHover: "#3ecf8e", primaryActive: "#169c68", inputBorder: "#333333" },
    },
    light: {
      background: "#fcfcfc", surface: "#ffffff", foreground: "#171717", muted: "#626262",
      border: "#dfdfdf", primary: "#0e754d", onPrimary: "#ffffff", accent: "#e6f7f0",
      chrome: { rail: "#f3f4f3", hover: "#ededed",
        segmented: "#e6e8e6", primaryHover: "#0d6744", primaryActive: "#0b593b", inputBorder: "#dfdfdf" },
    },
  },
  catppuccin: {
    dark: {
      background: "#181825", surface: "#1e1e2e", foreground: "#cdd6f4", muted: "#a6adc8",
      border: "#313244", primary: "#cba6f7", onPrimary: "#1e1e2e", accent: "#26233a",
      chrome: { rail: "#11111b", hover: "#292c3c",
        segmented: "#0e0e16", primaryHover: "#d7b8f9", primaryActive: "#b48ef0", inputBorder: "#45475a" },
    },
    light: {
      background: "#eff1f5", surface: "#ffffff", foreground: "#4c4f69", muted: "#55586e",
      border: "#bcc0cc", primary: "#8839ef", onPrimary: "#ffffff", accent: "#eae0fc",
      chrome: { rail: "#e6e9ef", hover: "#ccd0da",
        segmented: "#dce0e8", primaryHover: "#7626de", primaryActive: "#641bc4", inputBorder: "#bcc0cc" },
    },
  },
  ocean: {
    dark: {
      background: "#0f172a", surface: "#1e293b", foreground: "#e2e8f0", muted: "#94a3b8",
      border: "#334155", primary: "#34d399", onPrimary: "#0f172a", accent: "#162e38",
      chrome: { rail: "#0b1120", hover: "#2d3748",
        segmented: "#080c17", primaryHover: "#4ade80", primaryActive: "#10b981", inputBorder: "#475569" },
    },
    light: {
      background: "#f0f8ff", surface: "#ffffff", foreground: "#1e293b", muted: "#56667c",
      border: "#cbd5e1", primary: "#13743b", onPrimary: "#ffffff", accent: "#dcfce7",
      chrome: { rail: "#e0f2fe", hover: "#e0f2fe",
        segmented: "#dce7f3", primaryHover: "#116535", primaryActive: "#0e542b", inputBorder: "#cbd5e1" },
    },
  },
};

const languageFonts: Record<Locale, string> = {
  "en": "Inter, SF Pro Text, Segoe UI, Ubuntu, Ubuntu Sans, Cantarell, Noto Sans, Noto Sans CJK SC, DejaVu Sans, system-ui",
  "zh-CN": "PingFang SC, Hiragino Sans GB, Microsoft YaHei UI, Microsoft YaHei, Noto Sans CJK SC, Source Han Sans SC, Noto Sans CJK HK, WenQuanYi Micro Hei, Inter, Ubuntu, Noto Sans, DejaVu Sans, system-ui",
  "ja": "Hiragino Sans, Hiragino Kaku Gothic ProN, Yu Gothic UI, Meiryo, Noto Sans CJK JP, Source Han Sans JP, Inter, Ubuntu, Noto Sans, DejaVu Sans, system-ui",
};

const aliases: Record<string, Palette> = {
  terracotta: "claude", amber: "claude", mono: "vercel", default: "vercel",
  jade: "supabase", emerald: "supabase", indigo: "catppuccin", rose: "catppuccin",
  "ocean-breeze": "ocean", cobalt: "ocean",
};

export function paletteFor(value: string): Palette {
  const normalized = value.trim().toLowerCase();
  return palettes.find((palette) => palette === normalized) ?? aliases[normalized] ?? "unoskills";
}

export function makeTheme(prefs: Preferences): Theme {
  const scheme = prefs.theme === "system" ? colorScheme() : prefs.theme;
  const palette = colors[paletteFor(prefs.accent_palette)][scheme];
  const fontSize = Number(prefs.font_size);
  const radius = Number(prefs.corner_radius);
  return {
    ...palette,
    modalBackdrop: "#00000072",
    danger: scheme === "dark" ? "#fda4af" : palette.destructive?.background ?? "#a32136",
    warning: scheme === "dark" ? "#fbc879" : "#844a04",
    success: scheme === "dark" ? "#8adcb9" : "#176147",
    fontSize: fontSize >= 12 && fontSize <= 18 ? fontSize : 14,
    radius: radius >= 0 && radius <= 16 ? radius : 8,
    fontFamily: prefs.ui_font === "auto" || !prefs.ui_font.trim() ? languageFonts[prefs.language] : prefs.ui_font.trim(),
    monoFamily: prefs.mono_font === "auto" || !prefs.mono_font.trim()
      ? "Maple Mono NF CN, JetBrains Mono, SF Mono, Cascadia Mono, Ubuntu Sans Mono, Ubuntu Mono, DejaVu Sans Mono, Menlo, Consolas, monospace"
      : prefs.mono_font.trim(),
  };
}

export function applyNativeTheme(theme: Theme) {
  const typography = {
    "radius": `${theme.radius}px`,
    "font-sans": theme.fontFamily ?? "sans-serif",
    "font-mono": theme.monoFamily ?? "monospace",
  };
  setTheme({ cssVars: theme.sidebar ? {
    light: unoSkills.cssVars.light,
    dark: { ...unoSkills.cssVars.dark, "primary-foreground": colors.unoskills.dark.onPrimary },
    theme: typography,
  } : { theme: {
    "background": theme.background,
    "foreground": theme.foreground,
    "card": theme.surface,
    "card-foreground": theme.foreground,
    "popover": theme.surface,
    "popover-foreground": theme.foreground,
    "muted": theme.chrome?.hover ?? theme.accent,
    "muted-foreground": theme.muted,
    "primary": theme.primary,
    "primary-foreground": theme.onPrimary,
    "secondary": theme.accent,
    "secondary-foreground": theme.foreground,
    "accent": theme.accent,
    "accent-foreground": theme.foreground,
    "border": theme.border,
    "input": theme.chrome?.inputBorder ?? theme.border,
    "ring": theme.primary,
    "destructive": theme.danger,
    "destructive-foreground": theme.background,
    ...typography,
  } } });
  Scrollbar.setTheme({
    mode: "hover",
    vertical: {
      class: "absolute inset-y-0 right-0 w-4 items-end p-1",
      thumbClass: "w-1.5 rounded-full bg-muted-foreground opacity-60 hover:w-2 hover:opacity-100",
    },
    horizontal: {
      class: "absolute inset-x-0 bottom-0 h-4 items-end p-1",
      thumbClass: "h-1.5 rounded-full bg-muted-foreground opacity-60 hover:h-2 hover:opacity-100",
    },
  });
}
