import { colorScheme, host, nativeModule, setTheme } from "nsh";
import { Scrollbar } from "nsh/base";
import catppuccin from "../themes/catppuccin.json";
import claude from "../themes/claude.json";
import ocean from "../themes/ocean.json";
import supabase from "../themes/supabase.json";
import unoSkills from "../themes/unoskills.json";
import vercel from "../themes/vercel.json";
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
  corner_radius: "auto", ui_font: "auto", mono_font: "auto", sidebar_collapsed: "false",
  deploy_mode: "symlink", proxy: "", token: "", root_dir: "", version: "",
};

export const palettes = ["claude", "vercel", "supabase", "catppuccin", "ocean", "unoskills"] as const;
type Palette = typeof palettes[number];

type Scheme = "light" | "dark";
type Swatches = typeof unoSkills.cssVars.light;

const presets: Record<Palette, typeof unoSkills> = {
  claude, vercel, supabase, catppuccin, ocean, unoskills: unoSkills,
};

// Adjust foregrounds where the published color pairs fall below 4.5:1 for small text.
const readableInk: Partial<Record<Palette, Partial<Record<Scheme, Partial<Swatches>>>>> = {
  unoskills: {
    light: { "secondary-foreground": unoSkills.cssVars.light.foreground },
    dark: { "primary-foreground": unoSkills.cssVars.dark.background },
  },
  claude: {
    light: { "primary-foreground": "#141413" },
    dark: { "primary-foreground": "#141413", "destructive-foreground": "#141413" },
  },
  vercel: { light: { "destructive-foreground": "#141413" } },
  catppuccin: { light: { "accent-foreground": "#141413", "sidebar-accent-foreground": "#141413" } },
  ocean: {
    light: { "primary-foreground": ocean.cssVars.dark.background, "destructive-foreground": "#141413" },
    dark: { "accent-foreground": ocean.cssVars.dark.foreground, "sidebar-accent-foreground": ocean.cssVars.dark.foreground },
  },
};

function swatches(palette: Palette, scheme: Scheme): Swatches {
  return { ...presets[palette].cssVars[scheme], ...readableInk[palette]?.[scheme] };
}

function mix(first: string, weight: number, second: string) {
  return `color-mix(in oklab, ${first} ${weight}%, ${second})`;
}

export function loadThemeFonts() {
  return host.params.bundled_fonts === true
    ? nativeModule<{ Fonts: { load(): Promise<void> } }>("unoskills/desktop").Fonts.load()
    : Promise.resolve();
}

const presetFamily = (definition: string) => definition.split(",")[0].trim();

export const bundledThemeFonts = [...new Set(Object.values(presets).flatMap(({ cssVars }) =>
  [cssVars.theme["font-sans"], cssVars.theme["font-mono"]].map(presetFamily),
).filter((name) => !["ui-sans-serif", "ui-monospace", "monospace"].includes(name)))].sort();

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
  const palette = paletteFor(prefs.accent_palette);
  const preset = presets[palette];
  const tokens = swatches(palette, scheme);
  const shadows = preset.shadows[scheme];
  const baseRadius = parseFloat(preset.cssVars.theme.radius) * 16;
  const requestedRadius = prefs.corner_radius.trim() ? Number(prefs.corner_radius) : NaN;
  const radiusOverride = requestedRadius >= 0 && requestedRadius <= 16 ? requestedRadius : undefined;
  const fontSize = Number(prefs.font_size);
  const sans = presetFamily(preset.cssVars.theme["font-sans"]);
  const mono = presetFamily(preset.cssVars.theme["font-mono"]);
  const autoSans = sans === "ui-sans-serif" ? languageFonts[prefs.language] : `${sans}, ${languageFonts[prefs.language]}`;
  const autoMono = ["ui-monospace", "monospace"].includes(mono) ? "monospace"
    : `${mono}, SF Mono, Cascadia Mono, Ubuntu Mono, DejaVu Sans Mono, monospace`;
  const inputFill = scheme === "dark" ? mix(tokens.input, 30, "transparent") : "transparent";
  const hoverWeight = palette === "catppuccin" && scheme === "dark" ? 24 : 56;
  return {
    background: tokens.background, surface: tokens.card, foreground: tokens.foreground,
    muted: tokens["muted-foreground"], border: tokens.border, primary: tokens.primary,
    onPrimary: tokens["primary-foreground"], accent: tokens.accent, onAccent: tokens["accent-foreground"],
    secondary: { background: tokens.secondary, foreground: tokens["secondary-foreground"] },
    destructive: { background: tokens.destructive, foreground: tokens["destructive-foreground"] },
    sidebar: {
      foreground: tokens["sidebar-foreground"], selected: tokens["sidebar-accent"],
      onSelected: tokens["sidebar-accent-foreground"], ring: tokens["sidebar-ring"],
    },
    chrome: {
      rail: tokens.sidebar, hover: mix(tokens.accent, hoverWeight, tokens.card), segmented: tokens.muted,
      primaryHover: mix(tokens.primary, 92, tokens.background),
      primaryActive: mix(tokens.primary, 86, tokens.background), inputBorder: tokens.input,
    },
    modalBackdrop: "#00000072",
    danger: scheme === "dark" ? mix(tokens.destructive, 50, tokens.foreground) : mix(tokens.destructive, 76, tokens.foreground),
    warning: scheme === "dark" ? "#fbc879" : "#844a04",
    success: scheme === "dark" ? "#8adcb9" : "#176147",
    ring: tokens.ring,
    fontSize: fontSize >= 12 && fontSize <= 18 ? fontSize : 14,
    radius: radiusOverride ?? baseRadius,
    control: {
      radius: radiusOverride ?? Math.max(0, baseRadius - 2), inputFill,
      outlineFill: scheme === "dark" ? inputFill : tokens.background,
      shadow: shadows["shadow-xs"],
    },
    panel: { radius: radiusOverride ?? baseRadius + 4, shadow: shadows["shadow-sm"] },
    popup: { background: tokens.popover, foreground: tokens["popover-foreground"], shadow: shadows["shadow-md"] },
    fontFamily: prefs.ui_font === "auto" || !prefs.ui_font.trim() ? autoSans : prefs.ui_font.trim(),
    monoFamily: prefs.mono_font === "auto" || !prefs.mono_font.trim() ? autoMono : prefs.mono_font.trim(),
  };
}

export function applyNativeTheme(theme: Theme, paletteName: string) {
  const palette = paletteFor(paletteName);
  setTheme({ cssVars: {
    light: swatches(palette, "light"),
    dark: swatches(palette, "dark"),
    theme: {
      "radius": `${theme.radius}px`,
      "font-sans": theme.fontFamily ?? "sans-serif",
      "font-mono": theme.monoFamily ?? "monospace",
    },
  } });
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
