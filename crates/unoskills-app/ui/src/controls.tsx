import { Image, Text, View, type JSX } from "nsh";
import { Button, Dialog, Input, Tab, Tooltip,
  type ButtonProps, type TabProps } from "nsh/base";
import { iconSource, type IconName } from "./icons";
import type { Theme } from "./shared";

export function Icon(props: { name: IconName; color: string; size?: number; spinning?: boolean }) {
  return <Image accessibilityElementsHidden src={iconSource(props.name)}
    class={props.spinning ? "animate-spin motion-reduce:animate-none" : undefined}
    style={{ width: props.size ?? 16, height: props.size ?? 16, flexShrink: 0, tintColor: props.color }} />;
}

export function textSize(theme: Theme, points: number) {
  return Math.round(theme.fontSize / 0.875) * points / 16;
}

export function PageHeader(props: { theme: Theme; title: string; children?: JSX.Element }) {
  return <View style={{ flexDirection: "row", alignItems: "center", minHeight: 56, paddingHorizontal: 20,
    paddingVertical: 8, gap: 12, borderBottomWidth: 1, borderColor: props.theme.border }}>
    <View style={{ flex: 1, minWidth: 0 }}>
      <Label theme={props.theme} heading truncate size={textSize(props.theme, 20)}>{props.title}</Label>
    </View>
    {props.children ? <View style={{ flexShrink: 0 }}>{props.children}</View> : null}
  </View>;
}

export function focusOutline(theme: Theme, offset = -1) {
  return { outlineWidth: 1, outlineOffset: offset, outlineColor: theme.primary };
}

export function interactionStyles(theme: Theme, options: { selectedFill?: string; primary?: boolean } = {}) {
  // Bind each state prop explicitly so NSH can track theme and selection changes.
  const hover = options.selectedFill
    ?? (options.primary ? theme.chrome?.primaryHover ?? theme.primary : theme.chrome?.hover ?? theme.accent);
  // The native Tabs indicator remains visible through the selected tab.
  if (hover === "transparent") {
    return { hoverStyle: { backgroundColor: hover }, activeStyle: { backgroundColor: hover, ...focusOutline(theme) } };
  }
  const pressed = options.primary && !options.selectedFill ? theme.chrome?.primaryActive : undefined;
  return {
    hoverStyle: { backgroundColor: hover },
    activeStyle: {
      backgroundColor: pressed ?? `color-mix(in srgb, ${hover} 90%, ${options.primary ? theme.onPrimary : theme.foreground})`,
    },
  };
}

export const disabledControlStyle = { opacity: 0.5 };

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

export function TabItem(props: Pick<TabProps, "accessibilityLabel" | "class" | "style" | "children"> & {
  theme: Theme;
  selected: boolean;
  onPress: () => void;
}) {
  const states = () => interactionStyles(props.theme, { selectedFill: props.selected ? "transparent" : undefined });
  return (
    <Tab
      accessibilityLabel={props.accessibilityLabel}
      selected={props.selected}
      onPress={props.onPress}
      class={props.class}
      style={props.style}
      hoverStyle={states().hoverStyle}
      activeStyle={states().activeStyle}
      focusVisibleStyle={focusOutline(props.theme)}
    >
      {props.children}
    </Tab>
  );
}

export function Label(props: {
  theme: Theme;
  children?: JSX.Element;
  muted?: boolean;
  heading?: boolean;
  mono?: boolean;
  size?: number;
  color?: string;
  truncate?: boolean;
  weight?: "400" | "500" | "600";
}) {
  return (
    <Text
      role={props.heading ? "header" : undefined}
      style={{
        minWidth: 0,
        numberOfLines: props.truncate ? 1 : undefined,
        color: props.color ?? (props.muted ? props.theme.muted : props.theme.foreground),
        fontFamily: props.mono ? props.theme.monoFamily : props.theme.fontFamily,
        fontSize: props.size ?? props.theme.fontSize,
        fontWeight: props.weight ?? (props.heading ? "600" : "400"),
      }}
    >{props.children}</Text>
  );
}

