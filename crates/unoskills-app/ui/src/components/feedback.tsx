import { View, type JSX } from "nsh";
import { Progress, Tooltip } from "nsh/base";
import type { Theme } from "../shared";
import { Label, textSize } from "./layout";

export function Hint(props: { theme: Theme; text: string; placement?: "bottom" | "right" }) {
  return <Tooltip placement={props.placement ?? "bottom"} offset={6} style={{
    paddingHorizontal: 8, paddingVertical: 4, borderRadius: props.theme.radius, backgroundColor: props.theme.foreground,
  }}>
    <Label theme={props.theme} size={textSize(props.theme, 12)} color={props.theme.background}>{props.text}</Label>
  </Tooltip>;
}

const progressWidths = { indicator: 16, compact: 40, loading: 56, full: undefined };

export function ProgressBar(props: {
  theme: Theme; label: string; value?: number | null; onPrimary?: boolean;
  size?: keyof typeof progressWidths; children?: JSX.Element;
}) {
  const pending = () => props.value == null;
  const width = () => progressWidths[props.size ?? "full"];
  return <Progress value={props.value ?? 0} indeterminate={pending()} accessibilityLabel={props.label}
    style={{ flexDirection: "row", alignItems: "center", gap: 8, flexShrink: 1 }}
    trackStyle={{ width: width() ?? "100%", height: 4, borderRadius: 2, flexShrink: 0, overflow: "hidden",
      backgroundColor: props.theme.border }}
    indicatorClass={pending() ? "h-full w-1/3 animate-pulse motion-reduce:animate-none" : "h-full"}
    indicatorStyle={{ borderRadius: 2, backgroundColor: props.onPrimary ? props.theme.onPrimary : props.theme.primary }}>
    {props.children}
  </Progress>;
}

export function LoadingLine(props: { theme: Theme; label: string; compact?: boolean }) {
  return <ProgressBar theme={props.theme} label={props.label} size={props.compact ? "compact" : "loading"}>
    <Label theme={props.theme} muted size={props.compact ? 12 : 13} truncate>{props.label}</Label>
  </ProgressBar>;
}

type FeedbackTone = "error" | "warning" | "info" | "success";

export function AlertBanner(props: { theme: Theme; tone: FeedbackTone; message: string }) {
  const color = () => props.tone === "error" ? props.theme.danger : props.tone === "warning"
    ? props.theme.warning : props.tone === "success" ? props.theme.success : props.theme.primary;
  return <View role={props.tone === "success" ? "summary" : "alert"}
    style={{ minWidth: 0, padding: 8, backgroundColor: props.theme.surface,
      borderRadius: props.theme.radius, borderColor: color(), borderWidth: 1 }}>
    <Label theme={props.theme} color={props.tone === "error" ? color() : undefined}>{props.message}</Label>
  </View>;
}

export function InlineError(props: { theme: Theme; message: string }) {
  return <AlertBanner theme={props.theme} tone="error" message={props.message} />;
}
