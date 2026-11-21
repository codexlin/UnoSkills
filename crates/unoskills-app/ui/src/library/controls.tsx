import { type InputRef, type JSX, Text, View } from "nsh";
import { Button, Input, Progress, Tabs, Tooltip } from "nsh/base";
import { InlineError, Label, TabItem, focusOutline, interactionStyles } from "../controls";
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
      <Label theme={theme()} size={12} color={theme().background}>
        {text()}
      </Label>
    </Tooltip>
  );
}

export function ActionButton(props: {
  theme: Theme;
  label?: string;
  icon?: string;
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
  const radius = () => props.theme.radius ?? 8;
  const hasLabel = () => Boolean(props.label && props.label.length > 0);

  const bg = () => {
    switch (kind()) {
      case "primary":
        return props.theme.primary;
      case "danger":
        return props.theme.danger;
      case "outline":
        return props.theme.background;
      case "ghost":
      default:
        return "transparent";
    }
  };

  const fg = () => {
    switch (kind()) {
      case "primary":
        return props.theme.onPrimary;
      case "danger":
        return props.theme.background;
      default:
        return props.theme.foreground;
    }
  };

  const iconColor = () => {
    switch (kind()) {
      case "primary":
        return props.theme.onPrimary;
      case "danger":
        return props.theme.background;
      case "ghost":
        return hasLabel() ? props.theme.muted : props.theme.foreground;
      default:
        return props.theme.muted;
    }
  };

  const states = () => interactionStyles(props.theme, {
    primary: kind() === "primary",
    selectedFill: kind() === "danger" ? props.theme.danger : props.selected ? props.theme.accent : undefined,
  });
  return (
    <Button
      onPress={props.onPress}
      disabled={isDisabled()}
      selected={props.selected}
      accessibilityLabel={props.accessibilityLabel ?? props.label ?? props.icon ?? ""}
      tooltip={
        props.tooltip
          ? hintTooltip(
              () => props.theme,
              () => props.tooltip ?? "",
            )
          : undefined
      }
      class="transition-colors"
      style={{
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "center",
        gap: 6,
        minHeight: props.compact ? 28 : 34,
        minWidth: hasLabel() ? undefined : props.compact ? 28 : 34,
        borderRadius: radius(),
        borderWidth: kind() === "outline" ? 1 : 0,
        borderColor: kind() === "outline" ? props.theme.border : "transparent",
        backgroundColor: bg(),
        paddingHorizontal: hasLabel() ? (props.compact ? 10 : 12) : props.compact ? 6 : 8,
        paddingVertical: props.compact ? 4 : 6,
        flexShrink: 0,
      }}
      hoverStyle={states().hoverStyle}
      activeStyle={states().activeStyle}
      focusVisibleStyle={focusOutline(props.theme)}
      selectedStyle={{
        backgroundColor: props.theme.accent,
        borderColor: props.theme.primary,
      }}
      disabledStyle={{ opacity: 0.48 }}
    >
      <If when={Boolean(props.icon)}>
        <Text
          accessibilityElementsHidden={hasLabel()}
          style={{
            fontSize: props.compact ? 12 : 13,
            color: iconColor(),
            fontFamily: props.theme.fontFamily,
          }}
        >
          {props.loading && !hasLabel() ? "…" : (props.icon ?? "")}
        </Text>
      </If>
      <If when={hasLabel()}>
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
          {props.loading ? `${props.label ?? ""}…` : (props.label ?? "")}
        </Text>
      </If>
    </Button>
  );
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
      class="flex-row items-center transition-colors"
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
      <Input
        ref={props.inputRef}
        value={props.value}
        onChange={({ value }) => props.onChange(value)}
        onSubmit={props.onSubmit ? ({ text }) => props.onSubmit?.(text) : undefined}
        placeholder={props.placeholder}
        accessibilityLabel={props.accessibilityLabel ?? props.placeholder}
        returnKeyType={props.returnKeyType}
        submitBehavior={props.submitBehavior}
        style={{
          flex: 1,
          minWidth: 0,
          fontSize: 13,
          paddingVertical: 6,
          color: props.theme.foreground,
          placeholderColor: props.theme.muted,
          fontFamily: props.theme.fontFamily,
        }}
      />
      <If when={props.value.length > 0}>
        <Button
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
          hoverStyle={interactionStyles(props.theme).hoverStyle}
          activeStyle={interactionStyles(props.theme).activeStyle}
          focusVisibleStyle={focusOutline(props.theme)}
        >
          <Text style={{ fontSize: 12, color: props.theme.muted }}>×</Text>
        </Button>
      </If>
    </View>
  );
}

