import { colorScheme } from "nsh";
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

type Colors = Pick<Theme, "background" | "surface" | "foreground" | "muted" | "border" | "primary" | "onPrimary" | "accent"> & {
  chrome: NonNullable<Theme["chrome"]>;
};

// The rail, canvas and group box retain their legacy contrast in all six palettes.
const colors: Record<Palette, { light: Colors; dark: Colors }> = {
  unoskills: {
    dark: {
      background: "#14171d", surface: "#1d222c", foreground: "#e8edf5", muted: "#a9b3c4",
      border: "#343d4e", primary: "#a6bcff", onPrimary: "#14223e", accent: "#2b354a",
      chrome: { rail: "#181c23", railSelection: "#1d222c", hover: "#262d3a",
        segmented: "#151922", primaryHover: "#b6c8ff", primaryActive: "#95adfa", inputBorder: "#465166" },
    },
    light: {
      background: "#fcfcfd", surface: "#ffffff", foreground: "#182232", muted: "#526071",
      border: "#dce2eb", primary: "#315ac5", onPrimary: "#ffffff", accent: "#e9eefc",
      chrome: { rail: "#f3f5f9", railSelection: "#eef2fa", hover: "#eef2fa",
        segmented: "#eef1f6", primaryHover: "#294ead", primaryActive: "#234496", inputBorder: "#dce2eb" },
    },
  },
  claude: {
    dark: {
      background: "#262624", surface: "#2e2e2b", foreground: "#faf9f5", muted: "#b7b5a9",
      border: "#3e3e38", primary: "#d97757", onPrimary: "#ffffff", accent: "#332722",
      chrome: { rail: "#1f1e1d", railSelection: "#2e2e2b", hover: "#373632",
        segmented: "#181816", primaryHover: "#e38668", primaryActive: "#c96442", inputBorder: "#52514a" },
    },
    light: {
      background: "#faf9f5", surface: "#ffffff", foreground: "#141413", muted: "#65645e",
      border: "#dad9d4", primary: "#c96442", onPrimary: "#ffffff", accent: "#f5e8e2",
      chrome: { rail: "#f5f4ee", railSelection: "#e9e6dc", hover: "#e9e6dc",
        segmented: "#e6e2d6", primaryHover: "#b85737", primaryActive: "#a34b2e", inputBorder: "#dad9d4" },
    },
  },
  vercel: {
    dark: {
      background: "#0a0a0a", surface: "#121212", foreground: "#ffffff", muted: "#a4a4a4",
      border: "#242424", primary: "#ffffff", onPrimary: "#000000", accent: "#1a1a1a",
      chrome: { rail: "#050505", railSelection: "#121212", hover: "#1f1f1f",
        segmented: "#181818", primaryHover: "#e5e5e5", primaryActive: "#cccccc", inputBorder: "#333333" },
    },
    light: {
      background: "#fcfcfc", surface: "#ffffff", foreground: "#000000", muted: "#525252",
      border: "#e4e4e4", primary: "#000000", onPrimary: "#ffffff", accent: "#f0f0f0",
      chrome: { rail: "#f5f5f5", railSelection: "#ebebeb", hover: "#ebebeb",
        segmented: "#e5e5e5", primaryHover: "#262626", primaryActive: "#404040", inputBorder: "#e4e4e4" },
    },
  },
  supabase: {
    dark: {
      background: "#171717", surface: "#1f1f1f", foreground: "#e2e8f0", muted: "#a2a2a2",
      border: "#292929", primary: "#24b47e", onPrimary: "#ffffff", accent: "#162620",
      chrome: { rail: "#121212", railSelection: "#1f1f1f", hover: "#282828",
        segmented: "#0e0e0e", primaryHover: "#3ecf8e", primaryActive: "#169c68", inputBorder: "#333333" },
    },
    light: {
      background: "#fcfcfc", surface: "#ffffff", foreground: "#171717", muted: "#626262",
      border: "#dfdfdf", primary: "#169c68", onPrimary: "#ffffff", accent: "#e6f7f0",
      chrome: { rail: "#f3f4f3", railSelection: "#ededed", hover: "#ededed",
        segmented: "#e6e8e6", primaryHover: "#128356", primaryActive: "#0e6b46", inputBorder: "#dfdfdf" },
    },
  },
  catppuccin: {
    dark: {
      background: "#181825", surface: "#1e1e2e", foreground: "#cdd6f4", muted: "#a6adc8",
      border: "#313244", primary: "#cba6f7", onPrimary: "#1e1e2e", accent: "#26233a",
      chrome: { rail: "#11111b", railSelection: "#1e1e2e", hover: "#292c3c",
        segmented: "#0e0e16", primaryHover: "#d7b8f9", primaryActive: "#b48ef0", inputBorder: "#45475a" },
    },
    light: {
      background: "#eff1f5", surface: "#ffffff", foreground: "#4c4f69", muted: "#55586e",
      border: "#bcc0cc", primary: "#8839ef", onPrimary: "#ffffff", accent: "#eae0fc",
      chrome: { rail: "#e6e9ef", railSelection: "#ccd0da", hover: "#ccd0da",
        segmented: "#dce0e8", primaryHover: "#7626de", primaryActive: "#641bc4", inputBorder: "#bcc0cc" },
    },
  },
  ocean: {
    dark: {
      background: "#0f172a", surface: "#1e293b", foreground: "#e2e8f0", muted: "#94a3b8",
      border: "#334155", primary: "#34d399", onPrimary: "#0f172a", accent: "#162e38",
      chrome: { rail: "#0b1120", railSelection: "#1e293b", hover: "#2d3748",
        segmented: "#080c17", primaryHover: "#4ade80", primaryActive: "#10b981", inputBorder: "#475569" },
    },
    light: {
      background: "#f0f8ff", surface: "#ffffff", foreground: "#1e293b", muted: "#56667c",
      border: "#cbd5e1", primary: "#16a34a", onPrimary: "#ffffff", accent: "#dcfce7",
      chrome: { rail: "#e0f2fe", railSelection: "#e0f2fe", hover: "#e0f2fe",
        segmented: "#dce7f3", primaryHover: "#15803d", primaryActive: "#166534", inputBorder: "#cbd5e1" },
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
    danger: scheme === "dark" ? "#ef4444" : "#d93838",
    warning: scheme === "dark" ? "#d99b26" : "#c47f17",
    success: scheme === "dark" ? "#2ea06b" : "#168a53",
    fontSize: fontSize >= 12 && fontSize <= 18 ? fontSize : 14,
    radius: radius >= 0 && radius <= 16 ? radius : 8,
    fontFamily: prefs.ui_font === "auto" || !prefs.ui_font.trim() ? languageFonts[prefs.language] : prefs.ui_font.trim(),
    monoFamily: prefs.mono_font === "auto" || !prefs.mono_font.trim()
      ? "Maple Mono NF CN, JetBrains Mono, SF Mono, Cascadia Mono, Ubuntu Sans Mono, Ubuntu Mono, DejaVu Sans Mono, Menlo, Consolas, monospace"
      : prefs.mono_font.trim(),
  };
}
