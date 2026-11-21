import { Image, Text, View, type JSX } from "nsh";
import { iconSource, type IconName } from "../icons";
import type { Theme } from "../shared";

export function panelSurface(theme: Theme) {
  return {
    backgroundColor: theme.surface,
    borderWidth: 1,
    borderColor: theme.border,
    borderRadius: theme.panel.radius,
    boxShadow: theme.panel.shadow,
  };
}

export function Icon(props: { name: IconName; color: string; size?: number }) {
  return <Image accessibilityElementsHidden src={iconSource(props.name)}
    style={{ width: props.size ?? 16, height: props.size ?? 16, flexShrink: 0,
      pointerEvents: "none", tintColor: props.color }} />;
}

export function textSize(theme: Theme, points: number) {
  return Math.round(theme.fontSize / 0.875) * points / 16;
}

export function PageHeader(props: { theme: Theme; title: string; children?: JSX.Element }) {
  return <View style={{ flexDirection: "row", alignItems: "center", minHeight: 56, paddingHorizontal: 20,
    paddingVertical: 8, gap: 12, borderBottomWidth: 1, borderColor: props.theme.border }}>
    <View style={{ flex: 1, minWidth: 0 }}>
      <Label theme={props.theme} heading truncate size={textSize(props.theme, 20)}>{props.title}</Label>
    </View>
    {props.children ? <View style={{ flexShrink: 0 }}>{props.children}</View> : null}
  </View>;
}

export function Label(props: {
  theme: Theme;
  children?: JSX.Element;
  muted?: boolean;
  heading?: boolean;
  mono?: boolean;
  size?: number;
  color?: string;
  truncate?: boolean;
  weight?: "400" | "500" | "600";
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
        fontWeight: props.weight ?? (props.heading ? "600" : "400"),
      }}
    >{props.children}</Text>
  );
}
