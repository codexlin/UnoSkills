import {
  type InputRef,
  List,
  type ListRef,
  Text,
  View,
  computed,
  openUrl,
  signal,
  windowSize,
} from "nsh";
import { Button, Menu, MenuItem, Select } from "nsh/base";
import { Label } from "../controls";
import type { Theme } from "../shared";
import {
  ActionButton,
  AlertBanner,
  EmptyState,
  LoadingLine,
  SearchField,
  hintTooltip,
} from "./controls";
import { formatInstallCount, marketInstallStatus } from "./format";
import { type Locale, boardLabel, getStrings } from "./strings";
import type { MarketBoard, MarketplaceSkillItem } from "./types";

export const ALL_SOURCES_VALUE = "__all_sources__";
export const MARKET_BOARDS: MarketBoard[] = ["trending", "hot", "all-time"];

const SKELETON_ROWS = [0, 1, 2, 3];

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
  const containerWidth = signal(0);

  // Match legacy 3-column desktop grid while adapting cleanly on narrower viewports.
  const gridColumns = computed(() => {
    const measured = containerWidth();
    const width = measured > 0 ? measured : windowSize().width;
    if (width >= 860) {
      return 3;
    }
    if (width >= 540) {
      return 2;
    }
    return 1;
  });

  const skeletonCols = computed(() => Array.from({ length: gridColumns() }, (_, i) => i));

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

  const actionIconFor = (item: MarketplaceSkillItem): string => {
    const status = marketInstallStatus(item, props.installedSourceRefs, props.installedSkillNames);
    if (status === "installed") {
      return "✓";
    }
    if (status === "same_name") {
      return "!";
    }
    return "+";
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

  const cardRadius = () => Math.max(props.theme.radius ?? 8, 8);

  return (
    <View style={{ flex: 1, minHeight: 0, minWidth: 0 }}>
      {/* Marketplace Toolbar */}
      <View
        style={{
          minHeight: 48,
          flexDirection: "row",
          flexWrap: "wrap",
          alignItems: "center",
          gap: 8,
          paddingHorizontal: 20,
          paddingVertical: 8,
          flexShrink: 0,
        }}
      >
        <SearchField
          theme={props.theme}
          inputRef={props.onSearchInputRef}
          value={props.searchQuery}
          placeholder={s().ph_market_search}
          accessibilityLabel={s().ph_market_search}
          clearLabel={s().adopt_clear_selection}
          minWidth={160}
          maxWidth={360}
          returnKeyType="search"
          submitBehavior="submit"
          onChange={props.onSearchChange}
          onSubmit={props.onSearchSubmit}
          onClear={props.onClearSearch}
        />

        <ActionButton
          theme={props.theme}
          kind="ghost"
          icon="⌕"
          accessibilityLabel={s().btn_search}
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
              class="transition-colors"
              style={{
                minHeight: 34,
                flexDirection: "row",
                alignItems: "center",
                gap: 6,
                paddingHorizontal: 12,
                paddingVertical: 6,
                borderRadius: props.theme.radius ?? 8,
                borderWidth: 1,
                borderColor: props.theme.border,
                backgroundColor: props.theme.background,
              }}
              hoverStyle={{ backgroundColor: props.theme.accent }}
              focusVisibleStyle={{ borderColor: props.theme.primary }}
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
                class="transition-colors"
                style={{
                  width: 176,
                  minHeight: 34,
                  flexDirection: "row",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: 6,
                  paddingHorizontal: 10,
                  paddingVertical: 6,
                  borderRadius: props.theme.radius ?? 8,
                  borderWidth: 1,
                  borderColor: props.theme.border,
                  backgroundColor: props.theme.background,
                }}
                hoverStyle={{ backgroundColor: props.theme.accent }}
                focusVisibleStyle={{ borderColor: props.theme.primary }}
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
              icon="×"
              accessibilityLabel={s().market_clear_source_filter}
              tooltip={s().market_clear_source_filter}
              onPress={() => props.onSelectSourceFilter(null)}
            />
          </If>
        </View>

        <ActionButton
          theme={props.theme}
          kind="ghost"
          icon="↻"
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

      {/* Marketplace Responsive Virtual Grid / Skeleton / Empty */}
      <View
        style={{ flex: 1, minHeight: 0, minWidth: 0 }}
        onLayout={({ bounds }) => {
          if (bounds.size.width > 0) {
            containerWidth.set(bounds.size.width);
          }
        }}
      >
        <If
          when={props.loading && props.entries.length === 0}
          else={
            <If
              when={props.entries.length === 0}
              else={
                <List
                  testID="marketplace-grid"
                  ref={props.onListRef}
                  each={props.entries}
                  keyExtractor={(entry) => entry.item.id}
                  numColumns={gridColumns()}
                  estimatedItemSize={112}
                  loadMoreThreshold={1}
                  onLoadMore={() => {
                    if (!props.loadMoreFailed) {
                      props.onLoadMore();
                    }
                  }}
                  contentStyle={{
                    paddingHorizontal: 20,
                    paddingVertical: 8,
                    rowGap: 12,
                    columnGap: 12,
                  }}
                  style={{ flex: 1, minHeight: 0 }}
                  footer={
                    <If when={props.showFooter}>
                      <View
                        style={{
                          height: 84,
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
                              icon="↻"
                              label={s().btn_load_more}
                              onPress={props.onLoadMore}
                            />
                          }
                        >
                          <LoadingLine
                            theme={props.theme}
                            compact
                            label={s().market_loading_more}
                          />
                        </If>
                      </View>
                    </If>
                  }
                >
                  {(entry) => (
                    <View
                      style={{
                        height: 100,
                        paddingHorizontal: 12,
                        paddingVertical: 8,
                        gap: 4,
                        justifyContent: "space-between",
                        borderRadius: cardRadius(),
                        borderWidth: 1,
                        borderColor:
                          !props.searchActive && entry().rankIndex === 0
                            ? props.theme.primary
                            : props.theme.border,
                        backgroundColor: props.theme.surface,
                      }}
                    >
                      {/* Row 1: Name, Official verified badge, 3-tier Rank */}
                      <View
                        style={{
                          height: 20,
                          flexDirection: "row",
                          alignItems: "center",
                          gap: 8,
                        }}
                      >
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
                            fontSize: 14,
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
                              width: 16,
                              height: 16,
                              borderRadius: 999,
                              backgroundColor: props.theme.accent,
                              borderWidth: 1,
                              borderColor: props.theme.primary,
                              alignItems: "center",
                              justifyContent: "center",
                              flexShrink: 0,
                            }}
                          >
                            <Text
                              accessibilityElementsHidden
                              style={{
                                fontSize: 10,
                                fontWeight: "700",
                                color: props.theme.primary,
                              }}
                            >
                              ✓
                            </Text>
                          </View>
                        </If>

                        <If when={!props.searchActive}>
                          <If
                            when={entry().rankIndex === 0}
                            else={
                              <If
                                when={entry().rankIndex === 1 || entry().rankIndex === 2}
                                else={
                                  <Text
                                    style={{
                                      fontSize: 12,
                                      color: props.theme.muted,
                                      fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                                      flexShrink: 0,
                                    }}
                                  >
                                    {`#${entry().rankIndex + 1}`}
                                  </Text>
                                }
                              >
                                <View
                                  style={{
                                    paddingHorizontal: 6,
                                    paddingVertical: 1,
                                    borderRadius: 4,
                                    borderWidth: 1,
                                    borderColor: props.theme.border,
                                    backgroundColor: props.theme.accent,
                                    flexShrink: 0,
                                  }}
                                >
                                  <Text
                                    style={{
                                      fontSize: 11,
                                      fontWeight: "600",
                                      color: props.theme.foreground,
                                      fontFamily: props.theme.fontFamily,
                                    }}
                                  >
                                    {`#${entry().rankIndex + 1}`}
                                  </Text>
                                </View>
                              </If>
                            }
                          >
                            <View
                              style={{
                                flexDirection: "row",
                                alignItems: "center",
                                gap: 4,
                                flexShrink: 0,
                              }}
                            >
                              <Text
                                accessibilityElementsHidden
                                style={{ fontSize: 12, color: props.theme.primary }}
                              >
                                ★
                              </Text>
                              <Text
                                style={{
                                  fontSize: 12,
                                  fontWeight: "600",
                                  color: props.theme.foreground,
                                  fontFamily: props.theme.fontFamily,
                                }}
                              >
                                #1
                              </Text>
                            </View>
                          </If>
                        </If>
                      </View>

                      {/* Row 2: Source repo filter + external link */}
                      <View
                        style={{
                          height: 20,
                          flexDirection: "row",
                          alignItems: "center",
                          gap: 6,
                        }}
                      >
                        <If when={entry().item.source.length > 0}>
                          <Button
                            onPress={() => props.onSelectSourceFilter(entry().item.source)}
                            accessibilityLabel={`${s().market_filter_scope} · ${entry().item.name} · ${entry().item.source}`}
                            tooltip={hintTooltip(
                              () => props.theme,
                              () => `${s().market_filter_scope} · ${entry().item.source}`,
                            )}
                            class="flex-row items-center gap-1 transition-colors"
                            style={{
                              flex: 1,
                              minWidth: 0,
                              borderRadius: 4,
                            }}
                            hoverStyle={{ opacity: 0.8 }}
                            focusVisibleStyle={{ borderWidth: 1, borderColor: props.theme.primary }}
                          >
                            <Text
                              accessibilityElementsHidden
                              style={{
                                fontSize: 11,
                                color:
                                  props.sourceFilter === entry().item.source
                                    ? props.theme.primary
                                    : props.theme.muted,
                              }}
                            >
                              ↳
                            </Text>
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
                            icon="↗"
                            accessibilityLabel={`${s().market_view_details} · ${entry().item.name} · ${entry().item.source}`}
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
                      <View
                        style={{
                          height: 26,
                          flexDirection: "row",
                          alignItems: "center",
                          gap: 8,
                        }}
                      >
                        <View
                          tooltip={hintTooltip(
                            () => props.theme,
                            () => `${entry().item.installs} ${s().installs_unit}`,
                          )}
                          style={{
                            flex: 1,
                            minWidth: 0,
                            flexDirection: "row",
                            alignItems: "center",
                            gap: 6,
                          }}
                        >
                          <Text
                            accessibilityElementsHidden
                            style={{ fontSize: 11, color: props.theme.muted, flexShrink: 0 }}
                          >
                            ↓
                          </Text>
                          <Text
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
                        </View>

                        <ActionButton
                          theme={props.theme}
                          kind="outline"
                          compact
                          icon={actionIconFor(entry().item)}
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
                icon="◇"
                title={s().empty_market_title}
                description={s().empty_market_desc}
              />
            </If>
          }
        >
          {/* Legacy 4-row Skeleton Card Grid */}
          <View
            testID="market-loading-skeleton"
            role="summary"
            accessibilityState={{ busy: true }}
            accessibilityLabel={s().market_loading}
            style={{
              flex: 1,
              minHeight: 0,
              overflow: "hidden",
              paddingHorizontal: 20,
              paddingVertical: 8,
              gap: 12,
            }}
          >
            <Each of={SKELETON_ROWS} key={(row) => String(row)}>
              {() => (
                <View style={{ flexDirection: "row", gap: 12 }}>
                  <Each of={skeletonCols()} key={(col) => String(col)}>
                    {() => (
                      <View
                        style={{
                          flex: 1,
                          minWidth: 0,
                          height: 100,
                          paddingHorizontal: 12,
                          paddingVertical: 10,
                          justifyContent: "space-between",
                          borderRadius: cardRadius(),
                          borderWidth: 1,
                          borderColor: props.theme.border,
                          backgroundColor: props.theme.surface,
                        }}
                      >
                        <View
                          class="h-3 w-1/2 rounded-sm animate-pulse motion-reduce:animate-none"
                          style={{ backgroundColor: props.theme.border }}
                        />
                        <View
                          class="h-2.5 w-2/5 rounded-sm animate-pulse motion-reduce:animate-none"
                          style={{ backgroundColor: props.theme.border }}
                        />
                        <View
                          style={{
                            flexDirection: "row",
                            alignItems: "center",
                            justifyContent: "space-between",
                          }}
                        >
                          <View
                            class="h-2.5 w-1/4 rounded-sm animate-pulse motion-reduce:animate-none"
                            style={{ backgroundColor: props.theme.border }}
                          />
                          <View
                            class="h-6 w-1/4 rounded-sm animate-pulse motion-reduce:animate-none"
                            style={{ backgroundColor: props.theme.border }}
                          />
                        </View>
                      </View>
                    )}
                  </Each>
                </View>
              )}
            </Each>
          </View>
        </If>
      </View>
    </View>
  );
}
