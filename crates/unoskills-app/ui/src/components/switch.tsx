import type { JSX } from "nsh";
import { Switch } from "nsh/base";
import type { Theme } from "../shared";
import { disabledControlStyle, focusOutline } from "./interaction";
import { controlMotionClass } from "./motion";

export function ToggleSwitch(props: {
  theme: Theme;
  accessibilityLabel: string;
  checked: boolean;
  disabled?: boolean;
  compact?: boolean;
  tooltip?: JSX.Element;
  onChange(checked: boolean): void;
}) {
  return <Switch
    accessibilityLabel={props.accessibilityLabel}
    checked={props.checked}
    disabled={props.disabled}
    tooltip={props.tooltip}
    onChange={({ checked }) => props.onChange(checked)}
    class="shrink-0 rounded-full"
    focusVisibleStyle={focusOutline(props.theme, 2)}
    disabledStyle={disabledControlStyle}
    trackClass={`rounded-full p-0.5 ${controlMotionClass}`}
    trackStyle={{
      width: props.compact ? 36 : 44,
      backgroundColor: props.checked ? props.theme.primary : props.theme.switch.offTrack,
    }}
    thumbClass={`rounded-full motion-safe:transition-transform motion-safe:duration-120 motion-safe:ease-out
      ${props.compact ? "size-4 checked:translate-x-4" : "size-5 checked:translate-x-5"}`}
    thumbStyle={{ backgroundColor: props.checked ? props.theme.switch.onThumb : props.theme.switch.offThumb }}
  />;
}
