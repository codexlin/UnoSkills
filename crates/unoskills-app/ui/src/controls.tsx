import { Image, Text, View, type JSX } from "nsh";
import { Button, Dialog, Input, Select, Tab, Tooltip, type TabProps } from "nsh/base";
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

export function focusOutline(theme: Theme) {
  return { outlineWidth: 1, outlineOffset: -1, outlineColor: theme.primary };
}

export function interactionStyles(theme: Theme, options: { selectedFill?: string; primary?: boolean } = {}) {
  // Bind each state prop explicitly so NSH can track theme and selection changes.
  const hover = theme.chrome?.hover ?? theme.accent;
  return {
    hoverStyle: {
      backgroundColor: options.selectedFill
        ?? (options.primary ? theme.chrome?.primaryHover ?? theme.primary : hover),
    },
    activeStyle: {
      backgroundColor: options.selectedFill
        ?? (options.primary ? theme.chrome?.primaryActive ?? theme.primary
          : `color-mix(in srgb, ${hover} 60%, ${theme.background})`),
    },
  };
}

export function menuItemStyles(theme: Theme, selected = false) {
  return {
    ...interactionStyles(theme, { selectedFill: selected ? theme.accent : undefined }),
    focusStyle: {
      ...focusOutline(theme),
      backgroundColor: selected ? theme.accent : theme.chrome?.hover ?? theme.accent,
    },
    checkedStyle: { backgroundColor: theme.accent },
    disabledStyle: { opacity: 0.55 },
  };
}

export function selectPopupStyles(theme: Theme, options: { searchable?: boolean; width?: number; minWidth?: number; height?: number } = {}) {
  const maxHeight = options.height ?? 244;
  return {
    popupStyle: {
      width: options.width, minWidth: options.minWidth, maxHeight, padding: 4,
      borderWidth: 1, borderColor: theme.border, borderRadius: theme.radius, backgroundColor: theme.surface,
    },
    listStyle: { maxHeight: maxHeight - (options.searchable ? 48 : 10), minHeight: 0, flexShrink: 1 },
    itemClass: "focus:bg-muted selected:bg-accent selected:focus:bg-accent focus:outline-1 focus:-outline-offset-1 focus:outline-ring disabled:opacity-50",
    searchStyle: options.searchable ? {
      marginBottom: 6, paddingHorizontal: 8, minHeight: 32, flexShrink: 0,
      color: theme.foreground, placeholderColor: theme.muted, backgroundColor: theme.background,
      fontFamily: theme.fontFamily, fontSize: theme.fontSize,
      borderWidth: 1, borderColor: theme.chrome?.inputBorder ?? theme.border, borderRadius: theme.radius,
    } : undefined,
  };
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
  const states = () => interactionStyles(props.theme, {
    primary: primary(),
    selectedFill: !primary() && props.selected ? props.theme.accent : undefined,
  });
  return (
    <Button
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
      hoverStyle={states().hoverStyle}
      activeStyle={states().activeStyle}
      focusVisibleStyle={focusOutline(props.theme)}
      disabledStyle={{ opacity: 0.5 }}
    >
      {props.glyph ? <Icon name={props.glyph} color={ink()} size={height() <= 20 ? 12 : height() <= 24 ? 14 : 16} /> : null}
      {!props.iconOnly ? <Label theme={props.theme} color={ink()} weight="500" size={height() <= 20 ? textSize(props.theme, 12) : undefined}>
        {props.icon ?? props.label}
      </Label> : null}
    </Button>
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

export function Choice(props: {
  theme: Theme;
  title: string;
  detail?: string;
  value: string;
  items: { value: string; title: string }[];
  inline?: boolean;
  searchable?: boolean;
  searchLabel?: string;
  onChange: (value: string) => void;
}) {
  const popup = () => selectPopupStyles(props.theme, { searchable: props.searchable });
  return (
    <View style={{ gap: 4, minWidth: 0 }}>
      {!props.inline ? <Label theme={props.theme}>{props.title}</Label> : null}
      {!props.inline && props.detail ? <Label theme={props.theme} muted>{props.detail}</Label> : null}
      <Select
        items={props.items}
        value={props.value}
        accessibilityLabel={props.title}
        searchable={props.searchable}
        searchAccessibilityLabel={props.searchLabel}
        searchPlaceholder={props.searchLabel}
        onChange={({ value }) => { if (value !== null) props.onChange(value); }}
        offset={4}
        popupStyle={popup().popupStyle}
        listStyle={popup().listStyle}
        searchStyle={popup().searchStyle}
        itemClass={popup().itemClass}
        cover={false}
        itemStyle={{ borderRadius: props.theme.radius, paddingHorizontal: 8, paddingVertical: 6 }}
        renderItem={(item) => <Label theme={props.theme}>{item.title}</Label>}
        trigger={
          <View
            hoverStyle={interactionStyles(props.theme).hoverStyle}
            activeStyle={interactionStyles(props.theme).activeStyle}
            focusVisibleStyle={focusOutline(props.theme)} style={{ minHeight: props.inline ? 28 : 36, paddingHorizontal: 10, borderRadius: props.theme.radius,
            borderColor: props.theme.border, borderWidth: 1, backgroundColor: props.theme.background,
            flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
            <Label theme={props.theme} truncate>{props.items.find((item) => item.value === props.value)?.title ?? props.value}</Label>
            <Text accessibilityElementsHidden style={{ color: props.theme.muted }}>{"▾"}</Text>
          </View>
        }
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