export function Action(props: {
  theme: Theme;
  label: string;
  onPress: () => void;
  onPrimary?: boolean;
  danger?: boolean;
  selected?: boolean;
  disabled?: boolean;
  compact?: boolean;
  icon?: string;
  glyph?: IconName;
  iconOnly?: boolean;
  tooltip?: string;
  variant?: "primary" | "outline" | "ghost";
  height?: number;
  role?: "button" | "link";
}) {
  const primary = () => props.variant === "primary" || props.onPrimary;
  const ghost = () => props.variant === "ghost";
  const height = () => props.height ?? (props.compact ? 30 : 36);
  const ink = () => primary() ? props.theme.onPrimary : props.danger ? props.theme.danger : props.theme.foreground;
  return (
    <ActionControl theme={props.theme} primary={primary()}
      role={props.role}
      accessibilityLabel={props.label}
      onPress={() => props.onPress()}
      disabled={props.disabled}
      selected={props.selected}
      tooltip={props.tooltip ? <Tooltip style={{
        backgroundColor: props.theme.surface, borderColor: props.theme.border, borderWidth: 1,
        borderRadius: props.theme.radius, paddingHorizontal: 8, paddingVertical: 4,
      }}><Label theme={props.theme} size={textSize(props.theme, 12)}>{props.tooltip}</Label></Tooltip> : undefined}
      style={{
        alignSelf: "flex-start",
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "center",
        minWidth: props.iconOnly ? height() : undefined,
        minHeight: height(),
        gap: height() <= 24 ? 4 : 8,
        paddingHorizontal: props.iconOnly ? 0 : height() <= 20 ? 4 : height() <= 30 ? 8 : 10,
        borderRadius: props.theme.radius,
        backgroundColor: primary() ? props.theme.primary : props.selected ? props.theme.accent : ghost() ? "transparent" : props.theme.surface,
        borderColor: primary() ? props.theme.primary : ghost() ? "transparent" : props.theme.border,
        borderWidth: 1,
      }}
    >
      {props.glyph ? <Icon name={props.glyph} color={ink()} size={height() <= 20 ? 12 : height() <= 24 ? 14 : 16} /> : null}
      {!props.iconOnly ? <Label theme={props.theme} color={ink()} weight="500" size={height() <= 20 ? textSize(props.theme, 12) : undefined}>
        {props.icon ?? props.label}
      </Label> : null}
    </ActionControl>
  );
}

export function Section(props: { theme: Theme; title: string; children?: JSX.Element }) {
  return (
    <View style={{ gap: 12, minWidth: 0, padding: 16, backgroundColor: props.theme.surface,
      borderRadius: props.theme.radius, borderColor: props.theme.border, borderWidth: 1 }}>
      <Label theme={props.theme} heading size={props.theme.fontSize + 3}>{props.title}</Label>
      {props.children}
    </View>
  );
}

export function InlineError(props: { theme: Theme; message: string }) {
  return (
    <View role="alert" style={{ padding: 8, backgroundColor: props.theme.surface,
      borderRadius: props.theme.radius, borderColor: props.theme.danger, borderWidth: 1 }}>
      <Label theme={props.theme} color={props.theme.danger}>{props.message}</Label>
    </View>
  );
}

export function FormText(props: {
  theme: Theme;
  title: string;
  detail?: string;
  value: string;
  placeholder?: string;
  masked?: boolean;
  autoFocus?: boolean;
  inline?: boolean;
  onChange: (text: string) => void;
  onSubmit?: () => void;
}) {
  return (
    <View style={{ gap: 4, minWidth: 0 }}>
      {!props.inline ? <Label theme={props.theme}>{props.title}</Label> : null}
      {!props.inline && props.detail ? <Label theme={props.theme} muted>{props.detail}</Label> : null}
      <Input
        value={props.value}
        accessibilityLabel={props.title}
        placeholder={props.placeholder}
        masked={props.masked}
        autoFocus={props.autoFocus}
        onChange={({ value }) => props.onChange(value)}
        onSubmit={props.onSubmit}
        style={{
          color: props.theme.foreground, backgroundColor: props.theme.background, placeholderColor: props.theme.muted,
          fontFamily: props.theme.fontFamily, fontSize: props.theme.fontSize,
          borderColor: props.theme.chrome?.inputBorder ?? props.theme.border, borderWidth: 1, borderRadius: props.theme.radius,
          minHeight: 32, paddingHorizontal: 10, paddingVertical: 4,
        }}
        focusStyle={{ borderColor: props.theme.primary }}
      />
    </View>
  );
}

export function Modal(props: {
  theme: Theme;
  label: string;
  open: boolean;
  width?: number;
  onClose: () => void;
  children?: JSX.Element;
}) {
  return (
    <Dialog open={props.open} accessibilityLabel={props.label}
      onOpenChange={({ open }) => { if (!open) props.onClose(); }}
      backdropStyle={{ backgroundColor: props.theme.modalBackdrop }}
      style={{ alignItems: "center", justifyContent: "center", padding: 24 }}>
      <View style={{ width: props.width ?? 440, maxWidth: "100%", gap: 16, padding: 20,
        backgroundColor: props.theme.background, borderColor: props.theme.border, borderWidth: 1,
        borderRadius: props.theme.radius }}>
        <Label theme={props.theme} heading size={20}>{props.label}</Label>
        {props.children}
      </View>
    </Dialog>
  );
}
