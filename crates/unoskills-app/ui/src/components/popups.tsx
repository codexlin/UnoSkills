import { View, signal, type JSX } from "nsh";
import { Dialog, Menu, MenuItem, MenuSeparator, Select, type SelectItem } from "nsh/base";
import type { IconName } from "../icons";
import { Icon, Label, textStyle } from "./layout";
import { disabledControlStyle, focusOutline, interactionStyles } from "./interaction";
import { Hint } from "./feedback";
import { dialogAnimation } from "./motion";
import type { Theme } from "../shared";

export function popupSurface(theme: Theme) {
  return { borderWidth: 1, borderColor: theme.border,
    backgroundColor: theme.popup.background, boxShadow: theme.popup.shadow };
}

function PopupTrigger(props: {
  theme: Theme; open: boolean; title?: string; value?: string; hint?: string; disabled?: boolean; muted?: boolean;
  compact?: boolean; toolbar?: boolean; width?: number; maxWidth?: number;
}) {
  const iconOnly = () => props.value === undefined;
  const states = () => interactionStyles(props.theme, { selectedFill: props.open ? props.theme.accent : undefined });
  return <View accessibilityLabel={props.title}
    tooltip={props.hint && !props.open ? <Hint theme={props.theme} text={props.hint} /> : undefined}
    style={{
      flexDirection: "row", alignItems: "center", justifyContent: iconOnly() ? "center" : "space-between",
      width: iconOnly() ? 32 : props.width, maxWidth: props.maxWidth, minWidth: 0,
      minHeight: props.compact ? 28 : props.toolbar ? 34 : 32, gap: 8,
      paddingHorizontal: iconOnly() ? 0 : props.compact ? 8 : 10,
      borderRadius: props.theme.control.radius, borderWidth: 1,
      boxShadow: iconOnly() || props.open ? undefined : props.theme.control.shadow,
      borderColor: props.open ? props.theme.ring : iconOnly() ? "transparent"
        : props.theme.chrome?.inputBorder ?? props.theme.border,
      backgroundColor: props.open ? props.theme.accent : iconOnly() ? "transparent" : props.theme.control.outlineFill,
    }}
    hoverStyle={props.disabled ? undefined : states().hoverStyle}
    activeStyle={props.disabled ? undefined : states().activeStyle}
    focusVisibleStyle={focusOutline(props.theme, 1)} disabledStyle={disabledControlStyle}>
    {iconOnly() ? <Icon name="ellipsis" color={props.open ? props.theme.onAccent : props.theme.foreground} size={16} /> : <>
      <View style={{ flexShrink: 1, minWidth: 0 }}>
        <Label theme={props.theme} truncate muted={props.muted} color={props.open ? props.theme.onAccent : undefined}>{props.value}</Label>
      </View>
      <Icon name="chevron-down" color={props.open ? props.theme.onAccent : props.theme.muted} size={14} />
    </>}
  </View>;
}

export function ActionMenu(props: {
  theme: Theme; label: string; value?: string; minWidth?: number; maxTriggerWidth?: number; children?: JSX.Element;
}) {
  const open = signal(false);
  return <Menu open={open()} onOpenChange={(event) => open.set(event.open)}
    align={props.value === undefined ? "end" : "start"} offset={4}
    popupStyle={{ ...popupSurface(props.theme), padding: 4, borderRadius: props.theme.radius, minWidth: props.minWidth ?? 200 }}
    trigger={<PopupTrigger theme={props.theme} title={props.label} hint={props.label}
      open={open()} value={props.value} maxWidth={props.maxTriggerWidth} toolbar={props.value !== undefined} />}>
    {props.children}
  </Menu>;
}

