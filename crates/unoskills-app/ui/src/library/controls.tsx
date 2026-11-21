import { type InputRef, type JSX, Text, View } from "nsh";
import { Action, ActionControl, Hint, Icon, Label, TextInput, type ActionVariant } from "../components";
import type { IconName } from "../icons";
import type { Theme } from "../shared";
import { TOKEN_BUDGET_WARN_THRESHOLD, formatTokensShort } from "./format";

type ButtonKind = Exclude<ActionVariant, "danger-outline">;

export function hintTooltip(theme: () => Theme, text: () => string): JSX.Element {
  return <Hint theme={theme()} text={text()} />;
}

export function ActionButton(props: {
  theme: Theme;
  label?: string;
  glyph?: IconName;
  onPress: () => void;
  kind?: ButtonKind;
  compact?: boolean;
  disabled?: boolean;
  loading?: boolean;
  selected?: boolean;
  accessibilityLabel?: string;
  tooltip?: string;
}) {
  return <Action theme={props.theme} label={props.label ?? ""} glyph={props.glyph}
    accessibilityLabel={props.accessibilityLabel ?? props.label ?? ""}
    variant={props.kind ?? "outline"} size={props.compact ? "compact" : "regular"} iconOnly={!props.label}
    disabled={props.disabled} pending={props.loading} selected={props.selected} tooltip={props.tooltip}
    onPress={props.onPress} />;
}

export function SearchField(props: {
  theme: Theme;
  value: string;
  placeholder: string;
  accessibilityLabel?: string;
  clearLabel: string;
  inputRef?: (ref: InputRef) => void;
  minWidth?: number;
  maxWidth?: number;
  returnKeyType?: "search" | "done" | "default";
  submitBehavior?: "submit" | "blurAndSubmit" | "newline";
  onChange: (value: string) => void;
  onSubmit?: (text: string) => void;
  onClear?: () => void;
}) {
  return (
    <View
      class="flex-row items-center"
      style={{
        flex: 1,
        minWidth: props.minWidth ?? 160,
        maxWidth: props.maxWidth ?? 280,
        minHeight: 34,
        borderRadius: props.theme.radius ?? 8,
        borderWidth: 1,
        borderColor: props.theme.border,
        backgroundColor: props.theme.background,
        paddingLeft: 10,
        paddingRight: props.value.length > 0 ? 4 : 10,
        gap: 4,
      }}
      focusWithinStyle={{ borderColor: props.theme.primary }}
    >
      <TextInput theme={props.theme} variant="embedded" inputRef={props.inputRef} value={props.value}
        onChange={props.onChange} onSubmit={props.onSubmit} placeholder={props.placeholder}
        accessibilityLabel={props.accessibilityLabel ?? props.placeholder}
        returnKeyType={props.returnKeyType} submitBehavior={props.submitBehavior} />
      <If when={props.value.length > 0}>
        <ActionControl theme={props.theme}
          accessibilityLabel={props.clearLabel}
          onPress={() => {
            if (props.onClear) {
              props.onClear();
            } else {
              props.onChange("");
            }
          }}
          style={{
            width: 22,
            height: 22,
            borderRadius: 4,
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Icon name="x" size={12} color={props.theme.muted} />
        </ActionControl>
      </If>
    </View>
  );
}

export function PageHeader(props: {
  theme: Theme;
  title: string;
  right?: JSX.Element;
}) {
  return (
    <View
      style={{
        minHeight: 56,
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "space-between",
        gap: 12,
        paddingHorizontal: 20,
        paddingVertical: 8,
        borderBottomWidth: 1,
        borderColor: props.theme.border,
        backgroundColor: props.theme.background,
        flexShrink: 0,
      }}
    >
      <View style={{ flex: 1, minWidth: 0 }}>
        <Label theme={props.theme} heading size={20} truncate>
          {props.title}
        </Label>
      </View>
      <If when={Boolean(props.right)}>
        <View style={{ flexDirection: "row", alignItems: "center", gap: 8, flexShrink: 0 }}>
          {props.right}
        </View>
      </If>
    </View>
  );
}

export function EmptyState(props: {
  theme: Theme;
  glyph?: IconName;
  title: string;
  description: string;
  actions?: JSX.Element;
}) {
  return (
    <View
      style={{
        flex: 1,
        alignItems: "center",
        justifyContent: "center",
        padding: 24,
        gap: 8,
      }}
    >
      <If when={props.glyph}>{(glyph) =>
        <View
          accessibilityElementsHidden
          style={{
            width: 44,
            height: 44,
            borderRadius: Math.max((props.theme.radius ?? 8) + 2, 10),
            borderWidth: 1,
            borderColor: props.theme.border,
            backgroundColor: props.theme.surface,
            alignItems: "center",
            justifyContent: "center",
            marginBottom: 4,
          }}
        >
          <Icon name={glyph()} size={20} color={props.theme.muted} />
        </View>
      }</If>
      <Text
        style={{
          fontSize: 15,
          fontWeight: "600",
          color: props.theme.foreground,
          fontFamily: props.theme.fontFamily,
          textAlign: "center",
        }}
      >
        {props.title}
      </Text>
      <Text
        style={{
          fontSize: 13,
          lineHeight: 19,
          color: props.theme.muted,
          fontFamily: props.theme.fontFamily,
          textAlign: "center",
          maxWidth: 360,
        }}
      >
        {props.description}
      </Text>
      <If when={Boolean(props.actions)}>
        <View
          style={{
            flexDirection: "row",
            flexWrap: "wrap",
            alignItems: "center",
            justifyContent: "center",
            gap: 8,
            marginTop: 8,
          }}
        >
          {props.actions}
        </View>
      </If>
    </View>
  );
}

export function TokenBadge(props: { theme: Theme; tokens: number; selected?: boolean }) {
  const overBudget = () => props.tokens > TOKEN_BUDGET_WARN_THRESHOLD;
  const ink = () => props.selected
    ? overBudget() ? `color-mix(in oklab, ${props.theme.warning} 32%, ${props.theme.onAccent})` : props.theme.onAccent
    : overBudget() ? props.theme.warning : props.theme.muted;
  return (
    <Label
      theme={props.theme}
      mono
      size={12}
      truncate
      color={ink()}
    >
      {formatTokensShort(props.tokens)}
    </Label>
  );
}
