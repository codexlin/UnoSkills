import { cleanup, effect, signal, Text, untracked, View, type InputRef, type JSX } from "nsh";
import { AlertDialog, Button, Checkbox, Input, Progress, Select } from "nsh/base";
import { errorMessage, type ScreenProps, type Theme } from "../shared";
import { strings } from "./strings";

export const actions = { flexDirection: "row", flexWrap: "wrap", gap: 8, alignItems: "center" } as const;
export const viewport = { flex: 1, minHeight: 0, minWidth: 0, overflow: "hidden" } as const;
export const padding = { padding: 24, gap: 16 } as const;

export function Heading(props: { theme: Theme; label: string; level?: 1 | 2 }) {
  return <Text role="header" style={{ color: props.theme.foreground, fontFamily: props.theme.fontFamily, fontSize: props.theme.fontSize + (props.level === 1 ? 6 : 2), fontWeight: "600", minWidth: 0, flexShrink: 1 }}>{props.label}</Text>;
}

export function Label(props: { theme: Theme; label: string; muted?: boolean; lines?: number; danger?: boolean }) {
  return <Text style={{
    color: props.danger ? props.theme.danger : props.muted ? props.theme.muted : props.theme.foreground,
    fontFamily: props.theme.fontFamily, fontSize: props.theme.fontSize + (props.muted ? -2 : 0), numberOfLines: props.lines,
    minWidth: 0, flexShrink: 1,
  }}>{props.label}</Text>;
}

export function Action(props: {
  theme: Theme; label: string; run: () => void | Promise<void>;
  disabled?: boolean; selected?: boolean; primary?: boolean; danger?: boolean;
}) {
  return <Button accessibilityLabel={props.label} disabled={props.disabled} selected={props.selected} onPress={props.run}
    hoverStyle={{ backgroundColor: props.primary ? props.theme.primary : props.theme.accent }}
    activeStyle={{ opacity: 0.8 }}
    focusStyle={{ borderColor: props.theme.primary }}
    selectedStyle={{ backgroundColor: props.theme.accent, borderColor: props.theme.primary }}
    disabledStyle={{ opacity: 0.5 }}
    style={{
      alignItems: "center", justifyContent: "center", minHeight: 32, flexShrink: 0,
      paddingHorizontal: 12, paddingVertical: 4, borderRadius: props.theme.radius, borderWidth: 1,
      borderColor: props.primary ? props.theme.primary : props.theme.border,
      backgroundColor: props.primary ? props.theme.primary : props.theme.surface,
    }}>
    <Text style={{ fontFamily: props.theme.fontFamily, fontSize: props.theme.fontSize, fontWeight: "500",
      color: props.primary ? props.theme.onPrimary : props.danger ? props.theme.danger : props.theme.foreground
    }}>{props.label}</Text>
  </Button>;
}

export function RowButton(props: { theme: Theme; title: string; detail?: string; selected?: boolean; run: () => void }) {
  return <Button selected={props.selected} onPress={props.run} style={{
    minWidth: 0, minHeight: 48, gap: 2, justifyContent: "center", paddingHorizontal: 12, paddingVertical: 6,
    borderWidth: 1, borderColor: props.theme.surface, borderRadius: props.theme.radius,
  }} hoverStyle={{ backgroundColor: props.theme.accent }}
    selectedStyle={{ backgroundColor: props.theme.accent, borderColor: props.theme.border }}
    focusStyle={{ borderColor: props.theme.primary }} activeStyle={{ opacity: 0.8 }}>
    <Label theme={props.theme} label={props.title} lines={1} />
    <If when={props.detail}>{(detail) => <Label theme={props.theme} label={detail()} muted lines={1} />}</If>
  </Button>;
}

export function Field(props: {
  theme: Theme; label: string; value: string; update: (value: string) => void;
  multiline?: boolean; search?: boolean; mono?: boolean; submit?: () => void; receiveRef?: (element: InputRef) => void;
}) {
  return <View style={{ minWidth: 0, gap: 4 }}>
    <If when={!props.search}><Label theme={props.theme} label={props.label} /></If>
    <Input ref={props.receiveRef} value={props.value} placeholder={props.search ? props.label : undefined}
      accessibilityLabel={props.label} multiline={props.multiline}
      onChange={({ value }) => props.update(value)}
      onSubmit={() => props.submit?.()}
      focusStyle={{ borderColor: props.theme.primary }}
      style={{
        minWidth: 0, minHeight: props.multiline ? 112 : 32, paddingHorizontal: 8, paddingVertical: 6,
        backgroundColor: props.theme.background, color: props.theme.foreground,
        fontFamily: props.mono ? props.theme.monoFamily : props.theme.fontFamily, fontSize: props.theme.fontSize,
        borderWidth: 1, borderColor: props.theme.border, borderRadius: props.theme.radius,
      }} />
  </View>;
}

