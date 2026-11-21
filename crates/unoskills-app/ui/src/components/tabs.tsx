import { Text, View } from "nsh";
import { Tab, Tabs } from "nsh/base";
import { Icon } from "./layout";
import { focusOutline } from "./interaction";
import { controlMotionClass } from "./motion";
import type { IconName } from "../icons";
import type { Theme } from "../shared";

interface TabOption<Id extends string> {
  id: Id;
  label: string;
  accessibilityLabel?: string;
  count?: number;
  disabled?: boolean;
  glyph?: IconName;
}

interface TabListProps<Id extends string> {
  theme: Theme;
  accessibilityLabel: string;
  tabs: readonly TabOption<Id>[];
  selectedId: Id;
  onSelect: (id: Id) => void;
  compact?: boolean;
}

const tabInset = 8;

export function TabList<Id extends string>(props: TabListProps<Id>) {
  return <View style={{ marginHorizontal: -tabInset }}>
    <View style={{ pointerEvents: "none", position: "absolute", left: 0, right: 0, bottom: 0,
      height: 1, backgroundColor: props.theme.border }} />
    <Tabs accessibilityLabel={props.accessibilityLabel} style={{ flexDirection: "row", gap: 2, paddingTop: 3 }}
      indicatorStyle={{ bottom: 0, height: 2, borderRadius: 1, backgroundColor: props.theme.primary }}>
      <Each of={props.tabs} key={(tab) => tab.id}>
        {(tab) => {
          const selected = () => props.selectedId === tab().id;
          const color = () => selected() ? props.theme.foreground : props.theme.muted;
          return <Tab accessibilityLabel={tab().accessibilityLabel ?? tab().label} selected={selected()}
            disabled={tab().disabled} onPress={() => { if (!selected()) props.onSelect(tab().id); }}
            class={controlMotionClass}
            style={{
              minHeight: props.compact ? 24 : 30, paddingHorizontal: tabInset, marginBottom: 4,
              flexDirection: "row", alignItems: "center", justifyContent: "center", gap: 6,
              borderRadius: props.theme.control.radius, borderWidth: 1, borderColor: "transparent",
            }}
            hoverStyle={selected() || tab().disabled ? undefined
              : { backgroundColor: props.theme.chrome?.hover ?? props.theme.accent }}
            activeStyle={{ opacity: 0.7 }} disabledStyle={{ opacity: 0.5 }}
            focusVisibleStyle={focusOutline(props.theme)}>
            <If when={tab().glyph}>{(glyph) => <Icon name={glyph()} size={14} color={color()} />}</If>
            <Text style={{ color: color(), fontSize: props.theme.fontSize, fontFamily: props.theme.fontFamily,
              fontWeight: "500" }}>{tab().label}</Text>
            {(tab().count ?? 0) > 0 ? <View accessibilityElementsHidden
              style={{ minWidth: 18, minHeight: 16, paddingHorizontal: 5, borderRadius: 8, alignItems: "center",
                justifyContent: "center", backgroundColor: props.theme.chrome?.segmented ?? props.theme.background,
                borderWidth: 1, borderColor: props.theme.border }}>
              <Text style={{ fontSize: Math.max(10, props.theme.fontSize - 4), fontWeight: "600",
                fontFamily: props.theme.fontFamily, color: props.theme.muted, fontVariant: ["tabular-nums"] }}>
                {tab().count}
              </Text>
            </View> : null}
          </Tab>;
        }}
      </Each>
    </Tabs>
  </View>;
}

export function PageTabs<Id extends string>(props: TabListProps<Id> & {
  paddingHorizontal?: number;
  backgroundColor?: string;
}) {
  return <View style={{ flexShrink: 0, paddingHorizontal: props.paddingHorizontal ?? 20,
    backgroundColor: props.backgroundColor ?? props.theme.background }}>
    <TabList theme={props.theme} accessibilityLabel={props.accessibilityLabel} tabs={props.tabs}
      selectedId={props.selectedId} onSelect={props.onSelect} compact={props.compact} />
  </View>;
}
