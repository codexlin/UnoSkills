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

type Colors = Pick<Theme, "background" | "surface" | "foreground" | "muted" | "border" | "primary" | "onPrimary" | "accent">;

const colors: Record<Palette, { light: Colors; dark: Colors }> = {
  unoskills: {
    dark: { background: "#14171d", surface: "#1d222c", foreground: "#e8edf5", muted: "#a9b3c4", border: "#343d4e", primary: "#a6bcff", onPrimary: "#14223e", accent: "#2b354a" },
    light: { background: "#fcfcfd", surface: "#f3f5f9", foreground: "#182232", muted: "#526071", border: "#dce2eb", primary: "#315ac5", onPrimary: "#ffffff", accent: "#e9eefc" },
  },
  claude: {
    dark: { background: "#262624", surface: "#2e2e2b", foreground: "#faf9f5", muted: "#b7b5a9", border: "#3e3e38", primary: "#d97757", onPrimary: "#ffffff", accent: "#332722" },
    light: { background: "#faf9f5", surface: "#f5f4ee", foreground: "#141413", muted: "#65645e", border: "#dad9d4", primary: "#c96442", onPrimary: "#ffffff", accent: "#f5e8e2" },
  },
  vercel: {
    dark: { background: "#0a0a0a", surface: "#121212", foreground: "#ffffff", muted: "#a4a4a4", border: "#242424", primary: "#ffffff", onPrimary: "#000000", accent: "#1a1a1a" },
    light: { background: "#fcfcfc", surface: "#f5f5f5", foreground: "#000000", muted: "#525252", border: "#e4e4e4", primary: "#000000", onPrimary: "#ffffff", accent: "#f0f0f0" },
  },
  supabase: {
    dark: { background: "#171717", surface: "#1f1f1f", foreground: "#e2e8f0", muted: "#a2a2a2", border: "#292929", primary: "#24b47e", onPrimary: "#ffffff", accent: "#162620" },
    light: { background: "#fcfcfc", surface: "#f3f4f3", foreground: "#171717", muted: "#626262", border: "#dfdfdf", primary: "#169c68", onPrimary: "#ffffff", accent: "#e6f7f0" },
  },
  catppuccin: {
    dark: { background: "#181825", surface: "#1e1e2e", foreground: "#cdd6f4", muted: "#a6adc8", border: "#313244", primary: "#cba6f7", onPrimary: "#1e1e2e", accent: "#26233a" },
    light: { background: "#eff1f5", surface: "#e6e9ef", foreground: "#4c4f69", muted: "#55586e", border: "#bcc0cc", primary: "#8839ef", onPrimary: "#ffffff", accent: "#eae0fc" },
  },
  ocean: {
    dark: { background: "#0f172a", surface: "#1e293b", foreground: "#e2e8f0", muted: "#94a3b8", border: "#334155", primary: "#34d399", onPrimary: "#0f172a", accent: "#162e38" },
    light: { background: "#f0f8ff", surface: "#e0f2fe", foreground: "#1e293b", muted: "#56667c", border: "#cbd5e1", primary: "#16a34a", onPrimary: "#ffffff", accent: "#dcfce7" },
  },
};

const languageFonts: Record<Locale, string> = {
  "en": "Inter, Segoe UI, Ubuntu, Noto Sans, sans-serif",
  "zh-CN": "PingFang SC, Microsoft YaHei UI, Noto Sans CJK SC, Source Han Sans SC, sans-serif",
  "ja": "Hiragino Sans, Yu Gothic UI, Meiryo, Noto Sans CJK JP, sans-serif",
};

const aliases: Record<string, Palette> = {
  terracotta: "claude", amber: "claude", mono: "vercel", default: "vercel",
  jade: "supabase", emerald: "supabase", indigo: "catppuccin", rose: "catppuccin",
  "ocean-breeze": "ocean", cobalt: "ocean",
};

export function paletteFor(value: string): Palette {
  return palettes.find((palette) => palette === value) ?? aliases[value] ?? "unoskills";
}

export function makeTheme(prefs: Preferences): Theme {
  const scheme = colorScheme();
  const palette = colors[paletteFor(prefs.accent_palette)][scheme];
  const fontSize = Number(prefs.font_size);
  const radius = Number(prefs.corner_radius);
  return {
    ...palette,
    modalBackdrop: "#00000072",
    danger: scheme === "dark" ? "#fda4af" : "#a32136",
    warning: scheme === "dark" ? "#fbc879" : "#844a04",
    success: scheme === "dark" ? "#8adcb9" : "#176147",
    fontSize: fontSize >= 12 && fontSize <= 18 ? fontSize : 14,
    radius: radius >= 0 && radius <= 16 ? radius : 8,
    fontFamily: prefs.ui_font === "auto" || !prefs.ui_font.trim() ? languageFonts[prefs.language] : prefs.ui_font,
    monoFamily: prefs.mono_font === "auto" || !prefs.mono_font.trim()
      ? "JetBrains Mono, SF Mono, Cascadia Mono, Ubuntu Mono, monospace"
      : prefs.mono_font,
  };
}
