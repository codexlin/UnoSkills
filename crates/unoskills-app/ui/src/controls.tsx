import { Text, View, type JSX } from "nsh";
import { Button, Dialog, Input, Select } from "nsh/base";
import type { Theme } from "./shared";

export function Label(props: {
  theme: Theme;
  children?: JSX.Element;
  muted?: boolean;
  heading?: boolean;
  mono?: boolean;
  size?: number;
  color?: string;
  truncate?: boolean;
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
        fontWeight: props.heading ? "600" : "400",
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
  role?: "button" | "link";
}) {
  return (
    <Button
      role={props.role}
      accessibilityLabel={props.label}
      onPress={() => props.onPress()}
      disabled={props.disabled}
      selected={props.selected}
      style={{
        alignSelf: "flex-start",
        justifyContent: "center",
        minHeight: props.compact ? 30 : 36,
        paddingHorizontal: props.compact ? 8 : 12,
        borderRadius: props.theme.radius,
        backgroundColor: props.onPrimary ? props.theme.primary : props.selected ? props.theme.accent : props.theme.surface,
        borderColor: props.onPrimary ? props.theme.primary : props.theme.border,
        borderWidth: 1,
      }}
      focusVisibleStyle={{ borderColor: props.theme.primary }}
      hoverStyle={{ opacity: 0.86 }}
      activeStyle={{ opacity: 0.72 }}
      disabledStyle={{ opacity: 0.5 }}
    >
      <Label theme={props.theme} color={props.onPrimary ? props.theme.onPrimary : props.danger ? props.theme.danger : undefined}>
        {props.icon ?? props.label}
      </Label>
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
    <View role="alert" style={{ padding: 8, borderRadius: props.theme.radius, borderColor: props.theme.danger, borderWidth: 1 }}>
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
  onChange: (text: string) => void;
  onSubmit?: () => void;
}) {
  return (
    <View style={{ gap: 4, minWidth: 0 }}>
      <Label theme={props.theme}>{props.title}</Label>
      {props.detail ? <Label theme={props.theme} muted>{props.detail}</Label> : null}
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
          borderColor: props.theme.border, borderWidth: 1, borderRadius: props.theme.radius,
          minHeight: 36, paddingHorizontal: 10, paddingVertical: 6,
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
  searchable?: boolean;
  searchLabel?: string;
  onChange: (value: string) => void;
}) {
  return (
    <View style={{ gap: 4, minWidth: 0 }}>
      <Label theme={props.theme}>{props.title}</Label>
      {props.detail ? <Label theme={props.theme} muted>{props.detail}</Label> : null}
      <Select
        items={props.items}
        value={props.value}
        accessibilityLabel={props.title}
        searchable={props.searchable}
        searchAccessibilityLabel={props.searchLabel}
        searchPlaceholder={props.searchLabel}
        onChange={({ value }) => { if (value !== null) props.onChange(value); }}
        offset={4}
        itemClass="rounded-md px-2 py-1.5 text-sm focus:bg-accent focus:text-accent-foreground disabled:opacity-50"
        popupStyle={{ padding: 4, backgroundColor: props.theme.surface, borderColor: props.theme.border,
          borderWidth: 1, borderRadius: props.theme.radius }}
        renderItem={(item) => <Label theme={props.theme}>{item.title}</Label>}
        trigger={
          <View style={{ minHeight: 36, paddingHorizontal: 10, borderRadius: props.theme.radius,
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
  onClose: () => void;
  children?: JSX.Element;
}) {
  return (
    <Dialog open={props.open} accessibilityLabel={props.label}
      onOpenChange={({ open }) => { if (!open) props.onClose(); }}
      backdropStyle={{ backgroundColor: props.theme.modalBackdrop }}
      style={{ alignItems: "center", justifyContent: "center", padding: 24 }}>
      <View style={{ width: 440, maxWidth: "100%", gap: 16, padding: 20,
        backgroundColor: props.theme.background, borderColor: props.theme.border, borderWidth: 1,
        borderRadius: props.theme.radius }}>
        <Label theme={props.theme} heading size={20}>{props.label}</Label>
        {props.children}
      </View>
    </Dialog>
  );
}
