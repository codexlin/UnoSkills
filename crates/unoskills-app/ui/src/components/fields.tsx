import { View, type InputRef } from "nsh";
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
};

export function TextInput(props: TextInputProps) {
  const embedded = () => props.variant === "embedded";
  return <Input ref={(input) => props.inputRef?.(input)} value={props.value} placeholder={props.placeholder} accessibilityLabel={props.accessibilityLabel}
    multiline={props.multiline} masked={props.masked} autoFocus={props.autoFocus} disabled={props.disabled}
    autoCorrect={props.autoCorrect} returnKeyType={props.returnKeyType} submitBehavior={props.submitBehavior}
    onChange={({ value }) => props.onChange(value)}
    onSubmit={props.onSubmit ? ({ text }) => props.onSubmit?.(text) : undefined}
    focusStyle={embedded() ? undefined : { borderColor: props.theme.primary }}
    style={{ minWidth: 0, flex: embedded() ? 1 : undefined, height: props.height,
      minHeight: props.multiline ? 112 : embedded() ? undefined : props.size === "compact" ? 28 : props.size === "large" ? 36 : 32,
      paddingHorizontal: embedded() ? 0 : 10, paddingVertical: embedded() || props.multiline ? 6 : 4,
      color: props.theme.foreground, placeholderColor: props.theme.muted,
      backgroundColor: embedded() ? "transparent" : props.variant === "surface" ? props.theme.surface : props.theme.background,
      fontFamily: props.mono ? props.theme.monoFamily ?? props.theme.fontFamily : props.theme.fontFamily,
      fontSize: embedded() ? props.theme.fontSize - 1 : props.theme.fontSize,
      borderColor: props.theme.chrome?.inputBorder ?? props.theme.border,
      borderWidth: embedded() ? 0 : 1, borderRadius: embedded() ? 0 : props.theme.radius }} />;
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
