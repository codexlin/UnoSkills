import { Text } from "nsh";
import { Button, type ButtonProps } from "nsh/base";
import type { IconName } from "../icons";
import type { Theme } from "../shared";
import { Hint, ProgressBar } from "./feedback";
import { disabledControlStyle, focusOutline, interactionStyles } from "./interaction";
import { Icon, Label, textSize } from "./layout";

type ActionControlProps = Pick<ButtonProps, "role" | "accessibilityLabel" | "onPress" | "disabled"
  | "selected" | "class" | "style" | "tooltip" | "children"> & {
  theme: Theme;
  primary?: boolean;
  dangerFill?: boolean;
};

export function ActionControl(props: ActionControlProps) {
  const selection = () => props.dangerFill ? props.theme.danger
    : props.selected && !props.primary ? props.theme.accent : undefined;
  const states = () => interactionStyles(props.theme, { primary: props.primary, selectedFill: selection() });
  return <Button role={props.role} accessibilityLabel={props.accessibilityLabel} onPress={props.onPress}
    disabled={props.disabled} selected={props.selected} class={props.class} style={props.style} tooltip={props.tooltip}
    hoverStyle={states().hoverStyle} activeStyle={states().activeStyle} focusVisibleStyle={focusOutline(props.theme, 1)}
    selectedStyle={selection() ? { backgroundColor: selection(), borderColor: props.theme.primary } : undefined}
    disabledStyle={disabledControlStyle}>{props.children}</Button>;
}


const sizes = {
  badge: { height: 20, padding: 4, icon: 12 },
  small: { height: 24, padding: 6, icon: 14 },
  compact: { height: 28, padding: 8, icon: 14 },
  regular: { height: 32, padding: 10, icon: 16 },
};

export type ActionVariant = "primary" | "outline" | "ghost" | "danger" | "danger-outline";

export function Action(props: {
  theme: Theme;
  label: string;
  accessibilityLabel?: string;
  onPress: () => void | Promise<void>;
  selected?: boolean;
  disabled?: boolean;
  pending?: boolean;
  leading?: string;
  glyph?: IconName;
  iconOnly?: boolean;
  tooltip?: string;
  variant?: ActionVariant;
  size?: keyof typeof sizes;
  role?: "button" | "link";
}) {
  const metrics = () => sizes[props.size ?? "regular"];
  const primary = () => props.variant === "primary";
  const dangerFill = () => props.variant === "danger";
  const ghost = () => props.variant === "ghost";
  const ink = () => primary() ? props.theme.onPrimary : dangerFill() ? props.theme.background
    : props.variant === "danger-outline" ? props.theme.danger : props.theme.foreground;
  return <ActionControl theme={props.theme} primary={primary()} dangerFill={dangerFill()}
    role={props.role} accessibilityLabel={props.accessibilityLabel ?? props.label} onPress={props.onPress}
    disabled={props.disabled || props.pending} selected={props.selected}
    tooltip={props.tooltip ? <Hint theme={props.theme} text={props.tooltip} /> : undefined}
    style={{
      alignSelf: "flex-start", flexDirection: "row", alignItems: "center", justifyContent: "center",
      flexShrink: 0, maxWidth: "100%", gap: metrics().height <= 24 ? 4 : 6,
      minWidth: props.iconOnly ? metrics().height : undefined, minHeight: metrics().height,
      paddingHorizontal: props.iconOnly ? 0 : metrics().padding,
      borderRadius: props.theme.radius, borderWidth: 1,
      backgroundColor: primary() ? props.theme.primary : dangerFill() ? props.theme.danger
        : props.selected ? props.theme.accent : ghost() ? "transparent" : props.theme.surface,
      borderColor: primary() ? props.theme.primary : ghost() || dangerFill() ? "transparent" : props.theme.border,
    }}>
    {props.pending ? <ProgressBar theme={props.theme} label={props.label} size="indicator" onPrimary={primary()} />
      : props.glyph ? <Icon name={props.glyph} color={ink()} size={metrics().icon} />
      : props.leading ? <Text accessibilityElementsHidden style={{
        color: ink(), fontFamily: props.theme.fontFamily, fontSize: metrics().icon,
      }}>{props.leading}</Text> : null}
    {!props.iconOnly ? <Label theme={props.theme} color={ink()} weight="500" truncate
      size={props.size === "badge" ? textSize(props.theme, 12) : undefined}>{props.label}</Label> : null}
  </ActionControl>;
}
