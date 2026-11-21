import { type InputRef, List, type ListRef, Text, View, openUrl } from "nsh";
import { Button, Input, Menu, MenuItem, Select } from "nsh/base";
import { Label } from "../controls";
import type { Theme } from "../shared";
import { ActionButton, AlertBanner, EmptyState, hintTooltip } from "./controls";
import { formatInstallCount, marketInstallStatus } from "./format";
import { type Locale, boardLabel, getStrings } from "./strings";
import type { MarketBoard, MarketplaceSkillItem } from "./types";

export const ALL_SOURCES_VALUE = "__all_sources__";
export const MARKET_BOARDS: MarketBoard[] = ["trending", "hot", "all-time"];

export interface RankedMarketEntry {
  item: MarketplaceSkillItem;
  rankIndex: number;
}

export function MarketplaceTab(props: {
  locale: Locale;
  theme: Theme;
  searchQuery: string;
  searchActive: boolean;
  board: MarketBoard;
  sourceFilter: string | null;
  sourceSelectItems: Array<{ value: string; title: string }>;
  totalItemsCount: number;
  entries: RankedMarketEntry[];
  countBadge: string;
  loading: boolean;
  loadingMore: boolean;
  loadMoreFailed: boolean;
  showFooter: boolean;
  error: string | null;
  installedSourceRefs: ReadonlySet<string>;
  installedSkillNames: ReadonlySet<string>;
  installingRefs: Record<string, boolean>;
  onSearchInputRef: (ref: InputRef) => void;
  onListRef: (ref: ListRef) => void;
  onSearchChange: (value: string) => void;
  onSearchSubmit: (query: string) => void;
  onClearSearch: () => void;
  onSelectBoard: (board: MarketBoard) => void;
  onSelectSourceFilter: (source: string | null) => void;
  onRefresh: () => void;
  onLoadMore: () => void;
  onItemAction: (item: MarketplaceSkillItem) => void;
}) {
  const s = () => getStrings(props.locale);

  const actionLabelFor = (item: MarketplaceSkillItem): string => {
    const status = marketInstallStatus(item, props.installedSourceRefs, props.installedSkillNames);
    if (status === "installed") {
      return s().market_open_skill;
    }
    if (status === "same_name") {
      return s().market_same_name;
    }
    return s().market_add_to_library;
  };

  const actionTooltipFor = (item: MarketplaceSkillItem): string => {
    const status = marketInstallStatus(item, props.installedSourceRefs, props.installedSkillNames);
    if (status === "installed") {
      return s().already_in_library;
    }
    if (status === "same_name") {
      return s().market_same_name;
    }
    return s().market_add_to_library;
  };

  return (
    <View style={{ flex: 1, minHeight: 0, minWidth: 0 }}>
      {/* Marketplace Toolbar */}
      <View
        style={{
          flexDirection: "row",
          flexWrap: "wrap",
          alignItems: "center",
          gap: 8,
          paddingHorizontal: 20,
          paddingVertical: 10,
          flexShrink: 0,
        }}
      >
        <View
          style={{
            flex: 1,
            minWidth: 160,
            maxWidth: 360,
            flexDirection: "row",
            alignItems: "center",
            gap: 4,
          }}
        >
          <Input
            ref={props.onSearchInputRef}
            value={props.searchQuery}
            onChange={({ value }) => props.onSearchChange(value)}
            onSubmit={({ text }) => props.onSearchSubmit(text)}
            placeholder={s().ph_market_search}
            accessibilityLabel={s().ph_market_search}
            returnKeyType="search"
            submitBehavior="submit"
            style={{
              flex: 1,
              fontSize: 13,
              paddingHorizontal: 10,
              paddingVertical: 6,
              borderRadius: props.theme.radius ?? 8,
              borderWidth: 1,
              borderColor: props.theme.border,
              backgroundColor: props.theme.surface,
              color: props.theme.foreground,
              placeholderColor: props.theme.muted,
              fontFamily: props.theme.fontFamily,
            }}
          />
          <If when={props.searchQuery.length > 0}>
            <ActionButton
              theme={props.theme}
              kind="ghost"
              compact
              label="×"
              accessibilityLabel={s().adopt_clear_selection}
              onPress={props.onClearSearch}
            />
          </If>
        </View>

        <ActionButton
          theme={props.theme}
          kind="ghost"
          label={s().btn_search}
          tooltip={s().market_search_hint}
          onPress={() => props.onSearchSubmit(props.searchQuery)}
        />

        {/* Board Picker Menu */}
        <Menu
          offset={4}
          popupStyle={{
            minWidth: 170,
            borderRadius: 10,
            borderWidth: 1,
            borderColor: props.theme.border,
            backgroundColor: props.theme.surface,
            padding: 4,
          }}
          trigger={
            <View
              accessibilityLabel={s().market_sort}
              tooltip={hintTooltip(
                () => props.theme,
                () => s().market_sort,
              )}
              style={{
                flexDirection: "row",
                alignItems: "center",
                gap: 6,
                paddingHorizontal: 12,
                paddingVertical: 6,
                borderRadius: props.theme.radius ?? 8,
                borderWidth: 1,
                borderColor: props.theme.border,
                backgroundColor: props.theme.surface,
              }}
            >
              <Label theme={props.theme} size={13}>
                {props.searchActive
                  ? s().market_search_results
                  : boardLabel(props.locale, props.board)}
              </Label>
              <Text style={{ fontSize: 11, color: props.theme.muted }}>▾</Text>
            </View>
          }
        >
          <Each of={MARKET_BOARDS} key={(board) => board}>
            {(board) => (
              <MenuItem
                role="menuitemradio"
                checked={!props.searchActive && props.board === board()}
                onSelect={() => props.onSelectBoard(board())}
                style={{
                  flexDirection: "row",
                  alignItems: "center",
                  gap: 8,
                  paddingHorizontal: 10,
                  paddingVertical: 7,
                  borderRadius: 6,
                }}
                focusStyle={{ backgroundColor: props.theme.accent }}
                hoverStyle={{ backgroundColor: props.theme.accent }}
              >
                <Text style={{ width: 14, fontSize: 12, color: props.theme.primary }}>
                  {!props.searchActive && props.board === board() ? "✓" : ""}
                </Text>
                <Label theme={props.theme} size={13}>
                  {boardLabel(props.locale, board())}
                </Label>
              </MenuItem>
            )}
          </Each>
        </Menu>

        {/* Source Filter Select (searchable) */}
        <View style={{ flexDirection: "row", alignItems: "center", gap: 4, flexShrink: 0 }}>
          <Select
            items={props.sourceSelectItems}
            value={props.sourceFilter ?? ALL_SOURCES_VALUE}
            searchable
            searchPlaceholder={s().market_source_search}
            searchAccessibilityLabel={s().market_source_search}
            accessibilityLabel={s().market_source_label}
            disabled={props.totalItemsCount === 0}
            offset={4}
            onChange={({ value }) => {
              props.onSelectSourceFilter(!value || value === ALL_SOURCES_VALUE ? null : value);
            }}
            popupStyle={{
              width: 288,
              maxHeight: 300,
              borderRadius: 10,
              borderWidth: 1,
              borderColor: props.theme.border,
              backgroundColor: props.theme.surface,
              padding: 4,
            }}
            searchStyle={{
              margin: 4,
              paddingHorizontal: 8,
              paddingVertical: 6,
              borderRadius: 6,
              borderWidth: 1,
              borderColor: props.theme.border,
              backgroundColor: props.theme.background,
              color: props.theme.foreground,
              placeholderColor: props.theme.muted,
              fontSize: 12,
            }}
            itemStyle={{
              paddingHorizontal: 10,
              paddingVertical: 7,
              borderRadius: 6,
              fontSize: 13,
              color: props.theme.foreground,
              fontFamily: props.theme.fontFamily,
            }}
            trigger={
              <View
                style={{
                  width: 176,
                  flexDirection: "row",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: 6,
                  paddingHorizontal: 10,
                  paddingVertical: 6,
                  borderRadius: props.theme.radius ?? 8,
                  borderWidth: 1,
                  borderColor: props.theme.border,
                  backgroundColor: props.theme.surface,
                }}
              >
                <Text
                  style={{
                    numberOfLines: 1,
                    textOverflow: "ellipsis",
                    flex: 1,
                    minWidth: 0,
                    fontSize: 13,
                    color: props.theme.foreground,
                    fontFamily: props.theme.fontFamily,
                  }}
                >
                  {props.sourceFilter ?? s().market_all_sources}
                </Text>
                <Text style={{ fontSize: 11, color: props.theme.muted }}>▾</Text>
              </View>
            }
          />

          <If when={props.sourceFilter !== null}>
            <ActionButton
              theme={props.theme}
              kind="ghost"
              compact
              label="×"
              accessibilityLabel={s().market_clear_source_filter}
              tooltip={s().market_clear_source_filter}
              onPress={() => props.onSelectSourceFilter(null)}
            />
          </If>
        </View>

        <ActionButton
          theme={props.theme}
          kind="ghost"
          label="↻"
          loading={props.loading}
          accessibilityLabel={props.loading ? s().market_loading : s().btn_trending}
          tooltip={props.loading ? s().market_loading : s().btn_trending}
          onPress={props.onRefresh}
        />

        <View style={{ flex: 1 }} />

        <If when={props.entries.length > 0}>
          <Label theme={props.theme} muted size={12}>
            {props.countBadge}
          </Label>
        </If>
      </View>

      <If when={Boolean(props.error)}>
        <View style={{ paddingHorizontal: 20, paddingBottom: 8 }}>
          <AlertBanner theme={props.theme} tone="error" message={props.error ?? ""} />
        </View>
      </If>

      {/* Marketplace Grid / Loading / Empty */}
      <If
        when={props.loading && props.entries.length === 0}
        else={
          <If
            when={props.entries.length === 0}
            else={
              <List
                ref={props.onListRef}
                each={props.entries}
                keyExtractor={(entry) => entry.item.id}
                numColumns={3}
                estimatedItemSize={116}
                loadMoreThreshold={1}
                onLoadMore={() => {
                  if (!props.loadMoreFailed) {
                    props.onLoadMore();
                  }
                }}
                contentStyle={{ paddingHorizontal: 20, paddingVertical: 8, gap: 12 }}
                style={{ flex: 1, minHeight: 0 }}
                footer={
                  <If when={props.showFooter}>
                    <View
                      style={{
                        height: 64,
                        flexDirection: "row",
                        alignItems: "center",
                        justifyContent: "center",
                        gap: 8,
                      }}
                    >
                      <If
                        when={props.loadingMore}
                        else={
                          <ActionButton
                            theme={props.theme}
                            kind="outline"
                            compact
                            label={s().btn_load_more}
                            onPress={props.onLoadMore}
                          />
                        }
                      >
                        <View role="summary" accessibilityState={{ busy: true }}>
                          <Label theme={props.theme} muted size={12}>
                            {s().market_loading_more}
                          </Label>
                        </View>
                      </If>
                    </View>
                  </If>
                }
              >
                {(entry) => (
                  <View
                    style={{
                      height: 108,
                      paddingHorizontal: 12,
                      paddingVertical: 10,
                      justifyContent: "space-between",
                      borderRadius: 10,
                      borderWidth: 1,
                      borderColor:
                        !props.searchActive && entry().rankIndex === 0
                          ? props.theme.primary
                          : props.theme.border,
                      backgroundColor: props.theme.surface,
                    }}
                  >
                    {/* Row 1: Name, Official badge, Rank */}
                    <View style={{ flexDirection: "row", alignItems: "center", gap: 6 }}>
                      <Text
                        tooltip={hintTooltip(
                          () => props.theme,
                          () => entry().item.name,
                        )}
                        style={{
                          numberOfLines: 1,
                          textOverflow: "ellipsis",
                          flex: 1,
                          minWidth: 0,
                          fontSize: 13,
                          fontWeight: "600",
                          color: props.theme.foreground,
                          fontFamily: props.theme.fontFamily,
                        }}
                      >
                        {entry().item.name}
                      </Text>

                      <If when={entry().item.is_official}>
                        <View
                          accessibilityLabel={s().market_official}
                          tooltip={hintTooltip(
                            () => props.theme,
                            () => s().market_official,
                          )}
                          style={{
                            paddingHorizontal: 5,
                            paddingVertical: 1,
                            borderRadius: 4,
                            backgroundColor: props.theme.accent,
                            flexShrink: 0,
                          }}
                        >
                          <Text
                            style={{
                              fontSize: 10,
                              fontWeight: "600",
                              color: props.theme.primary,
                              fontFamily: props.theme.fontFamily,
                            }}
                          >
                            {s().market_official}
                          </Text>
                        </View>
                      </If>

                      <If when={!props.searchActive}>
                        <Text
                          style={{
                            fontSize: 11,
                            fontWeight: entry().rankIndex < 3 ? "600" : "400",
                            color:
                              entry().rankIndex === 0 ? props.theme.primary : props.theme.muted,
                            fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                            flexShrink: 0,
                          }}
                        >
                          {`#${entry().rankIndex + 1}`}
                        </Text>
                      </If>
                    </View>

                    {/* Row 2: Source repo filter + external link */}
                    <View style={{ flexDirection: "row", alignItems: "center", gap: 6 }}>
                      <If when={entry().item.source.length > 0}>
                        <Button
                          onPress={() => props.onSelectSourceFilter(entry().item.source)}
                          accessibilityLabel={`${s().market_filter_scope} · ${entry().item.name} · ${entry().item.source}`}
                          tooltip={hintTooltip(
                            () => props.theme,
                            () => `${s().market_filter_scope} · ${entry().item.source}`,
                          )}
                          style={{
                            flex: 1,
                            minWidth: 0,
                            flexDirection: "row",
                            alignItems: "center",
                          }}
                        >
                          <Text
                            style={{
                              numberOfLines: 1,
                              textOverflow: "ellipsis",
                              flex: 1,
                              minWidth: 0,
                              fontSize: 12,
                              color:
                                props.sourceFilter === entry().item.source
                                  ? props.theme.primary
                                  : props.theme.muted,
                              fontFamily: props.theme.fontFamily,
                            }}
                          >
                            {entry().item.source}
                          </Text>
                        </Button>
                      </If>

                      <If when={Boolean(entry().item.detail_url)}>
                        <ActionButton
                          theme={props.theme}
                          kind="ghost"
                          compact
                          label="↗"
                          accessibilityLabel={`${s().market_view_details} · ${entry().item.name}`}
                          tooltip={s().market_view_details}
                          onPress={() => {
                            const url = entry().item.detail_url;
                            if (url) {
                              void openUrl(url);
                            }
                          }}
                        />
                      </If>
                    </View>

                    {/* Row 3: Installs count + Primary action */}
                    <View style={{ flexDirection: "row", alignItems: "center", gap: 8 }}>
                      <Text
                        tooltip={hintTooltip(
                          () => props.theme,
                          () => `${entry().item.installs} ${s().installs_unit}`,
                        )}
                        style={{
                          numberOfLines: 1,
                          textOverflow: "ellipsis",
                          flex: 1,
                          minWidth: 0,
                          fontSize: 12,
                          color: props.theme.muted,
                          fontFamily: props.theme.fontFamily,
                        }}
                      >
                        {`${formatInstallCount(entry().item.installs)} ${s().installs_unit}`}
                      </Text>

                      <ActionButton
                        theme={props.theme}
                        kind="outline"
                        compact
                        loading={Boolean(props.installingRefs[entry().item.install_ref.trim()])}
                        disabled={
                          marketInstallStatus(
                            entry().item,
                            props.installedSourceRefs,
                            props.installedSkillNames,
                          ) === "same_name"
                        }
                        label={actionLabelFor(entry().item)}
                        tooltip={actionTooltipFor(entry().item)}
                        accessibilityLabel={`${actionLabelFor(entry().item)} · ${entry().item.name} · ${entry().item.source}`}
                        onPress={() => props.onItemAction(entry().item)}
                      />
                    </View>
                  </View>
                )}
              </List>
            }
          >
            <EmptyState
              theme={props.theme}
              title={s().empty_market_title}
              description={s().empty_market_desc}
            />
          </If>
        }
      >
        <View
          role="summary"
          accessibilityState={{ busy: true }}
          accessibilityLabel={s().market_loading}
          style={{ flex: 1, alignItems: "center", justifyContent: "center", padding: 24 }}
        >
          <Label theme={props.theme} muted size={13}>
            {s().market_loading}
          </Label>
        </View>
      </If>
    </View>
  );
}
