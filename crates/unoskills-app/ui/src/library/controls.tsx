import { type JSX, Text, View } from "nsh";
import { Button, Tooltip } from "nsh/base";
import type { Theme } from "../shared";
import { TOKEN_BUDGET_WARN_THRESHOLD, formatTokensShort } from "./format";

export type ButtonKind = "primary" | "outline" | "ghost" | "danger";

export function hintTooltip(theme: () => Theme, text: () => string): JSX.Element {
  return (
    <Tooltip
      placement="bottom"
      offset={6}
      style={{
        borderRadius: 6,
        paddingHorizontal: 8,
        paddingVertical: 4,
        backgroundColor: theme().foreground,
      }}
    >
      <Text
        style={{
          fontSize: 12,
          color: theme().background,
          fontFamily: theme().fontFamily,
        }}
      >
        {text()}
      </Text>
    </Tooltip>
  );
}

export function ActionButton(props: {
  theme: Theme;
  label: string;
  onPress: () => void;
  kind?: ButtonKind;
  compact?: boolean;
  disabled?: boolean;
  loading?: boolean;
  selected?: boolean;
  accessibilityLabel?: string;
  tooltip?: string;
}) {
  const kind = () => props.kind ?? "outline";
  const isDisabled = () => Boolean(props.disabled || props.loading);

  const bg = () => {
    switch (kind()) {
      case "primary":
        return props.theme.primary;
      case "danger":
        return props.theme.danger;
      case "outline":
        return props.theme.surface;
      case "ghost":
      default:
        return "transparent";
    }
  };

  const fg = () => {
    switch (kind()) {
      case "primary":
      case "danger":
        return props.theme.onPrimary;
      default:
        return props.theme.foreground;
    }
  };

  const borderColor = () => {
    switch (kind()) {
      case "outline":
        return props.theme.border;
      default:
        return "transparent";
    }
  };

  const radius = () => props.theme.radius ?? 8;

  return (
    <Button
      onPress={props.onPress}
      disabled={isDisabled()}
      selected={props.selected}
      accessibilityLabel={props.accessibilityLabel ?? props.label}
      tooltip={
        props.tooltip
          ? hintTooltip(
              () => props.theme,
              () => props.tooltip ?? "",
            )
          : undefined
      }
      style={{
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "center",
        gap: 6,
        borderRadius: radius(),
        borderWidth: kind() === "outline" ? 1 : 0,
        borderColor: borderColor(),
        backgroundColor: bg(),
        paddingHorizontal: props.compact ? 10 : 12,
        paddingVertical: props.compact ? 5 : 7,
        flexShrink: 0,
      }}
      hoverStyle={{
        backgroundColor: kind() === "primary" || kind() === "danger" ? bg() : props.theme.accent,
        opacity: kind() === "primary" || kind() === "danger" ? 0.92 : 1,
      }}
      activeStyle={{
        opacity: 0.82,
      }}
      selectedStyle={{
        backgroundColor: props.theme.accent,
        borderColor: props.theme.primary,
      }}
      disabledStyle={{
        opacity: 0.48,
      }}
    >
      <Text
        style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
          fontSize: props.compact ? 12 : 13,
          fontWeight: kind() === "primary" || kind() === "danger" ? "600" : "500",
          color: fg(),
          fontFamily: props.theme.fontFamily,
        }}
      >
        {props.loading ? `${props.label}…` : props.label}
      </Text>
    </Button>
  );
}

export function PageHeader(props: {
  theme: Theme;
  title: string;
  busyText?: string | null;
  right?: JSX.Element;
}) {
  return (
    <View
      style={{
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "space-between",
        gap: 12,
        paddingHorizontal: 20,
        paddingVertical: 14,
        borderBottomWidth: 1,
        borderColor: props.theme.border,
        backgroundColor: props.theme.background,
        flexShrink: 0,
      }}
    >
      <View style={{ flex: 1, minWidth: 0, flexDirection: "row", alignItems: "center", gap: 10 }}>
        <Text
          style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
            fontSize: 18,
            fontWeight: "600",
            color: props.theme.foreground,
            fontFamily: props.theme.fontFamily,
          }}
        >
          {props.title}
        </Text>
        <If when={Boolean(props.busyText)}>
          <View
            role="summary"
            accessibilityState={{ busy: true }}
            style={{
              paddingHorizontal: 8,
              paddingVertical: 3,
              borderRadius: 999,
              backgroundColor: props.theme.accent,
              flexShrink: 1,
            }}
          >
            <Text
              style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                fontSize: 12,
                color: props.theme.muted,
                fontFamily: props.theme.fontFamily,
              }}
            >
              {props.busyText ?? ""}
            </Text>
          </View>
        </If>
      </View>
      <If when={Boolean(props.right)}>
        <View style={{ flexDirection: "row", alignItems: "center", gap: 8, flexShrink: 0 }}>
          {props.right}
        </View>
      </If>
    </View>
  );
}

export function AlertBanner(props: {
  theme: Theme;
  tone: "error" | "warning" | "info";
  message: string;
}) {
  const accentColor = () => {
    switch (props.tone) {
      case "error":
        return props.theme.danger;
      case "warning":
        return props.theme.warning;
      case "info":
      default:
        return props.theme.primary;
    }
  };

  return (
    <View
      role="alert"
      style={{
        flexDirection: "row",
        alignItems: "flex-start",
        gap: 8,
        paddingHorizontal: 12,
        paddingVertical: 10,
        borderRadius: 8,
        borderWidth: 1,
        borderColor: accentColor(),
        backgroundColor: props.theme.surface,
      }}
    >
      <Text
        style={{
          fontSize: 12,
          fontWeight: "600",
          color: accentColor(),
          fontFamily: props.theme.fontFamily,
          flexShrink: 0,
        }}
      >
        {props.tone === "error" ? "!" : props.tone === "warning" ? "▲" : "i"}
      </Text>
      <Text
        style={{
          flex: 1,
          minWidth: 0,
          fontSize: 12,
          lineHeight: 18,
          color: props.theme.foreground,
          fontFamily: props.theme.fontFamily,
        }}
      >
        {props.message}
      </Text>
    </View>
  );
}

export function EmptyState(props: {
  theme: Theme;
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

export function TokenBadge(props: { theme: Theme; tokens: number }) {
  const overBudget = () => props.tokens > TOKEN_BUDGET_WARN_THRESHOLD;
  return (
    <Text
      style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
        fontSize: 12,
        fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
        color: overBudget() ? props.theme.warning : props.theme.muted,
        flexShrink: 0,
      }}
    >
      {formatTokensShort(props.tokens)}
    </Text>
  );
}
