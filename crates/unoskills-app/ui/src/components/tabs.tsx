import { Text, View, signal } from "nsh";
import { Tab, Tabs } from "nsh/base";
import { Icon } from "./layout";
import { focusOutline } from "./interaction";
import type { IconName } from "../icons";
import type { Theme } from "../shared";

interface TabOption<Id extends string> {
  id: Id;
  label: string;
  icon?: string;
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

export function TabList<Id extends string>(props: TabListProps<Id>) {
  const hovered = signal<Id | null>(null);
  return <Tabs accessibilityLabel={props.accessibilityLabel} class="flex-row self-start gap-1 p-1"
    style={{ borderRadius: props.theme.radius + 4, backgroundColor: props.theme.chrome?.segmented ?? props.theme.background }}
    indicatorClass="inset-y-1"
    indicatorStyle={{ borderRadius: props.theme.radius, backgroundColor: props.theme.surface }}>
    <Each of={props.tabs} key={(tab) => tab.id}>
      {(tab) => {
        const selected = () => props.selectedId === tab().id;
        const color = () => selected() ? props.theme.foreground : hovered() === tab().id
          ? `color-mix(in srgb, ${props.theme.muted} 60%, ${props.theme.foreground})`
          : props.theme.muted;
        return <Tab accessibilityLabel={tab().label} selected={selected()}
          onPress={() => { if (!selected()) props.onSelect(tab().id); }}
          onMouseEnter={() => hovered.set(tab().id)}
          onMouseLeave={() => { if (hovered() === tab().id) hovered.set(null); }}
          class="flex-row items-center justify-center gap-1 px-3"
          style={{ minHeight: props.compact ? 24 : 28, borderRadius: props.theme.radius }}
          focusVisibleStyle={focusOutline(props.theme)}>
          <If when={tab().glyph}>{(glyph) => <Icon name={glyph()} size={14} color={color()} />}</If>
          {tab().icon ? <Text accessibilityElementsHidden
            style={{ color: color(), fontSize: 13, fontFamily: props.theme.fontFamily }}>{tab().icon}</Text> : null}
          <Text style={{ color: color(), fontSize: props.theme.fontSize, fontFamily: props.theme.fontFamily,
            fontWeight: "500" }}>{tab().label}</Text>
        </Tab>;
      }}
    </Each>
  </Tabs>;
}

export function PageTabs<Id extends string>(props: TabListProps<Id> & {
  paddingHorizontal?: number;
  backgroundColor?: string;
}) {
  return <View style={{ minHeight: 36, flexShrink: 0, paddingHorizontal: props.paddingHorizontal ?? 20,
    borderBottomWidth: 1, borderColor: props.theme.border, backgroundColor: props.backgroundColor ?? props.theme.background }}>
    <TabList theme={props.theme} accessibilityLabel={props.accessibilityLabel} tabs={props.tabs}
      selectedId={props.selectedId} onSelect={props.onSelect} compact={props.compact} />
  </View>;
}
