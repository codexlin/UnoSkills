import { Text, View, signal, type JSX } from "nsh";
import { Menu, MenuItem, MenuSeparator, Select, Tooltip, type SelectItem } from "nsh/base";
import { Icon, Label, disabledControlStyle, focusOutline, interactionStyles } from "./controls";
import type { Theme } from "./shared";

function popupSurface(theme: Theme) {
  return { padding: 4, borderWidth: 1, borderColor: theme.border, borderRadius: theme.radius, backgroundColor: theme.surface };
}

function PopupTrigger(props: {
  theme: Theme; open: boolean; title?: string; value?: string; hint?: string; disabled?: boolean; muted?: boolean;
  compact?: boolean; toolbar?: boolean; width?: number; maxWidth?: number;
}) {
  const iconOnly = () => props.value === undefined;
  const states = () => interactionStyles(props.theme, { selectedFill: props.open ? props.theme.accent : undefined });
  return <View accessibilityLabel={props.title}
    tooltip={props.hint && !props.open ? <Tooltip placement="bottom" offset={6} style={{
      paddingHorizontal: 8, paddingVertical: 4, borderRadius: props.theme.radius, backgroundColor: props.theme.foreground,
    }}><Label theme={props.theme} size={12} color={props.theme.background}>{props.hint}</Label></Tooltip> : undefined}
    style={{
      flexDirection: "row", alignItems: "center", justifyContent: iconOnly() ? "center" : "space-between",
      width: iconOnly() ? 32 : props.width, maxWidth: props.maxWidth, minWidth: 0,
      minHeight: props.compact ? 28 : props.toolbar ? 34 : 32, gap: 8,
      paddingHorizontal: iconOnly() ? 0 : props.compact ? 8 : 10,
      borderRadius: props.theme.radius, borderWidth: 1,
      borderColor: props.open ? props.theme.primary : iconOnly() ? "transparent" : props.theme.border,
      backgroundColor: props.open ? props.theme.accent : iconOnly() ? "transparent" : props.theme.background,
    }}
    hoverStyle={props.disabled ? undefined : states().hoverStyle}
    activeStyle={props.disabled ? undefined : states().activeStyle}
    focusVisibleStyle={focusOutline(props.theme, 1)} disabledStyle={disabledControlStyle}>
    {iconOnly() ? <Icon name="ellipsis" color={props.theme.foreground} size={16} /> : <>
      <View style={{ flexShrink: 1, minWidth: 0 }}>
        <Label theme={props.theme} truncate muted={props.muted}>{props.value}</Label>
      </View>
      <Text accessibilityElementsHidden style={{ color: props.theme.muted }}>▾</Text>
    </>}
  </View>;
}

export function ActionMenu(props: {
  theme: Theme; label: string; value?: string; minWidth?: number; maxTriggerWidth?: number; children?: JSX.Element;
}) {
  const open = signal(false);
  return <Menu open={open()} onOpenChange={(event) => open.set(event.open)}
    align={props.value === undefined ? "end" : "start"} offset={4}
    popupStyle={{ ...popupSurface(props.theme), minWidth: props.minWidth ?? 200 }}
    trigger={<PopupTrigger theme={props.theme} title={props.label} hint={props.label}
      open={open()} value={props.value} maxWidth={props.maxTriggerWidth} toolbar={props.value !== undefined} />}>
    {props.children}
  </Menu>;
}

export function MenuAction(props: {
  theme: Theme; label: string; onSelect: () => void | Promise<void>;
  checked?: boolean; disabled?: boolean; danger?: boolean; leading?: string;
}) {
  const states = () => interactionStyles(props.theme, { selectedFill: props.checked ? props.theme.accent : undefined });
  return <MenuItem accessibilityLabel={props.label} role={props.checked === undefined ? undefined : "menuitemradio"}
    checked={props.checked} onSelect={props.onSelect} disabled={props.disabled}
    style={{ flexDirection: "row", alignItems: "center", gap: 8, minWidth: 0,
      paddingHorizontal: 10, paddingVertical: 6, borderRadius: props.theme.radius }}
    hoverStyle={states().hoverStyle} activeStyle={states().activeStyle}
    focusStyle={{ ...focusOutline(props.theme), backgroundColor: states().hoverStyle.backgroundColor }}
    checkedStyle={{ backgroundColor: props.theme.accent }} disabledStyle={disabledControlStyle}>
    {props.checked !== undefined || props.leading ? <Text accessibilityElementsHidden
      style={{ width: 14, fontFamily: props.theme.fontFamily, color: props.theme.primary }}>
      {props.leading ?? (props.checked ? "✓" : "")}
    </Text> : null}
    <Label theme={props.theme} truncate color={props.danger ? props.theme.danger : undefined}>{props.label}</Label>
  </MenuItem>;
}

