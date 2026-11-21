import { View, cleanup, effect, keymap, signal, type InputRef } from "nsh";
import { Input, type InputProps } from "nsh/base";
import type { Theme } from "../shared";
import { Label } from "./layout";

type TextInputProps = Pick<InputProps, "value" | "placeholder" | "accessibilityLabel" | "multiline" | "masked"
  | "autoFocus" | "disabled" | "autoCorrect" | "returnKeyType" | "submitBehavior"> & {
  theme: Theme;
  size?: "compact" | "regular" | "large";
  variant?: "default" | "embedded" | "surface";
  mono?: boolean;
  height?: number;
  inputRef?: (ref: InputRef) => void;
  onChange: (value: string) => void;
  onSubmit?: (value: string) => void;
  onEscape?: () => void;
};

export function TextInput(props: TextInputProps) {
  const embedded = () => props.variant === "embedded";
  const focused = signal(false);
  let input: InputRef | undefined;
  effect(() => {
    if (focused()) {
      cleanup(keymap.bind("escape", () => {
        if (props.onEscape) props.onEscape();
        else input?.blur();
      }, { when: "always", forFocus: true }));
    }
  });
  return <Input ref={(ref) => { input = ref; props.inputRef?.(ref); }}
    value={props.value} placeholder={props.placeholder} accessibilityLabel={props.accessibilityLabel}
    multiline={props.multiline} masked={props.masked} autoFocus={props.autoFocus} disabled={props.disabled}
    autoCorrect={props.autoCorrect} returnKeyType={props.returnKeyType} submitBehavior={props.submitBehavior}
    onFocus={() => focused.set(true)} onBlur={() => focused.set(false)}
    onChange={({ value }) => props.onChange(value)}
    onSubmit={props.onSubmit ? ({ text }) => props.onSubmit?.(text) : undefined}
    focusStyle={embedded() ? undefined : { borderColor: props.theme.ring }}
    style={{ minWidth: 0, flex: embedded() ? 1 : undefined, height: props.height,
      minHeight: props.multiline ? 112 : embedded() ? undefined : props.size === "compact" ? 28 : 36,
      paddingHorizontal: embedded() ? 0 : 12, paddingVertical: embedded() || props.multiline ? 6 : 4,
      color: props.theme.foreground, placeholderColor: props.theme.muted,
      backgroundColor: embedded() ? "transparent" : props.variant === "surface" ? props.theme.surface : props.theme.control.inputFill,
      fontFamily: props.mono ? props.theme.monoFamily ?? props.theme.fontFamily : props.theme.fontFamily,
      fontSize: embedded() ? props.theme.fontSize - 1 : props.theme.fontSize,
      borderColor: props.theme.chrome?.inputBorder ?? props.theme.border,
      boxShadow: embedded() ? undefined : props.theme.control.shadow,
      borderWidth: embedded() ? 0 : 1, borderRadius: embedded() ? 0 : props.theme.control.radius }} />;
}

export function FormText(props: {
  theme: Theme; title: string; detail?: string; value: string; placeholder?: string;
  masked?: boolean; autoFocus?: boolean; inline?: boolean;
  onChange: (text: string) => void; onSubmit?: () => void;
}) {
  return <View style={{ gap: 4, minWidth: 0 }}>
    {!props.inline ? <Label theme={props.theme}>{props.title}</Label> : null}
    {!props.inline && props.detail ? <Label theme={props.theme} muted>{props.detail}</Label> : null}
    <TextInput theme={props.theme} value={props.value} accessibilityLabel={props.title} placeholder={props.placeholder}
      masked={props.masked} autoFocus={props.autoFocus} onChange={props.onChange} onSubmit={props.onSubmit} />
  </View>;
}