export function MenuAction(props: {
  theme: Theme; label: string; onSelect: () => void | Promise<void>;
  checked?: boolean; disabled?: boolean; danger?: boolean; glyph?: IconName;
}) {
  const states = () => interactionStyles(props.theme, { selectedFill: props.checked ? props.theme.accent : undefined });
  const glyph = () => props.glyph ?? (props.checked ? "check" : undefined);
  return <MenuItem accessibilityLabel={props.label} role={props.checked === undefined ? undefined : "menuitemradio"}
    checked={props.checked} onSelect={props.onSelect} disabled={props.disabled}
    style={{ flexDirection: "row", alignItems: "center", gap: 8, minWidth: 0,
      paddingHorizontal: 10, paddingVertical: 6, borderRadius: props.theme.control.radius }}
    hoverStyle={states().hoverStyle} activeStyle={states().activeStyle}
    focusStyle={{ ...focusOutline(props.theme), backgroundColor: states().hoverStyle.backgroundColor }}
    checkedStyle={{ backgroundColor: props.theme.accent }} disabledStyle={disabledControlStyle}>
    {props.checked !== undefined || props.glyph ? <View style={{ width: 14, flexShrink: 0 }}>
      <If when={glyph()}>{(name) => <Icon name={name()} color={props.checked ? props.theme.onAccent : props.theme.primary} size={14} />}</If>
    </View> : null}
    <Label theme={props.theme} truncate color={props.danger ? props.theme.danger
      : props.checked ? props.theme.onAccent : props.theme.popup.foreground}>{props.label}</Label>
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
    popupStyle={{ ...popupSurface(props.theme), padding: 4, borderRadius: props.theme.radius,
      width: props.popupWidth, minWidth: props.popupMinWidth, maxHeight: height() }}
    listStyle={{ maxHeight: height() - (props.searchable ? 48 : 10), minHeight: 0, flexShrink: 1 }}
    itemClass="focus:bg-muted selected:bg-accent selected:focus:bg-accent focus:outline-1 focus:-outline-offset-1 focus:outline-ring disabled:opacity-50"
    itemStyle={{ borderRadius: props.theme.control.radius, paddingHorizontal: 8, paddingVertical: 6 }}
    searchStyle={{
      marginBottom: 6, paddingHorizontal: 8, minHeight: 32, flexShrink: 0,
      color: props.theme.popup.foreground, placeholderColor: props.theme.muted, backgroundColor: props.theme.control.inputFill,
      ...textStyle(props.theme),
      borderWidth: 1, borderColor: props.theme.chrome?.inputBorder ?? props.theme.border, borderRadius: props.theme.control.radius,
    }}
    renderItem={(item) => {
      const detail = () => props.items[item.index]?.detail;
      const selected = () => item.value === props.value;
      return <View style={{ minWidth: 0, gap: 2 }}>
        <Label theme={props.theme} truncate color={selected() ? props.theme.onAccent : props.theme.popup.foreground}>{item.title}</Label>
        {detail() ? <Label theme={props.theme} muted truncate color={selected() ? props.theme.onAccent : undefined}
          size={props.theme.fontSize - 2}>{detail()}</Label> : null}
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

export function Modal(props: {
  theme: Theme;
  label: string;
  open: boolean;
  width?: number;
  closeOnEscape?: boolean;
  closeOnBackdropPress?: boolean;
  onClose: () => void;
  children?: JSX.Element;
}) {
  return (
    <Dialog open={props.open} animation={dialogAnimation()} accessibilityLabel={props.label}
      closeOnEscape={props.closeOnEscape}
      closeOnBackdropPress={props.closeOnBackdropPress}
      onOpenChange={({ open }) => { if (!open) props.onClose(); }}
      backdropStyle={{ backgroundColor: props.theme.modalBackdrop }}
      style={{ alignItems: "center", justifyContent: "center", padding: 24 }}>
      <View style={{ width: props.width ?? 440, maxWidth: "100%", gap: 16,
        ...popupSurface(props.theme), padding: 20, borderRadius: props.theme.panel.radius }}>
        <Label theme={props.theme} heading size={20} color={props.theme.popup.foreground}>{props.label}</Label>
        {props.children}
      </View>
    </Dialog>
  );
}
