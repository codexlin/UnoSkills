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
import { ActionControl, ActionMenu, AlertBanner, Icon, Label, LoadingLine, MenuAction, SelectControl, SkeletonBlock, textSize, textStyle } from "../components";
import type { Theme } from "../shared";
import { ActionButton, EmptyState, SearchField, hintTooltip } from "./controls";
import { formatInstallCount, marketInstallStatus } from "./format";
import { ALL_SOURCES_VALUE, type RankedMarketEntry } from "./marketplace";
import { type Locale, boardLabel, getStrings } from "./strings";
import type { MarketBoard, MarketplaceSkillItem } from "./types";

export const MARKET_BOARDS: MarketBoard[] = ["trending", "hot", "all-time"];

const SKELETON_ROWS = [0, 1, 2, 3];

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
  const cardHeight = () => Math.max(100, textSize(props.theme, 100));
  const textRowHeight = () => Math.max(20, textStyle(props.theme).lineHeight);

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
  const showSkeleton = () => props.loading && props.entries.length === 0;

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

  const actionGlyphFor = (item: MarketplaceSkillItem) => {
    const status = marketInstallStatus(item, props.installedSourceRefs, props.installedSkillNames);
    if (status === "installed") {
      return "check";
    }
    if (status === "same_name") {
      return "triangle-alert";
    }
    return "plus";
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
          glyph="search"
          accessibilityLabel={s().btn_search}
          tooltip={s().market_search_hint}
          onPress={() => props.onSearchSubmit(props.searchQuery)}
        />

        <ActionMenu theme={props.theme} label={s().market_sort} minWidth={170}
          value={props.searchActive ? s().market_search_results : boardLabel(props.locale, props.board)}>
          <Each of={MARKET_BOARDS} key={(board) => board}>
            {(board) => <MenuAction theme={props.theme} label={boardLabel(props.locale, board())}
              checked={!props.searchActive && props.board === board()}
              onSelect={() => props.onSelectBoard(board())} />}
          </Each>
        </ActionMenu>

        <View style={{ flexDirection: "row", alignItems: "center", gap: 4, flexShrink: 0 }}>
          <SelectControl theme={props.theme} label={s().market_source_label} items={props.sourceSelectItems}
            value={props.sourceFilter ?? ALL_SOURCES_VALUE} displayValue={props.sourceFilter ?? s().market_all_sources}
            disabled={props.totalItemsCount === 0} toolbar width={176} popupMinWidth={176}
            searchable searchLabel={s().market_source_search}
            onChange={(value) => props.onSelectSourceFilter(!value || value === ALL_SOURCES_VALUE ? null : value)} />

          <If when={props.sourceFilter !== null}>
            <ActionButton
              theme={props.theme}
              kind="ghost"
              compact
              glyph="x"
              accessibilityLabel={s().market_clear_source_filter}
              tooltip={s().market_clear_source_filter}
              onPress={() => props.onSelectSourceFilter(null)}
            />
          </If>
        </View>

        <ActionButton
          theme={props.theme}
          kind="ghost"
          glyph="refresh-cw"
          loading={props.loading && !showSkeleton()}
          disabled={showSkeleton()}
          accessibilityLabel={props.loading && !showSkeleton() ? s().market_loading : s().btn_trending}
          tooltip={s().btn_trending}
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
          when={showSkeleton()}
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
                  estimatedItemSize={cardHeight() + 12}
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
                              glyph="refresh-cw"
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
                        height: cardHeight(),
                        paddingHorizontal: 12,
                        paddingVertical: 8,
                        gap: 4,
                        justifyContent: "space-between",
                        borderRadius: props.theme.panel.radius,
                        boxShadow: props.theme.panel.shadow,
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
                          height: textRowHeight(),
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
                            ...textStyle(props.theme),
                            fontWeight: "600",
                            color: props.theme.foreground,
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
                            <Icon name="check" color={props.theme.onAccent} size={10} />
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
                                      ...textStyle(props.theme, { size: textSize(props.theme, 12), mono: true }),
                                      color: props.theme.muted,
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
                                      ...textStyle(props.theme, { size: textSize(props.theme, 11) }),
                                      fontWeight: "600",
                                      color: props.theme.onAccent,
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
                                style={{ ...textStyle(props.theme, { size: textSize(props.theme, 12) }), color: props.theme.primary }}
                              >
                                ★
                              </Text>
                              <Text
                                style={{
                                  ...textStyle(props.theme, { size: textSize(props.theme, 12) }),
                                  fontWeight: "600",
                                  color: props.theme.foreground,
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
                          height: textRowHeight(),
                          flexDirection: "row",
                          alignItems: "center",
                          gap: 6,
                        }}
                      >
                        <If when={entry().item.source.length > 0}>
                          <ActionControl theme={props.theme}
                            onPress={() => props.onSelectSourceFilter(entry().item.source)}
                            accessibilityLabel={`${s().market_filter_scope} · ${entry().item.name} · ${entry().item.source}`}
                            tooltip={hintTooltip(
                              () => props.theme,
                              () => `${s().market_filter_scope} · ${entry().item.source}`,
                            )}
                            class="flex-row items-center gap-1"
                            style={{
                              flex: 1,
                              minWidth: 0,
                              borderRadius: 4,
                            }}
                          >
                            <Text
                              accessibilityElementsHidden
                              style={{
                                ...textStyle(props.theme, { size: textSize(props.theme, 11) }),
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
                                ...textStyle(props.theme, { size: textSize(props.theme, 12) }),
                                color:
                                  props.sourceFilter === entry().item.source
                                    ? props.theme.primary
                                    : props.theme.muted,
                              }}
                            >
                              {entry().item.source}
                            </Text>
                          </ActionControl>
                        </If>

                        <If when={Boolean(entry().item.detail_url)}>
                          <ActionButton
                            theme={props.theme}
                            kind="ghost"
                            compact
                            glyph="external-link"
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
                            style={{ ...textStyle(props.theme, { size: textSize(props.theme, 11) }), color: props.theme.muted, flexShrink: 0 }}
                          >
                            ↓
                          </Text>
                          <Text
                            style={{
                              numberOfLines: 1,
                              textOverflow: "ellipsis",
                              flex: 1,
                              minWidth: 0,
                              ...textStyle(props.theme, { size: textSize(props.theme, 12) }),
                              color: props.theme.muted,
                            }}
                          >
                            {`${formatInstallCount(entry().item.installs)} ${s().installs_unit}`}
                          </Text>
                        </View>

                        <ActionButton
                          theme={props.theme}
                          kind="outline"
                          compact
                          glyph={actionGlyphFor(entry().item)}
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
              <If when={!props.error}>
                <EmptyState
                  theme={props.theme}
                  glyph="globe"
                  title={s().empty_market_title}
                  description={s().empty_market_desc}
                />
              </If>
            </If>
          }
        >
          <View
            testID="market-loading-skeleton"
            role="summary"
            accessibilityState={{ busy: true }}
            accessibilityLabel={props.searchActive ? s().market_searching : s().market_loading}
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
                          height: cardHeight(),
                          paddingHorizontal: 12,
                          paddingVertical: 10,
                          justifyContent: "space-between",
                          borderRadius: props.theme.panel.radius,
                          borderWidth: 1,
                          borderColor: props.theme.border,
                          backgroundColor: props.theme.surface,
                        }}
                      >
                        <SkeletonBlock theme={props.theme} width="50%" height={12} />
                        <SkeletonBlock theme={props.theme} width="40%" height={10} />
                        <View
                          style={{
                            flexDirection: "row",
                            alignItems: "center",
                            justifyContent: "space-between",
                          }}
                        >
                          <SkeletonBlock theme={props.theme} width="25%" height={10} />
                          <SkeletonBlock theme={props.theme} width="25%" height={24} />
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