export function Choice(props: {
  theme: Theme; label: string; options: { value: string; title: string }[];
  value: string | null; choose: (value: string) => void; disabled?: boolean; fill?: boolean;
}) {
  return <View style={{ minWidth: 0, gap: 4, flexGrow: props.fill ? 1 : 0, flexBasis: props.fill ? 148 : "auto" }}>
    <Label theme={props.theme} label={props.label} muted />
    <Select items={props.options} value={props.value} accessibilityLabel={props.label}
      disabled={props.disabled || props.options.length === 0}
      onChange={({ value }) => { if (value !== null) props.choose(value); }}
      popupStyle={{ backgroundColor: props.theme.surface, borderWidth: 1, borderColor: props.theme.border, borderRadius: props.theme.radius, padding: 4 }}
      itemStyle={{ fontSize: props.theme.fontSize, color: props.theme.foreground, padding: 8, borderRadius: 4 }}
      itemClass="focus:opacity-80 selected:font-semibold"
      trigger={<View style={{
        flexDirection: "row", justifyContent: "space-between", alignItems: "center",
        minWidth: 0, minHeight: 32, paddingHorizontal: 8, gap: 8, borderRadius: props.theme.radius,
        borderWidth: 1, borderColor: props.theme.border, backgroundColor: props.theme.surface,
      }} focusStyle={{ borderColor: props.theme.primary }} hoverStyle={{ backgroundColor: props.theme.accent }}>
        <Label theme={props.theme} label={props.options.find((option) => option.value === props.value)?.title ?? "—"} lines={1} />
        <Label theme={props.theme} label="▾" muted />
      </View>}
    />
  </View>;
}

export function Check(props: { theme: Theme; label: string; checked: boolean; update: (checked: boolean) => void; disabled?: boolean }) {
  return <Checkbox checked={props.checked} disabled={props.disabled}
    onChange={({ checked }) => props.update(checked)}
    focusStyle={{ borderColor: props.theme.primary }} disabledStyle={{ opacity: 0.5 }}
    style={{ flexDirection: "row", alignItems: "center", gap: 8, minWidth: 0, minHeight: 28, borderWidth: 1, borderColor: "transparent", borderRadius: 4 }}
    indicatorStyle={{
      width: 16, height: 16, flexShrink: 0, alignItems: "center", justifyContent: "center", borderWidth: 1,
      borderColor: props.checked ? props.theme.primary : props.theme.muted, borderRadius: 4,
      backgroundColor: props.checked ? props.theme.primary : props.theme.background,
    }}
    indicator={<Text style={{ color: props.theme.onPrimary, fontSize: 12 }}>{props.checked ? "✓" : ""}</Text>}>
    <Label theme={props.theme} label={props.label} />
  </Checkbox>;
}

export function Feedback(props: { theme: Theme; pending?: boolean; error?: string; label: string }) {
  return <View style={{ gap: 4 }}>
    <If when={props.pending}>
      <Progress indeterminate accessibilityLabel={props.label}
        trackStyle={{ height: 3, backgroundColor: props.theme.border, borderRadius: 2 }}
        indicatorStyle={{ height: 3, width: "24%", backgroundColor: props.theme.primary, borderRadius: 2 }}>
        <Label theme={props.theme} label={props.label} muted />
      </Progress>
    </If>
    <If when={props.error}>{(error) => <Label theme={props.theme} label={error()} danger />}</If>
  </View>;
}

export function WorkspacePane(props: { theme: Theme; children: JSX.Element }) {
  return <View style={{
    ...viewport, flexDirection: "row", backgroundColor: props.theme.background,
  }}>{props.children}</View>;
}

export function SidebarPane(props: { theme: Theme; children: JSX.Element }) {
  return <View style={{
    width: "32%", minWidth: 112, maxWidth: 280, minHeight: 0, overflow: "hidden",
    backgroundColor: props.theme.surface, borderRightWidth: 1, borderColor: props.theme.border,
  }}>{props.children}</View>;
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
