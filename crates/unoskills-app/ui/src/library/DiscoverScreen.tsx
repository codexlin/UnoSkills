import {
  type InputRef,
  List,
  type ListRef,
  Text,
  View,
  batch,
  cleanup,
  computed,
  effect,
  keymap,
  openUrl,
  signal,
} from "nsh";
import {
  AlertDialog,
  Button,
  Checkbox,
  Input,
  Menu,
  MenuItem,
  Select,
  Tab,
  Tabs,
} from "nsh/base";
import type { ScreenProps } from "../shared";
import { QuickInstallDialog } from "./QuickInstallDialog";
import { ActionButton, AlertBanner, EmptyState, PageHeader, TokenBadge, hintTooltip } from "./controls";
import { formatInstallCount, marketInstallStatus } from "./format";
import {
  adoptLocalSkills,
  errorMessage,
  fetchLeaderboard,
  installFromSource,
  loadLibraryState,
  scanAdoptableSkills,
  searchMarketplace,
} from "./host";
import {
  adoptConfirmTitle,
  adoptSelectedCount,
  boardLabel,
  busyAdoptingAll,
  busyInstalling,
  busySearching,
  getStrings,
  labelWithCount,
  toastAdoptedAll,
  toastInstalled,
} from "./strings";
import type { AdoptCandidate, MarketBoard, MarketplaceSkillItem, Skill } from "./types";

const ALL_SOURCES_VALUE = "__all_sources__";
const MAX_INITIALLY_EXPANDED_SKILLS = 8;
const MARKET_BOARDS: MarketBoard[] = ["trending", "hot", "all-time"];

interface CachedMarketBoard {
  items: MarketplaceSkillItem[];
  nextPage: number;
  total: number | null;
  hasMore: boolean;
}

interface AdoptGroup {
  key: string;
  name: string;
  candidates: AdoptCandidate[];
  expanded: boolean;
}

type AdoptRow =
  | { kind: "group"; id: string; group: AdoptGroup }
  | { kind: "candidate"; id: string; candidate: AdoptCandidate };

function parseMarketBoard(tab?: string): MarketBoard | null {
  switch (tab) {
    case "trending":
    case "hot":
    case "all-time":
      return tab;
    default:
      return null;
  }
}

