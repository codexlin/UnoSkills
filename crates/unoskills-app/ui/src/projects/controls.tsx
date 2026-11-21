import { Text, View, type InputRef, type JSX } from "nsh";
import { AlertDialog, Checkbox } from "nsh/base";
import { Action as ButtonAction, AlertBanner, Hint, LoadingLine, PageHeader, SelectControl, TextInput, focusOutline, interactionStyles, textSize } from "../components";
import type { IconName } from "../icons";
import type { ScreenProps, Theme } from "../shared";
import { strings } from "./strings";

export const actions = { flexDirection: "row", flexWrap: "wrap", gap: 8, alignItems: "center" } as const;
export const viewport = { flex: 1, minHeight: 0, minWidth: 0, overflow: "hidden" } as const;
export const padding = { padding: 20, gap: 12 } as const;
export const row = { flexDirection: "row", alignItems: "center", gap: 8 } as const;

export function Header(props: { theme: Theme; title: string; children?: JSX.Element }) {
  return <PageHeader theme={props.theme} title={props.title}>
    <View style={actions}>{props.children}</View>
  </PageHeader>;
}

export function Footer(props: { theme: Theme; children?: JSX.Element }) {
  return <View style={{ ...actions, minHeight: 56, flexShrink: 0, paddingHorizontal: 20,
    paddingVertical: 8, borderTopWidth: 1, borderColor: props.theme.border }}>{props.children}</View>;
}

export function Banner(props: { theme: Theme; label: string; tone: "error" | "warning" | "success" }) {
  return <AlertBanner theme={props.theme} tone={props.tone} message={props.label} />;
}

export function Heading(props: { theme: Theme; label: string; size?: number; lines?: number }) {
  return <Text role="header" style={{ color: props.theme.foreground, fontFamily: props.theme.fontFamily, fontSize: textSize(props.theme, props.size ?? 18), fontWeight: "600", numberOfLines: props.lines, minWidth: 0, flexShrink: 1 }}>{props.label}</Text>;
}

export function Label(props: { theme: Theme; label: string; muted?: boolean; lines?: number; danger?: boolean; size?: number; weight?: "500" | "600"; color?: string; title?: string }) {
  return <Text tooltip={props.title ? <Hint theme={props.theme} text={props.title} /> : undefined} style={{
    color: props.color ?? (props.danger ? props.theme.danger : props.muted ? props.theme.muted : props.theme.foreground),
    fontFamily: props.theme.fontFamily, fontSize: props.size ?? props.theme.fontSize + (props.muted ? -2 : 0), numberOfLines: props.lines,
    fontWeight: props.weight,
    minWidth: 0, flexShrink: 1,
  }}>{props.label}</Text>;
}

export function Action(props: {
  theme: Theme; label: string; run: () => void | Promise<void>;
  accessibilityLabel?: string; disabled?: boolean; selected?: boolean; primary?: boolean; danger?: boolean;
  ghost?: boolean; compact?: boolean; glyph?: IconName; iconOnly?: boolean; pending?: boolean;
}) {
  const variant = () => props.primary ? "primary" : props.ghost ? "ghost" : props.danger ? "danger-outline" : "outline";
  return <ButtonAction theme={props.theme} label={props.label} accessibilityLabel={props.accessibilityLabel}
    variant={variant()} size={props.compact ? "compact" : "regular"} glyph={props.glyph} iconOnly={props.iconOnly}
    disabled={props.disabled} selected={props.selected} pending={props.pending} onPress={props.run}
    tooltip={props.iconOnly ? props.label : undefined} />;
}

export function Field(props: {
  theme: Theme; label: string; value: string; update: (value: string) => void;
  multiline?: boolean; search?: boolean; hideLabel?: boolean; placeholder?: string; mono?: boolean; compact?: boolean; height?: number; submit?: () => void; receiveRef?: (element: InputRef) => void;
}) {
  return <View style={{ minWidth: 0, gap: 4 }}>
    <If when={!props.search && !props.hideLabel}><Label theme={props.theme} label={props.label} weight="600" /></If>
    <TextInput theme={props.theme} inputRef={props.receiveRef} value={props.value}
      placeholder={props.placeholder ?? (props.search ? props.label : undefined)} accessibilityLabel={props.label}
      multiline={props.multiline} size={props.compact ? "compact" : "regular"} mono={props.mono}
      height={props.height} onChange={props.update} onSubmit={props.submit} />
  </View>;
}

export function Choice(props: {
  theme: Theme; label: string; options: { value: string; title: string; group?: string; detail?: string }[];
  value: string | null; choose: (value: string) => void; disabled?: boolean;
  compact?: boolean; searchable?: boolean; displayValue?: string; menuWidth?: number; menuHeight?: number;
}) {
  return <View style={{ minWidth: 0 }}>
    <SelectControl theme={props.theme} label={props.label} items={props.options} value={props.value}
      disabled={props.disabled || props.options.length === 0} searchable={props.searchable} compact={props.compact}
      displayValue={props.displayValue} popupWidth={props.menuWidth} popupHeight={props.menuHeight}
      onChange={(value) => { if (value !== null) props.choose(value); }} />
  </View>;
}

export function Check(props: { theme: Theme; label: string; checked: boolean; update: (checked: boolean) => void; disabled?: boolean; compact?: boolean }) {
  return <Checkbox checked={props.checked} disabled={props.disabled}
    onChange={({ checked }) => props.update(checked)}
    hoverStyle={interactionStyles(props.theme).hoverStyle}
    activeStyle={interactionStyles(props.theme).activeStyle}
    focusVisibleStyle={focusOutline(props.theme)} disabledStyle={{ opacity: 0.5 }}
    style={{ flexDirection: "row", alignItems: "center", gap: 8, minWidth: 0, minHeight: props.compact ? 24 : 28, borderWidth: 1, borderColor: "transparent", borderRadius: 4 }}
    indicatorStyle={{
      width: 16, height: 16, flexShrink: 0, alignItems: "center", justifyContent: "center", borderWidth: 1,
      borderColor: props.checked ? props.theme.primary : props.theme.muted, borderRadius: 4,
      backgroundColor: props.checked ? props.theme.primary : props.theme.background,
    }}
    indicator={<Text style={{ color: props.theme.onPrimary, fontSize: 12 }}>{props.checked ? "✓" : ""}</Text>}>
    <Label theme={props.theme} label={props.label} lines={props.compact ? 1 : undefined} />
  </Checkbox>;
}

export function Feedback(props: { theme: Theme; pending?: boolean; error?: string; label: string }) {
  return <>
    {props.pending ? <LoadingLine theme={props.theme} label={props.label} compact /> : null}
    {props.error ? <Banner theme={props.theme} label={props.error} tone="error" /> : null}
  </>;
}

export async function confirm(props: ScreenProps, title: string, label: string, description?: string, destructive = false) {
  const { action } = await AlertDialog.show({
    title, description, closeOnEscape: true,
    actions: [
      { id: "cancel", label: strings(props.locale).cancel, role: "cancel" },
      { id: "confirm", label, role: destructive ? "destructive" : "default" },
    ],
  });
  return action === "confirm";
}