export function MenuDivider(props: { theme: Theme }) {
  return <MenuSeparator style={{ height: 1, marginVertical: 4, backgroundColor: props.theme.border }} />;
}

export function SelectControl(props: {
  theme: Theme; label: string; value: string | null; items: (SelectItem & { detail?: string })[];
  onChange: (value: string | null) => void; displayValue?: string; disabled?: boolean;
  compact?: boolean; toolbar?: boolean; searchable?: boolean; searchLabel?: string; hint?: string;
  width?: number; maxWidth?: number; popupWidth?: number; popupMinWidth?: number; popupHeight?: number;
}) {
  const open = signal(false);
  const height = () => props.popupHeight ?? 244;
  const searchLabel = () => props.searchLabel ?? props.label;
  return <Select items={props.items} value={props.value} accessibilityLabel={props.label} disabled={props.disabled}
    open={open()} onOpenChange={(event) => open.set(event.open)}
    onChange={({ value }) => props.onChange(value)} placeholder={props.label}
    searchable={props.searchable} searchPlaceholder={searchLabel()} searchAccessibilityLabel={searchLabel()}
    offset={4} cover={false}
    popupStyle={{ ...popupSurface(props.theme), width: props.popupWidth, minWidth: props.popupMinWidth, maxHeight: height() }}
    listStyle={{ maxHeight: height() - (props.searchable ? 48 : 10), minHeight: 0, flexShrink: 1 }}
    itemClass="focus:bg-muted selected:bg-accent selected:focus:bg-accent focus:outline-1 focus:-outline-offset-1 focus:outline-ring disabled:opacity-50"
    itemStyle={{ borderRadius: props.theme.radius, paddingHorizontal: 8, paddingVertical: 6 }}
    searchStyle={{
      marginBottom: 6, paddingHorizontal: 8, minHeight: 32, flexShrink: 0,
      color: props.theme.foreground, placeholderColor: props.theme.muted, backgroundColor: props.theme.background,
      fontFamily: props.theme.fontFamily, fontSize: props.theme.fontSize,
      borderWidth: 1, borderColor: props.theme.chrome?.inputBorder ?? props.theme.border, borderRadius: props.theme.radius,
    }}
    renderItem={(item) => {
      const detail = () => props.items[item.index]?.detail;
      return <View style={{ minWidth: 0, gap: 2 }}>
        <Label theme={props.theme} truncate>{item.title}</Label>
        {detail() ? <Label theme={props.theme} muted truncate size={props.theme.fontSize - 2}>{detail()}</Label> : null}
      </View>;
    }}
    trigger={<PopupTrigger theme={props.theme} open={open()} hint={props.hint} disabled={props.disabled}
      compact={props.compact} toolbar={props.toolbar} width={props.width} maxWidth={props.maxWidth} muted={props.value === null}
      value={props.displayValue ?? props.items.find((item) => item.value === props.value)?.title ?? props.value ?? props.label} />}
  />;
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
  return (
    <View style={{ gap: 4, minWidth: 0 }}>
      {!props.inline ? <Label theme={props.theme}>{props.title}</Label> : null}
      {!props.inline && props.detail ? <Label theme={props.theme} muted>{props.detail}</Label> : null}
      <SelectControl theme={props.theme} label={props.title} items={props.items} value={props.value}
        compact={props.inline} searchable={props.searchable} searchLabel={props.searchLabel}
        onChange={(value) => { if (value !== null) props.onChange(value); }} />
    </View>
  );
}