export function Discover(props: ScreenProps) {
  const subTab = signal<"marketplace" | "adopt">("marketplace");
  const installDialogOpen = signal(false);
  const installDialogInitialSource = signal<string | undefined>(undefined);

  // Installed skills snapshot (used to show Installed / SameName status & navigate to Library)
  const installedSkills = signal<Skill[]>([]);
  const installingRefs = signal<Record<string, boolean>>({});
  const busyText = signal<string | null>(null);

  // Marketplace state
  const marketBoard = signal<MarketBoard>("trending");
  const marketCache = new Map<MarketBoard, CachedMarketBoard>();
  const marketItems = signal<MarketplaceSkillItem[]>([]);
  const marketNextPage = signal(0);
  const marketTotal = signal<number | null>(null);
  const marketHasMore = signal(false);
  const marketLoading = signal(false);
  const marketLoadingMore = signal(false);
  const marketLoadMoreFailed = signal(false);
  const marketSearchActive = signal(false);
  const marketSearchInput = signal("");
  const marketError = signal<string | null>(null);
  const marketSourceFilter = signal<string | null>(null);

  // Adopt (Local skills) state
  const adoptCandidates = signal<AdoptCandidate[]>([]);
  const adoptSearch = signal("");
  const selectedAdoptPaths = signal<Record<string, boolean>>({});
  const adoptExpansion = signal<Record<string, boolean>>({});
  const adopting = signal(false);
  const scanningAdopt = signal(false);
  const adoptError = signal<string | null>(null);

  let marketSearchRef: InputRef | undefined;
  let adoptSearchRef: InputRef | undefined;
  let marketListRef: ListRef | undefined;
  let adoptListRef: ListRef | undefined;
  let marketReqSeq = 0;
  let lastRouteSignature = "";
  let wasActive = false;

  const s = () => getStrings(props.locale);
  const isActive = () =>
    (props as ScreenProps & { active?: boolean }).active ?? props.route.page === "discover";

  const installedSourceRefs = computed(() => {
    const set = new Set<string>();
    for (const skill of installedSkills()) {
      if (skill.source_ref) {
        set.add(skill.source_ref.toLowerCase());
      }
    }
    return set;
  });

  const installedSkillNames = computed(() => {
    const set = new Set<string>();
    for (const skill of installedSkills()) {
      set.add(skill.slug.toLowerCase());
      set.add(skill.name.toLowerCase());
    }
    return set;
  });

  const filteredMarketItems = computed(() => {
    const filter = marketSourceFilter();
    const items = marketItems();
    if (!filter) {
      return items.map((item, idx) => ({ item, rankIndex: idx }));
    }
    const result: Array<{ item: MarketplaceSkillItem; rankIndex: number }> = [];
    for (let idx = 0; idx < items.length; idx++) {
      const item = items[idx];
      if (item.source === filter) {
        result.push({ item, rankIndex: idx });
      }
    }
    return result;
  });

  const marketSourceSelectItems = computed(() => {
    const sources = new Set<string>();
    for (const item of marketItems()) {
      if (item.source) {
        sources.add(item.source);
      }
    }
    const sorted = Array.from(sources).sort((a, b) => a.localeCompare(b));
    return [
      { value: ALL_SOURCES_VALUE, title: s().market_all_sources },
      ...sorted.map((source) => ({ value: source, title: source })),
    ];
  });

  const adoptGroups = computed<AdoptGroup[]>(() => {
    const query = adoptSearch().trim().toLowerCase();
    const grouped = new Map<string, { key: string; name: string; candidates: AdoptCandidate[] }>();

    for (const candidate of adoptCandidates()) {
      const matches =
        !query ||
        candidate.name.toLowerCase().includes(query) ||
        candidate.agent_name.toLowerCase().includes(query) ||
        candidate.path.toLowerCase().includes(query);
      if (!matches) {
        continue;
      }
      const mapKey = `${candidate.agent_name}\0${candidate.agent_key}`;
      let entry = grouped.get(mapKey);
      if (!entry) {
        entry = { key: candidate.agent_key, name: candidate.agent_name, candidates: [] };
        grouped.set(mapKey, entry);
      }
      entry.candidates.push(candidate);
    }

    const expansion = adoptExpansion();
    return Array.from(grouped.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([, group]) => ({
        ...group,
        expanded:
          query.length > 0 ||
          (expansion[group.key] !== undefined
            ? expansion[group.key]
            : group.candidates.length <= MAX_INITIALLY_EXPANDED_SKILLS),
      }));
  });

  const adoptRows = computed<AdoptRow[]>(() => {
    const rows: AdoptRow[] = [];
    for (const group of adoptGroups()) {
      rows.push({ kind: "group", id: `group:${group.key}`, group });
      if (group.expanded) {
        for (const candidate of group.candidates) {
          rows.push({ kind: "candidate", id: `cand:${candidate.path}`, candidate });
        }
      }
    }
    return rows;
  });

  const selectedAdoptCount = computed(() => Object.keys(selectedAdoptPaths()).length);

  const refreshInstalledAndAdopt = async () => {
    scanningAdopt.set(true);
    adoptError.set(null);
    try {
      const [libState, scanResult] = await Promise.all([
        loadLibraryState(null),
        scanAdoptableSkills(),
      ]);
      const validPaths = new Set(scanResult.candidates.map((candidate) => candidate.path));
      batch(() => {
        installedSkills.set(libState.skills);
        adoptCandidates.set(scanResult.candidates);
        selectedAdoptPaths.update((prev) => {
          const next: Record<string, boolean> = {};
          for (const path of Object.keys(prev)) {
            if (validPaths.has(path)) {
              next[path] = true;
            }
          }
          return next;
        });
      });
    } catch (error) {
      const message = errorMessage(error);
      adoptError.set(message);
      props.notify(message, "error");
    } finally {
      scanningAdopt.set(false);
    }
  };

  const resetMarketScroll = () => {
    marketListRef?.scrollToOffset({ offset: 0, animated: false });
  };

  const loadMarketBoard = async (board: MarketBoard, forceRefresh: boolean) => {
    const keepResults = marketBoard() === board && !marketSearchActive();
    if (!forceRefresh && keepResults && (marketLoading() || marketCache.has(board))) {
      return;
    }

    const req = ++marketReqSeq;
    batch(() => {
      marketError.set(null);
      marketBoard.set(board);
      marketSearchActive.set(false);
      marketSourceFilter.set(null);
      marketLoadingMore.set(false);
      marketLoadMoreFailed.set(false);
    });

    const cached = !forceRefresh ? marketCache.get(board) : undefined;
    if (cached) {
      batch(() => {
        marketLoading.set(false);
        marketItems.set(cached.items);
        marketNextPage.set(cached.nextPage);
        marketTotal.set(cached.total);
        marketHasMore.set(cached.hasMore);
      });
      resetMarketScroll();
      return;
    }

    batch(() => {
      marketLoading.set(true);
      busyText.set(s().busy_loading_market);
      if (!keepResults) {
        marketItems.set([]);
        marketTotal.set(null);
        marketHasMore.set(false);
      }
    });

    try {
      const page = await fetchLeaderboard(board, 0);
      if (req !== marketReqSeq) {
        return;
      }
      const next: CachedMarketBoard = {
        items: page.items,
        nextPage: page.page + 1,
        total: page.total,
        hasMore: page.has_more,
      };
      marketCache.set(board, next);
      batch(() => {
        marketItems.set(next.items);
        marketNextPage.set(next.nextPage);
        marketTotal.set(next.total);
        marketHasMore.set(next.hasMore);
        marketError.set(null);
      });
      resetMarketScroll();
    } catch (error) {
      if (req !== marketReqSeq) {
        return;
      }
      const message = errorMessage(error);
      marketError.set(message);
      props.notify(message, "error");
    } finally {
      if (req === marketReqSeq) {
        batch(() => {
          marketLoading.set(false);
          busyText.set(null);
        });
      }
    }
  };

  const loadNextMarketPage = async () => {
    if (
      marketSearchActive() ||
      marketSourceFilter() !== null ||
      !marketHasMore() ||
      marketLoading() ||
      marketLoadingMore()
    ) {
      return;
    }

    const board = marketBoard();
    const nextPage = marketNextPage();
    const req = marketReqSeq;
    batch(() => {
      marketLoadingMore.set(true);
      marketLoadMoreFailed.set(false);
    });

    try {
      const pageData = await fetchLeaderboard(board, nextPage);
      if (req !== marketReqSeq || marketBoard() !== board) {
        return;
      }
      const existing = marketItems();
      const seenIds = new Set(existing.map((item) => item.id));
      const appended: MarketplaceSkillItem[] = [];
      for (const item of pageData.items) {
        if (!seenIds.has(item.id)) {
          seenIds.add(item.id);
          appended.push(item);
        }
      }
      const merged = appended.length > 0 ? [...existing, ...appended] : existing;
      const updatedTotal = pageData.total !== null ? pageData.total : marketTotal();
      const hasMore = pageData.has_more && appended.length > 0;
      const updatedNextPage = pageData.page + 1;

      marketCache.set(board, {
        items: merged,
        nextPage: updatedNextPage,
        total: updatedTotal,
        hasMore,
      });

      batch(() => {
        marketItems.set(merged);
        marketNextPage.set(updatedNextPage);
        marketTotal.set(updatedTotal);
        marketHasMore.set(hasMore);
      });
    } catch (error) {
      if (req !== marketReqSeq) {
        return;
      }
      marketLoadMoreFailed.set(true);
      props.notify(errorMessage(error), "error");
    } finally {
      if (req === marketReqSeq) {
        marketLoadingMore.set(false);
      }
    }
  };

  const runMarketSearch = async (rawQuery: string) => {
    const query = rawQuery.trim();
    if (!query) {
      await loadMarketBoard(marketBoard(), false);
      return;
    }

    const req = ++marketReqSeq;
    batch(() => {
      marketError.set(null);
      marketSearchActive.set(true);
      marketSourceFilter.set(null);
      marketItems.set([]);
      marketLoading.set(true);
      marketLoadingMore.set(false);
      marketLoadMoreFailed.set(false);
      marketHasMore.set(false);
      marketTotal.set(null);
      busyText.set(busySearching(props.locale, query));
    });

    try {
      const result = await searchMarketplace(query, 60);
      if (req !== marketReqSeq) {
        return;
      }
      marketItems.set(result.items);
      resetMarketScroll();
    } catch (error) {
      if (req !== marketReqSeq) {
        return;
      }
      const message = errorMessage(error);
      marketError.set(message);
      props.notify(message, "error");
    } finally {
      if (req === marketReqSeq) {
        batch(() => {
          marketLoading.set(false);
          busyText.set(null);
        });
      }
    }
  };

  const handleInstallMarketItem = async (item: MarketplaceSkillItem) => {
    const status = marketInstallStatus(item, installedSourceRefs(), installedSkillNames());
    if (status === "installed") {
      const targetKey = item.install_ref.toLowerCase();
      const match = installedSkills().find(
        (skill) => skill.source_ref?.toLowerCase() === targetKey,
      );
      if (match) {
        props.navigate({ page: "library", skillId: match.id });
      }
      return;
    }
    if (status === "same_name") {
      return;
    }

    const refKey = item.install_ref.trim();
    if (!refKey || installingRefs()[refKey]) {
      return;
    }

    installingRefs.update((prev) => ({ ...prev, [refKey]: true }));
    busyText.set(busyInstalling(props.locale, refKey));
    try {
      const skill = await installFromSource(refKey);
      props.notify(toastInstalled(props.locale, skill.name, skill.estimated_tokens), "success");
      props.refresh();
      await refreshInstalledAndAdopt();
    } catch (error) {
      props.notify(errorMessage(error), "error");
    } finally {
      installingRefs.update((prev) => {
        const next = { ...prev };
        delete next[refKey];
        return next;
      });
      busyText.set(null);
    }
  };

  const toggleAdoptGroupExpanded = (groupKey: string) => {
    const current = adoptGroups().find((group) => group.key === groupKey);
    if (!current) {
      return;
    }
    const nextExpanded = !current.expanded;
    if (!adoptSearch().trim()) {
      adoptExpansion.update((prev) => ({ ...prev, [groupKey]: nextExpanded }));
    }
  };

  const toggleAdoptGroupSelected = (group: AdoptGroup) => {
    const paths = group.candidates.map((candidate) => candidate.path);
    const current = selectedAdoptPaths();
    const allSelected = paths.length > 0 && paths.every((path) => Boolean(current[path]));
    selectedAdoptPaths.update((prev) => {
      const next = { ...prev };
      if (allSelected) {
        for (const path of paths) {
          delete next[path];
        }
      } else {
        for (const path of paths) {
          next[path] = true;
        }
      }
      return next;
    });
  };

  const toggleAdoptCandidateSelected = (path: string, checked: boolean) => {
    selectedAdoptPaths.update((prev) => {
      const next = { ...prev };
      if (checked) {
        next[path] = true;
      } else {
        delete next[path];
      }
      return next;
    });
  };

  const confirmAndAdoptSelected = async () => {
    if (adopting()) {
      return;
    }
    const selectedMap = selectedAdoptPaths();
    const chosen = adoptCandidates().filter((candidate) => Boolean(selectedMap[candidate.path]));
    if (chosen.length === 0) {
      return;
    }

    const { action } = await AlertDialog.show({
      title: adoptConfirmTitle(props.locale, chosen.length),
      description: `${s().adopt_confirm_benefit}\n\n${s().adopt_confirm_description}`,
      actions: [
        { id: "cancel", label: s().btn_cancel, role: "cancel" },
        { id: "adopt", label: s().btn_adopt, role: "default" },
      ],
    });
    if (action !== "adopt") {
      return;
    }

    batch(() => {
      adopting.set(true);
      busyText.set(busyAdoptingAll(props.locale, chosen.length));
    });
    try {
      const result = await adoptLocalSkills(chosen);
      selectedAdoptPaths.set({});
      props.refresh();
      await refreshInstalledAndAdopt();
      let message = toastAdoptedAll(props.locale, result.adopted_count, result.failed.length);
      if (result.failed.length > 0) {
        const first = result.failed[0];
        message += `\n${first.name} · ${first.error}`;
      }
      props.notify(message, result.failed.length > 0 ? "error" : "success");
    } catch (error) {
      props.notify(errorMessage(error), "error");
    } finally {
      batch(() => {
        adopting.set(false);
        busyText.set(null);
      });
    }
  };

  // Synchronize with route deep-links and shell refresh revisions while active.
  effect(() => {
    const rev = props.revision;
    const active = isActive();
    const route = props.route;
    const becameActive = active && !wasActive;
    wasActive = active;

    if (!active || route.page !== "discover") {
      void rev;
      return;
    }

    const sig = `${route.page}|${route.tab ?? ""}|${route.source ?? ""}`;
    const routeChanged = becameActive || sig !== lastRouteSignature;
    lastRouteSignature = sig;

    void refreshInstalledAndAdopt();

    if (routeChanged) {
      const boardFromRoute = parseMarketBoard(route.tab);
      if (route.tab === "adopt" || route.tab === "local") {
        subTab.set("adopt");
      } else if (boardFromRoute) {
        subTab.set("marketplace");
        void loadMarketBoard(boardFromRoute, false);
      } else if (route.tab === "marketplace") {
        subTab.set("marketplace");
      }

      if (route.tab === "install" || route.source) {
        installDialogInitialSource.set(route.source);
        installDialogOpen.set(true);
      }
    }

    if (
      subTab() === "marketplace" &&
      !marketLoading() &&
      !marketSearchActive() &&
      !marketCache.has(marketBoard())
    ) {
      void loadMarketBoard(marketBoard(), false);
    }

    void rev;
  });

  // Scope mod+k shortcut to while Discover is the active retained screen.
  effect(() => {
    if (!isActive()) {
      return;
    }
    const unbind = keymap.bind("mod+k", () => {
      if (subTab() === "marketplace") {
        marketSearchRef?.focus();
      } else {
        adoptSearchRef?.focus();
      }
    });
    cleanup(unbind);
  });

  const marketCountBadge = () => {
    const visible = filteredMarketItems().length;
    if (marketSourceFilter()) {
      return `${s().market_source_filter} · ${visible}`;
    }
    const total = marketTotal();
    if (total !== null && !marketSearchActive() && total > visible) {
      return `${visible} / ${total}`;
    }
    return String(visible);
  };

  const showMarketFooter = () =>
    !marketSearchActive() &&
    marketSourceFilter() === null &&
    filteredMarketItems().length > 0 &&
    (marketHasMore() || marketLoadingMore() || marketLoadMoreFailed());

  return (
    <View
      style={{
        flex: 1,
        minHeight: 0,
        minWidth: 0,
        backgroundColor: props.theme.background,
      }}
    >
      <PageHeader
        theme={props.theme}
        title={s().discover_title}
        busyText={busyText()}
        right={
          <ActionButton
            theme={props.theme}
            kind="outline"
            label={`+ ${s().btn_install_from_link}`}
            onPress={() => {
              installDialogInitialSource.set(undefined);
              installDialogOpen.set(true);
            }}
          />
        }
      />

      {/* Sub-tabs: Marketplace | Local skills (count) */}
      <View
        style={{
          paddingHorizontal: 20,
          borderBottomWidth: 1,
          borderColor: props.theme.border,
          backgroundColor: props.theme.background,
          flexShrink: 0,
        }}
      >
        <Tabs
          accessibilityLabel={s().discover_title}
          style={{ flexDirection: "row", gap: 20 }}
          indicatorStyle={{
            bottom: 0,
            height: 2,
            backgroundColor: props.theme.primary,
          }}
        >
          <Tab
            selected={subTab() === "marketplace"}
            onPress={() => {
              subTab.set("marketplace");
              if (!marketLoading() && !marketSearchActive() && !marketCache.has(marketBoard())) {
                void loadMarketBoard(marketBoard(), false);
              }
            }}
            style={{ paddingVertical: 10 }}
          >
            <Text
              style={{
                fontSize: 13,
                fontWeight: subTab() === "marketplace" ? "600" : "500",
                color: subTab() === "marketplace" ? props.theme.foreground : props.theme.muted,
                fontFamily: props.theme.fontFamily,
              }}
            >
              {s().discover_market_tab}
            </Text>
          </Tab>
          <Tab
            selected={subTab() === "adopt"}
            onPress={() => subTab.set("adopt")}
            style={{ paddingVertical: 10 }}
          >
            <Text
              style={{
                fontSize: 13,
                fontWeight: subTab() === "adopt" ? "600" : "500",
                color: subTab() === "adopt" ? props.theme.foreground : props.theme.muted,
                fontFamily: props.theme.fontFamily,
              }}
            >
              {labelWithCount(props.locale, s().discover_adopt_tab, adoptCandidates().length)}
            </Text>
          </Tab>
        </Tabs>
      </View>

      {/* Main Content */}
      <If
        when={subTab() === "marketplace"}
        else={
          /* Local Skills (Adopt) Tab */
          <View style={{ flex: 1, minHeight: 0, minWidth: 0 }}>
            <View
              style={{
                flexDirection: "row",
                flexWrap: "wrap",
                alignItems: "center",
                gap: 8,
                paddingHorizontal: 20,
                paddingVertical: 10,
                borderBottomWidth: 1,
                borderColor: props.theme.border,
                flexShrink: 0,
              }}
            >
              <View style={{ flex: 1, minWidth: 180, maxWidth: 360, flexDirection: "row", alignItems: "center", gap: 4 }}>
                <Input
                  ref={adoptSearchRef}
                  value={adoptSearch()}
                  onChange={({ value }) => {
                    adoptSearch.set(value);
                    adoptListRef?.scrollToOffset({ offset: 0, animated: false });
                  }}
                  placeholder={s().ph_adopt_search}
                  accessibilityLabel={s().ph_adopt_search}
                  style={{
                    flex: 1,
                    fontSize: 13,
                    paddingHorizontal: 10,
                    paddingVertical: 6,
                    borderRadius: 8,
                    borderWidth: 1,
                    borderColor: props.theme.border,
                    backgroundColor: props.theme.surface,
                    color: props.theme.foreground,
                    placeholderColor: props.theme.muted,
                    fontFamily: props.theme.fontFamily,
                  }}
                />
                <If when={adoptSearch().length > 0}>
                  <ActionButton
                    theme={props.theme}
                    kind="ghost"
                    compact
                    label="×"
                    accessibilityLabel={s().adopt_clear_selection}
                    onPress={() => adoptSearch.set("")}
                  />
                </If>
              </View>

              <ActionButton
                theme={props.theme}
                kind="outline"
                label={s().btn_rescan}
                loading={scanningAdopt()}
                disabled={adopting()}
                onPress={() => {
                  props.refresh();
                  void refreshInstalledAndAdopt();
                }}
              />
            </View>

            <If when={Boolean(adoptError())}>
              <View style={{ paddingHorizontal: 20, paddingTop: 10 }}>
                <AlertBanner theme={props.theme} tone="error" message={adoptError() ?? ""} />
              </View>
            </If>

            <If
              when={adoptRows().length === 0}
              else={
                <List
                  ref={adoptListRef}
                  each={adoptRows()}
                  keyExtractor={(row) => row.id}
                  estimatedItemSize={42}
                  style={{ flex: 1, minHeight: 0 }}
                >
                  {(row) => (
                    <View style={{ paddingHorizontal: 20 }}>
                      <If
                        when={row().kind === "group"}
                        else={
                          /* Candidate row */
                          <View
                            style={{
                              height: 42,
                              flexDirection: "row",
                              alignItems: "center",
                              gap: 10,
                              paddingHorizontal: 8,
                              borderBottomWidth: 1,
                              borderColor: props.theme.border,
                            }}
                          >
                            <Checkbox
                              checked={Boolean(
                                row().kind === "candidate" &&
                                  selectedAdoptPaths()[
                                    (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.path
                                  ],
                              )}
                              disabled={adopting()}
                              accessibilityLabel={
                                row().kind === "candidate"
                                  ? (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.name
                                  : ""
                              }
                              onChange={({ checked }) => {
                                const current = row();
                                if (current.kind === "candidate") {
                                  toggleAdoptCandidateSelected(current.candidate.path, checked);
                                }
                              }}
                              indicatorStyle={{
                                width: 16,
                                height: 16,
                                borderRadius: 4,
                                borderWidth: 1,
                                borderColor:
                                  row().kind === "candidate" &&
                                  selectedAdoptPaths()[
                                    (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.path
                                  ]
                                    ? props.theme.primary
                                    : props.theme.border,
                                backgroundColor:
                                  row().kind === "candidate" &&
                                  selectedAdoptPaths()[
                                    (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.path
                                  ]
                                    ? props.theme.primary
                                    : props.theme.background,
                                alignItems: "center",
                                justifyContent: "center",
                              }}
                              indicator={
                                <Text
                                  style={{
                                    fontSize: 11,
                                    fontWeight: "700",
                                    color: props.theme.onPrimary,
                                  }}
                                >
                                  {row().kind === "candidate" &&
                                  selectedAdoptPaths()[
                                    (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.path
                                  ]
                                    ? "✓"
                                    : ""}
                                </Text>
                              }
                            />

                            <Text
                              style={{
          numberOfLines: 1,
          textOverflow: "ellipsis",
                                width: "32%",
                                minWidth: 0,
                                fontSize: 13,
                                color: props.theme.foreground,
                                fontFamily: props.theme.fontFamily,
                              }}
                            >
                              {row().kind === "candidate"
                                ? (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.name
                                : ""}
                            </Text>

                            <Text
                              tooltip={hintTooltip(
                                () => props.theme,
                                () =>
                                  row().kind === "candidate"
                                    ? (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.path
                                    : "",
                              )}
                              style={{
                                numberOfLines: 1,
                                textOverflow: "ellipsis",
                                flex: 1,
                                minWidth: 0,
                                fontSize: 12,
                                color: props.theme.muted,
                                fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                              }}
                            >
                              {row().kind === "candidate"
                                ? (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.path
                                : ""}
                            </Text>

                            <TokenBadge
                              theme={props.theme}
                              tokens={
                                row().kind === "candidate"
                                  ? (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate
                                      .estimated_tokens
                                  : 0
                              }
                            />
                          </View>
                        }
                      >
                        {/* Group header row */}
                        <View
                          style={{
                            height: 40,
                            flexDirection: "row",
                            alignItems: "center",
                            justifyContent: "space-between",
                            gap: 8,
                            paddingHorizontal: 10,
                            borderRadius: 6,
                            backgroundColor: props.theme.surface,
                            borderBottomWidth: 1,
                            borderColor: props.theme.border,
                          }}
                        >
                          <Button
                            onPress={() => {
                              const current = row();
                              if (current.kind === "group") {
                                toggleAdoptGroupExpanded(current.group.key);
                              }
                            }}
                            style={{
                              flex: 1,
                              minWidth: 0,
                              flexDirection: "row",
                              alignItems: "center",
                              gap: 6,
                              paddingVertical: 4,
                            }}
                          >
                            <Text style={{ fontSize: 12, color: props.theme.muted }}>
                              {row().kind === "group" &&
                              (row() as Extract<AdoptRow, { kind: "group" }>).group.expanded
                                ? "▾"
                                : "▸"}
                            </Text>
                            <Text
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
                              {row().kind === "group"
                                ? `${(row() as Extract<AdoptRow, { kind: "group" }>).group.name} (${
                                    (row() as Extract<AdoptRow, { kind: "group" }>).group.candidates.length
                                  })`
                                : ""}
                            </Text>
                          </Button>

                          <ActionButton
                            theme={props.theme}
                            kind="ghost"
                            compact
                            disabled={adopting()}
                            label={
                              row().kind === "group" &&
                              (row() as Extract<AdoptRow, { kind: "group" }>).group.candidates.every(
                                (candidate) => Boolean(selectedAdoptPaths()[candidate.path]),
                              )
                                ? s().adopt_clear_selection
                                : s().adopt_select_group
                            }
                            onPress={() => {
                              const current = row();
                              if (current.kind === "group") {
                                toggleAdoptGroupSelected(current.group);
                              }
                            }}
                          />
                        </View>
                      </If>
                    </View>
                  )}
                </List>
              }
            >
              <EmptyState
                theme={props.theme}
                title={
                  adoptCandidates().length === 0
                    ? s().empty_adopt_done_title
                    : s().empty_adopt_filter_title
                }
                description={
                  adoptCandidates().length === 0
                    ? s().empty_adopt_done_desc
                    : s().empty_adopt_filter_desc
                }
              />
            </If>

            {/* Selection Action Bar */}
            <If when={selectedAdoptCount() > 0}>
              <View
                style={{
                  minHeight: 52,
                  flexDirection: "row",
                  alignItems: "center",
                  gap: 10,
                  paddingHorizontal: 20,
                  paddingVertical: 10,
                  borderTopWidth: 1,
                  borderColor: props.theme.border,
                  backgroundColor: props.theme.surface,
                  flexShrink: 0,
                }}
              >
                <Text
                  style={{
                    flex: 1,
                    minWidth: 0,
                    fontSize: 13,
                    color: props.theme.foreground,
                    fontFamily: props.theme.fontFamily,
                  }}
                >
                  {adoptSelectedCount(props.locale, selectedAdoptCount())}
                </Text>
                <ActionButton
                  theme={props.theme}
                  kind="ghost"
                  disabled={adopting()}
                  label={s().adopt_clear_selection}
                  onPress={() => selectedAdoptPaths.set({})}
                />
                <ActionButton
                  theme={props.theme}
                  kind="primary"
                  loading={adopting()}
                  label={labelWithCount(props.locale, s().btn_adopt, selectedAdoptCount())}
                  onPress={() => {
                    void confirmAndAdoptSelected();
                  }}
                />
              </View>
            </If>
          </View>
        }
      >
        {/* Marketplace Tab */}
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
            <View style={{ flex: 1, minWidth: 160, maxWidth: 360, flexDirection: "row", alignItems: "center", gap: 4 }}>
              <Input
                ref={marketSearchRef}
                value={marketSearchInput()}
                onChange={({ value }) => {
                  marketSearchInput.set(value);
                  if (marketSearchActive() && value.trim().length === 0) {
                    void loadMarketBoard(marketBoard(), false);
                  }
                }}
                onSubmit={({ text }) => {
                  void runMarketSearch(text);
                }}
                placeholder={s().ph_market_search}
                accessibilityLabel={s().ph_market_search}
                returnKeyType="search"
                submitBehavior="submit"
                style={{
                  flex: 1,
                  fontSize: 13,
                  paddingHorizontal: 10,
                  paddingVertical: 6,
                  borderRadius: 8,
                  borderWidth: 1,
                  borderColor: props.theme.border,
                  backgroundColor: props.theme.surface,
                  color: props.theme.foreground,
                  placeholderColor: props.theme.muted,
                  fontFamily: props.theme.fontFamily,
                }}
              />
              <If when={marketSearchInput().length > 0}>
                <ActionButton
                  theme={props.theme}
                  kind="ghost"
                  compact
                  label="×"
                  accessibilityLabel={s().adopt_clear_selection}
                  onPress={() => {
                    marketSearchInput.set("");
                    if (marketSearchActive()) {
                      void loadMarketBoard(marketBoard(), false);
                    }
                  }}
                />
              </If>
            </View>

            <ActionButton
              theme={props.theme}
              kind="ghost"
              label={s().btn_search}
              tooltip={s().market_search_hint}
              onPress={() => {
                void runMarketSearch(marketSearchInput());
              }}
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
                    borderRadius: 8,
                    borderWidth: 1,
                    borderColor: props.theme.border,
                    backgroundColor: props.theme.surface,
                  }}
                >
                  <Text
                    style={{
                      fontSize: 13,
                      color: props.theme.foreground,
                      fontFamily: props.theme.fontFamily,
                    }}
                  >
                    {marketSearchActive()
                      ? s().market_search_results
                      : boardLabel(props.locale, marketBoard())}
                  </Text>
                  <Text style={{ fontSize: 11, color: props.theme.muted }}>▾</Text>
                </View>
              }
            >
              <Each of={MARKET_BOARDS} key={(board) => board}>
                {(board) => (
                  <MenuItem
                    role="menuitemradio"
                    checked={!marketSearchActive() && marketBoard() === board()}
                    onSelect={() => {
                      void loadMarketBoard(board(), false);
                    }}
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
                      {!marketSearchActive() && marketBoard() === board() ? "✓" : ""}
                    </Text>
                    <Text
                      style={{
                        fontSize: 13,
                        color: props.theme.foreground,
                        fontFamily: props.theme.fontFamily,
                      }}
                    >
                      {boardLabel(props.locale, board())}
                    </Text>
                  </MenuItem>
                )}
              </Each>
            </Menu>

            {/* Source Filter Select (searchable) */}
            <View style={{ flexDirection: "row", alignItems: "center", gap: 4, flexShrink: 0 }}>
              <Select
                items={marketSourceSelectItems()}
                value={marketSourceFilter() ?? ALL_SOURCES_VALUE}
                searchable
                searchPlaceholder={s().market_source_search}
                searchAccessibilityLabel={s().market_source_search}
                accessibilityLabel={s().market_source_label}
                disabled={marketItems().length === 0}
                offset={4}
                onChange={({ value }) => {
                  marketSourceFilter.set(!value || value === ALL_SOURCES_VALUE ? null : value);
                  resetMarketScroll();
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
                      borderRadius: 8,
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
                      {marketSourceFilter() ?? s().market_all_sources}
                    </Text>
                    <Text style={{ fontSize: 11, color: props.theme.muted }}>▾</Text>
                  </View>
                }
              />

              <If when={marketSourceFilter() !== null}>
                <ActionButton
                  theme={props.theme}
                  kind="ghost"
                  compact
                  label="×"
                  accessibilityLabel={s().market_clear_source_filter}
                  tooltip={s().market_clear_source_filter}
                  onPress={() => {
                    marketSourceFilter.set(null);
                    resetMarketScroll();
                  }}
                />
              </If>
            </View>

            <ActionButton
              theme={props.theme}
              kind="ghost"
              label="↻"
              loading={marketLoading()}
              accessibilityLabel={marketLoading() ? s().market_loading : s().btn_trending}
              tooltip={marketLoading() ? s().market_loading : s().btn_trending}
              onPress={() => {
                if (marketSearchActive()) {
                  void runMarketSearch(marketSearchInput());
                } else {
                  void loadMarketBoard(marketBoard(), true);
                }
              }}
            />

            <View style={{ flex: 1 }} />

            <If when={filteredMarketItems().length > 0}>
              <Text
                style={{
                  fontSize: 12,
                  color: props.theme.muted,
                  fontFamily: props.theme.fontFamily,
                  flexShrink: 0,
                }}
              >
                {marketCountBadge()}
              </Text>
            </If>
          </View>

          <If when={Boolean(marketError())}>
            <View style={{ paddingHorizontal: 20, paddingBottom: 8 }}>
              <AlertBanner theme={props.theme} tone="error" message={marketError() ?? ""} />
            </View>
          </If>

          {/* Marketplace Grid / Loading / Empty */}
          <If
            when={marketLoading() && filteredMarketItems().length === 0}
            else={
              <If
                when={filteredMarketItems().length === 0}
                else={
                  <List
                    ref={marketListRef}
                    each={filteredMarketItems()}
                    keyExtractor={(entry) => entry.item.id}
                    numColumns={3}
                    estimatedItemSize={116}
                    loadMoreThreshold={1}
                    onLoadMore={() => {
                      if (!marketLoadMoreFailed()) {
                        void loadNextMarketPage();
                      }
                    }}
                    contentStyle={{ paddingHorizontal: 20, paddingVertical: 8, gap: 12 }}
                    style={{ flex: 1, minHeight: 0 }}
                    footer={
                      <If when={showMarketFooter()}>
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
                            when={marketLoadingMore()}
                            else={
                              <ActionButton
                                theme={props.theme}
                                kind="outline"
                                compact
                                label={s().btn_load_more}
                                onPress={() => {
                                  void loadNextMarketPage();
                                }}
                              />
                            }
                          >
                            <Text
                              role="summary"
            accessibilityState={{ busy: true }}
                              style={{
                                fontSize: 12,
                                color: props.theme.muted,
                                fontFamily: props.theme.fontFamily,
                              }}
                            >
                              {s().market_loading_more}
                            </Text>
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
                            !marketSearchActive() && entry().rankIndex === 0
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

                          <If when={!marketSearchActive()}>
                            <Text
                              style={{
                                fontSize: 11,
                                fontWeight: entry().rankIndex < 3 ? "600" : "400",
                                color:
                                  entry().rankIndex === 0
                                    ? props.theme.primary
                                    : props.theme.muted,
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
                              onPress={() => {
                                marketSourceFilter.set(entry().item.source);
                                resetMarketScroll();
                              }}
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
                                    marketSourceFilter() === entry().item.source
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
                            loading={Boolean(installingRefs()[entry().item.install_ref.trim()])}
                            disabled={
                              marketInstallStatus(
                                entry().item,
                                installedSourceRefs(),
                                installedSkillNames(),
                              ) === "same_name"
                            }
                            label={
                              marketInstallStatus(
                                entry().item,
                                installedSourceRefs(),
                                installedSkillNames(),
                              ) === "installed"
                                ? s().market_open_skill
                                : marketInstallStatus(
                                      entry().item,
                                      installedSourceRefs(),
                                      installedSkillNames(),
                                    ) === "same_name"
                                  ? s().market_same_name
                                  : s().market_add_to_library
                            }
                            tooltip={
                              marketInstallStatus(
                                entry().item,
                                installedSourceRefs(),
                                installedSkillNames(),
                              ) === "installed"
                                ? s().already_in_library
                                : marketInstallStatus(
                                      entry().item,
                                      installedSourceRefs(),
                                      installedSkillNames(),
                                    ) === "same_name"
                                  ? s().market_same_name
                                  : s().market_add_to_library
                            }
                            accessibilityLabel={`${
                              marketInstallStatus(
                                entry().item,
                                installedSourceRefs(),
                                installedSkillNames(),
                              ) === "installed"
                                ? s().market_open_skill
                                : marketInstallStatus(
                                      entry().item,
                                      installedSourceRefs(),
                                      installedSkillNames(),
                                    ) === "same_name"
                                  ? s().market_same_name
                                  : s().market_add_to_library
                            } · ${entry().item.name} · ${entry().item.source}`}
                            onPress={() => {
                              void handleInstallMarketItem(entry().item);
                            }}
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
              style={{
                flex: 1,
                alignItems: "center",
                justifyContent: "center",
                padding: 24,
              }}
            >
              <Text
                style={{
                  fontSize: 13,
                  color: props.theme.muted,
                  fontFamily: props.theme.fontFamily,
                }}
              >
                {s().market_loading}
              </Text>
            </View>
          </If>
        </View>
      </If>

      <QuickInstallDialog
        open={installDialogOpen()}
        initialSource={installDialogInitialSource()}
        locale={props.locale}
        theme={props.theme}
        onOpenChange={(open) => {
          installDialogOpen.set(open);
          if (!open) {
            installDialogInitialSource.set(undefined);
          }
        }}
        onInstalled={(skill) => {
          props.refresh();
          void refreshInstalledAndAdopt();
          props.navigate({ page: "library", skillId: skill.id });
        }}
        onNotify={props.notify}
      />
    </View>
  );
}
