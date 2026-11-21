import { signal, View } from "nsh";
import { Button } from "nsh/base";
import type { IconName } from "../icons";
import type { Theme } from "../shared";
import { Hint } from "./feedback";
import { focusOutline } from "./interaction";
import { Icon, Label, textSize } from "./layout";

export function NavigationControl(props: {
  theme: Theme;
  label: string;
  glyph: IconName;
  selected: boolean;
  onPress: () => void;
  sidebar?: boolean;
  collapsed?: boolean;
  height?: number;
  glyphSize?: number;
  badge?: string;
}) {
  const hovered = signal(false);
  const pressed = signal(false);
  const sidebar = () => props.sidebar ? props.theme.sidebar : undefined;
  const interacting = () => hovered() || pressed();
  const highlighted = () => props.selected || interacting();
  const selectedStyle = () => ({ backgroundColor: props.theme.primary });
  const interactionStyle = () => props.selected ? selectedStyle()
    : { backgroundColor: sidebar()?.accent ?? props.theme.accent };
  const ink = () => props.selected ? props.theme.onPrimary
    : interacting() ? sidebar()?.onAccent ?? props.theme.onAccent
    : sidebar()?.foreground ?? props.theme.foreground;
  const iconInk = () => highlighted() || props.sidebar ? ink() : props.theme.muted;
  const badgeInk = () => highlighted() ? ink() : props.theme.warning;

  return <Button accessibilityLabel={props.badge ? `${props.label} ${props.badge}` : props.label}
    selected={props.selected} onPress={props.onPress}
    onMouseEnter={() => hovered.set(true)} onMouseLeave={() => hovered.set(false)}
    onPressDown={() => pressed.set(true)} onPressUp={() => pressed.set(false)}
    hoverStyle={interactionStyle()} activeStyle={interactionStyle()} selectedStyle={selectedStyle()}
    focusVisibleStyle={{ ...focusOutline(props.theme, 1), outlineColor: sidebar()?.ring ?? props.theme.ring }}
    tooltip={props.collapsed ? <Hint theme={props.theme} text={props.label} placement="right" /> : undefined}
    style={{ width: "100%", minWidth: 0, height: props.collapsed ? 32 : props.height ?? 32,
      flexDirection: "row", alignItems: "center", justifyContent: props.collapsed ? "center" : "flex-start",
      paddingHorizontal: props.collapsed ? 0 : 8, gap: 8, borderRadius: props.theme.control.radius }}>
    <Icon name={props.glyph} color={iconInk()} size={props.glyphSize ?? 16} />
    {!props.collapsed ? <>
      <View style={{ flex: 1, minWidth: 0 }}>
        <Label theme={props.theme} color={ink()} truncate weight={props.selected ? "600" : "400"}>{props.label}</Label>
      </View>
      {props.badge ? <View style={{ borderColor: badgeInk(), borderWidth: 1, paddingHorizontal: 4,
        borderRadius: Math.min(4, props.theme.radius), flexShrink: 0 }}>
        <Label theme={props.theme} size={textSize(props.theme, 12)} color={badgeInk()}>{props.badge}</Label>
      </View> : null}
    </> : null}
  </Button>;
}