export function LoadingLine(props: {
  theme: Theme;
  label: string;
  compact?: boolean;
}) {
  return (
    <View
      style={{
        flexDirection: "row",
        alignItems: "center",
        gap: 8,
        flexShrink: 1,
      }}
    >
      <Progress
        indeterminate
        accessibilityLabel={props.label}
        style={{ flexDirection: "row", alignItems: "center", gap: 8 }}
        trackStyle={{
          width: props.compact ? 40 : 56,
          height: 4,
          borderRadius: 999,
          overflow: "hidden",
          backgroundColor: props.theme.border,
          flexShrink: 0,
        }}
        indicatorClass="h-full w-1/3 rounded-full animate-pulse motion-reduce:animate-none"
        indicatorStyle={{
          backgroundColor: props.theme.primary,
        }}
      >
        <Label theme={props.theme} muted size={props.compact ? 12 : 13} truncate>
          {props.label}
        </Label>
      </Progress>
    </View>
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
      <View style={{ flex: 1, minWidth: 0, flexDirection: "row", alignItems: "center", gap: 12 }}>
        <Label theme={props.theme} heading size={20} truncate>
          {props.title}
        </Label>
        <If when={Boolean(props.busyText)}>
          <View
            style={{
              paddingHorizontal: 8,
              paddingVertical: 3,
              borderRadius: 999,
              backgroundColor: props.theme.surface,
              borderWidth: 1,
              borderColor: props.theme.border,
              flexShrink: 1,
            }}
          >
            <LoadingLine theme={props.theme} compact label={props.busyText ?? ""} />
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

export interface PageTabItem {
  id: string;
  icon: string;
  label: string;
}

export function PageTabs(props: {
  theme: Theme;
  accessibilityLabel: string;
  tabs: PageTabItem[];
  selectedId: string;
  paddingHorizontal?: number;
  backgroundColor?: string;
  onSelect: (id: string) => void;
}) {
  return (
    <View
      style={{
        minHeight: 36,
        paddingHorizontal: props.paddingHorizontal ?? 20,
        borderBottomWidth: 1,
        borderColor: props.theme.border,
        backgroundColor: props.backgroundColor ?? props.theme.background,
        flexShrink: 0,
      }}
    >
      <Tabs
        accessibilityLabel={props.accessibilityLabel}
        style={{ flexDirection: "row", gap: 18 }}
        indicatorStyle={{ bottom: 0, height: 2, backgroundColor: props.theme.primary }}
      >
        <Each of={props.tabs} key={(tab) => tab.id}>
          {(tab) => (
            <TabItem
              theme={props.theme}
              selected={props.selectedId === tab().id}
              onPress={() => props.onSelect(tab().id)}
              class="flex-row items-center gap-1.5 py-2"
              style={{ borderRadius: props.theme.radius }}
            >
              <Text
                accessibilityElementsHidden
                style={{
                  fontSize: 13,
                  color:
                    props.selectedId === tab().id ? props.theme.foreground : props.theme.muted,
                  fontFamily: props.theme.fontFamily,
                }}
              >
                {tab().icon}
              </Text>
              <Text
                style={{
                  fontSize: 13,
                  fontWeight: props.selectedId === tab().id ? "600" : "500",
                  color:
                    props.selectedId === tab().id ? props.theme.foreground : props.theme.muted,
                  fontFamily: props.theme.fontFamily,
                }}
              >
                {tab().label}
              </Text>
            </TabItem>
          )}
        </Each>
      </Tabs>
    </View>
  );
}

export function AlertBanner(props: {
  theme: Theme;
  tone: "error" | "warning" | "info";
  message: string;
}) {
  return (
    <If
      when={props.tone === "error"}
      else={
        <View
          role="alert"
          style={{
            flexDirection: "row",
            alignItems: "flex-start",
            gap: 8,
            paddingHorizontal: 12,
            paddingVertical: 10,
            borderRadius: props.theme.radius ?? 8,
            borderWidth: 1,
            borderColor: props.tone === "warning" ? props.theme.warning : props.theme.primary,
            backgroundColor: props.theme.surface,
          }}
        >
          <Label
            theme={props.theme}
            size={12}
            color={props.tone === "warning" ? props.theme.warning : props.theme.primary}
          >
            {props.tone === "warning" ? "▲" : "i"}
          </Label>
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
      }
    >
      <InlineError theme={props.theme} message={props.message} />
    </If>
  );
}

export function EmptyState(props: {
  theme: Theme;
  icon?: string;
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
      <If when={Boolean(props.icon)}>
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
          <Text
            style={{
              fontSize: 20,
              color: props.theme.muted,
              fontFamily: props.theme.fontFamily,
            }}
          >
            {props.icon ?? ""}
          </Text>
        </View>
      </If>
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
    <Label
      theme={props.theme}
      mono
      size={12}
      truncate
      color={overBudget() ? props.theme.warning : props.theme.muted}
    >
      {formatTokensShort(props.tokens)}
    </Label>
  );
}
