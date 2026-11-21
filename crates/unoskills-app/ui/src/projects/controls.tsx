import { cleanup, effect, signal, Text, untracked, View, type InputRef, type JSX } from "nsh";
import { AlertDialog, Checkbox, Input, Progress, Tooltip } from "nsh/base";
import { ActionControl, Icon, PageHeader, focusOutline, interactionStyles, textSize } from "../controls";
import { SelectControl } from "../popups";
import type { IconName } from "../icons";
import { errorMessage, type ScreenProps, type Theme } from "../shared";
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
  const color = () => props.tone === "error" ? props.theme.danger
    : props.tone === "warning" ? props.theme.warning : props.theme.success;
  return <View role={props.tone === "success" ? "summary" : "alert"}
    style={{ ...row, alignItems: "flex-start", padding: 8, borderWidth: 1,
      borderColor: color(), borderRadius: props.theme.radius }}>
    <View style={{ flex: 1, minWidth: 0 }}>
      <Label theme={props.theme} label={props.label} size={12} />
    </View>
  </View>;
}

export function Hint(props: { theme: Theme; text: string }) {
  return <Tooltip placement="bottom" offset={6} style={{
    paddingHorizontal: 8, paddingVertical: 4, borderRadius: 6, backgroundColor: props.theme.foreground,
  }}>
    <Text style={{ color: props.theme.background, fontSize: 12, fontFamily: props.theme.fontFamily }}>{props.text}</Text>
  </Tooltip>;
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
  return <ActionControl theme={props.theme} primary={props.primary} accessibilityLabel={props.accessibilityLabel ?? props.label} disabled={props.disabled || props.pending} selected={props.selected} onPress={props.run}
    tooltip={props.iconOnly ? <Hint theme={props.theme} text={props.label} /> : undefined}
    style={{
      ...row, justifyContent: "center", gap: 6, flexShrink: 0, maxWidth: "100%",
      minWidth: props.iconOnly ? 28 : undefined, minHeight: props.compact ? 28 : 32,
      paddingHorizontal: props.iconOnly ? 6 : props.compact ? 8 : 12, paddingVertical: 4,
      borderRadius: props.theme.radius, borderWidth: 1,
      borderColor: props.ghost ? "transparent" : props.primary ? props.theme.primary : props.theme.border,
      backgroundColor: props.primary ? props.theme.primary : props.ghost ? "transparent" : props.theme.surface,
    }}>
    <If when={props.pending} else={<If when={props.glyph}>
      {(glyph) => <Icon name={glyph()} color={props.primary ? props.theme.onPrimary : props.theme.foreground}
        size={props.compact ? 14 : 16} />}
    </If>}>
      <Activity theme={props.theme} label={props.label} onPrimary={props.primary} />
    </If>
    <If when={!props.iconOnly}>
      <Text style={{ fontFamily: props.theme.fontFamily, fontSize: props.compact ? 12 : props.theme.fontSize,
        fontWeight: "500", minWidth: 0, flexShrink: 1, numberOfLines: 1,
        color: props.primary ? props.theme.onPrimary : props.danger ? props.theme.danger : props.theme.foreground
      }}>{props.label}</Text>
    </If>
  </ActionControl>;
}

export function Field(props: {
  theme: Theme; label: string; value: string; update: (value: string) => void;
  multiline?: boolean; search?: boolean; hideLabel?: boolean; placeholder?: string; mono?: boolean; compact?: boolean; height?: number; submit?: () => void; receiveRef?: (element: InputRef) => void;
}) {
  return <View style={{ minWidth: 0, gap: 4 }}>
    <If when={!props.search && !props.hideLabel}><Label theme={props.theme} label={props.label} weight="600" /></If>
    <Input ref={props.receiveRef} value={props.value} placeholder={props.placeholder ?? (props.search ? props.label : undefined)}
      accessibilityLabel={props.label} multiline={props.multiline}
      onChange={({ value }) => props.update(value)}
      onSubmit={() => props.submit?.()}
      focusStyle={{ borderColor: props.theme.primary }}
      style={{
        minWidth: 0, height: props.height, minHeight: props.multiline ? 112 : props.compact ? 28 : 32, paddingHorizontal: 8, paddingVertical: 6,
        backgroundColor: props.theme.background, color: props.theme.foreground, placeholderColor: props.theme.muted,
        fontFamily: props.mono ? props.theme.monoFamily : props.theme.fontFamily, fontSize: props.theme.fontSize,
        borderWidth: 1, borderColor: props.theme.border, borderRadius: props.theme.radius,
      }} />
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

export function Activity(props: { theme: Theme; label: string; onPrimary?: boolean }) {
  return <Progress indeterminate accessibilityLabel={props.label} style={{ width: 16, flexShrink: 0 }}
    trackStyle={{ height: 4, backgroundColor: props.theme.border, borderRadius: 2 }}
    indicatorStyle={{ height: 4, width: "50%", backgroundColor: props.onPrimary ? props.theme.onPrimary : props.theme.primary, borderRadius: 2 }}
    indicatorClass="animate-pulse motion-reduce:animate-none" />;
}

export function Feedback(props: { theme: Theme; pending?: boolean; error?: string; label: string }) {
  return <>
    <If when={props.pending}>
      <View style={row}>
        <Activity theme={props.theme} label={props.label} />
        <Label theme={props.theme} label={props.label} muted />
      </View>
    </If>
    <If when={props.error}>{(error) => <Banner theme={props.theme} label={error()} tone="error" />}</If>
  </>;
}

export function load<T>(fetch: () => Promise<T> | undefined, retain = false) {
  const state = signal<{ data?: T; error?: string; pending: boolean }>({ pending: true });
  effect(() => {
    let active = true;
    cleanup(() => { active = false; });
    const request = fetch();
    if (!request) {
      state.set({ pending: false });
      return;
    }
    const data = retain ? untracked(() => state().data) : undefined;
    state.set({ data, pending: true });
    void request.then(
      (data) => { if (active) state.set({ data, pending: false }); },
      (error: unknown) => { if (active) state.set({ data, error: errorMessage(error), pending: false }); },
    );
  });
  return state;
}

export function operation(props: ScreenProps) {
  const busy = signal(false);
  const error = signal<string | undefined>(undefined);
  let active = true;
  cleanup(() => { active = false; });
  return {
    busy, error,
    async run<T>(work: () => Promise<T>, done?: (result: T) => void) {
      if (busy()) return;
      busy.set(true);
      error.set(undefined);
      try {
        const result = await work();
        if (active) done?.(result);
      } catch (reason) {
        if (active) {
          const description = errorMessage(reason);
          error.set(description);
          props.notify(description, "error");
        }
      } finally {
        if (active) busy.set(false);
      }
    },
  };
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

export function toggle<T>(values: ReadonlySet<T>, value: T, checked: boolean): Set<T> {
  const next = new Set(values);
  if (checked) next.add(value);
  else next.delete(value);
  return next;
}
