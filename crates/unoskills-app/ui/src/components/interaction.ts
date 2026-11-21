import type { Theme } from "../shared";

export function focusOutline(theme: Theme, offset = -1) {
  return { outlineWidth: 1, outlineOffset: offset, outlineColor: theme.primary };
}

export function interactionStyles(theme: Theme, options: { selectedFill?: string; primary?: boolean } = {}) {
  // Bind each state prop explicitly so NSH can track theme and selection changes.
  const hover = options.selectedFill
    ?? (options.primary ? theme.chrome?.primaryHover ?? theme.primary : theme.chrome?.hover ?? theme.accent);
  const pressed = options.primary && !options.selectedFill ? theme.chrome?.primaryActive : undefined;
  return {
    hoverStyle: { backgroundColor: hover },
    activeStyle: {
      backgroundColor: pressed ?? `color-mix(in srgb, ${hover} 90%, ${options.primary ? theme.onPrimary : theme.foreground})`,
    },
  };
}

export const disabledControlStyle = { opacity: 0.5 };
