import { Checkbox, type CheckboxProps } from "nsh/base";
import type { Theme } from "../shared";
import { disabledControlStyle, focusOutline, interactionStyles } from "./interaction";
import { Icon } from "./layout";
import { controlMotionClass } from "./motion";

type CheckControlProps = Pick<CheckboxProps, "checked" | "accessibilityLabel" | "disabled"
  | "hitSlop" | "children" | "style"> & {
  theme: Theme;
  onChange(checked: boolean): void;
};

export function CheckControl(props: CheckControlProps) {
  const states = () => interactionStyles(props.theme);
  return <Checkbox checked={props.checked} disabled={props.disabled}
    accessibilityLabel={props.accessibilityLabel} hitSlop={props.hitSlop}
    onChange={({ checked }) => props.onChange(checked)} style={props.style}
    hoverStyle={states().hoverStyle} activeStyle={states().activeStyle}
    focusVisibleStyle={focusOutline(props.theme)} disabledStyle={disabledControlStyle}
    indicatorClass={controlMotionClass}
    indicatorStyle={{
      width: 16, height: 16, flexShrink: 0, alignItems: "center", justifyContent: "center",
      borderWidth: 1, borderRadius: Math.min(4, props.theme.control.radius),
      borderColor: props.checked ? props.theme.primary : props.theme.chrome?.inputBorder ?? props.theme.border,
      backgroundColor: props.checked ? props.theme.primary : props.theme.background,
    }}
    indicator={props.checked ? <Icon name="check" color={props.theme.onPrimary} size={12} /> : null}>
    {props.children}
  </Checkbox>;
}
