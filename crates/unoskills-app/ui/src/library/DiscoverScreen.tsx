import { PageTabs } from "../components";
import {
  type InputRef,
  type ListRef,
  View,
  batch,
  cleanup,
  computed,
  effect,
  keymap,
  signal,
} from "nsh";
import { AlertDialog } from "nsh/base";
import type { ScreenProps } from "../shared";
import { type AdoptGroup, type AdoptRow, AdoptTab } from "./AdoptTab";
import { ALL_SOURCES_VALUE, MarketplaceTab } from "./MarketplaceTab";
import { QuickInstallDialog } from "./QuickInstallDialog";
import { ActionButton, PageHeader } from "./controls";
import { marketInstallStatus } from "./format";
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
  busyAdoptingAll,
  busyInstalling,
  busySearching,
  getStrings,
  labelWithCount,
  toastAdoptedAll,
  toastInstalled,
} from "./strings";
import type { AdoptCandidate, MarketBoard, MarketplaceSkillItem, Skill } from "./types";

const MAX_INITIALLY_EXPANDED_SKILLS = 8;

interface CachedMarketBoard {
  items: MarketplaceSkillItem[];
  nextPage: number;
  total: number | null;
  hasMore: boolean;
}

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
    if (!adoptSearch().trim()) {
      adoptExpansion.update((prev) => ({ ...prev, [groupKey]: !current.expanded }));
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
            icon="+"
            label={s().btn_install_from_link}
            onPress={() => {
              installDialogInitialSource.set(undefined);
              installDialogOpen.set(true);
            }}
          />
        }
      />

      {/* Sub-tabs: Marketplace | Local skills (count) */}
      <PageTabs
        theme={props.theme}
        accessibilityLabel={s().discover_title}
        selectedId={subTab()}
        paddingHorizontal={20}
        tabs={[
          { id: "marketplace", icon: "◇", label: s().discover_market_tab },
          {
            id: "adopt",
            icon: "◫",
            label: labelWithCount(props.locale, s().discover_adopt_tab, adoptCandidates().length),
          },
        ]}
        onSelect={(id) => {
          if (id === "marketplace") {
            subTab.set("marketplace");
            if (!marketLoading() && !marketSearchActive() && !marketCache.has(marketBoard())) {
              void loadMarketBoard(marketBoard(), false);
            }
          } else {
            subTab.set("adopt");
          }
        }}
      />

      <If
        when={subTab() === "marketplace"}
        else={
          <AdoptTab
            locale={props.locale}
            theme={props.theme}
            searchQuery={adoptSearch()}
            totalCandidates={adoptCandidates().length}
            rows={adoptRows()}
            selectedPaths={selectedAdoptPaths()}
            selectedCount={selectedAdoptCount()}
            scanning={scanningAdopt()}
            adopting={adopting()}
            error={adoptError()}
            onSearchInputRef={(ref) => {
              adoptSearchRef = ref;
            }}
            onListRef={(ref) => {
              adoptListRef = ref;
            }}
            onSearchChange={(value) => {
              adoptSearch.set(value);
              adoptListRef?.scrollToOffset({ offset: 0, animated: false });
            }}
            onRescan={() => {
              props.refresh();
              void refreshInstalledAndAdopt();
            }}
            onToggleGroupExpanded={toggleAdoptGroupExpanded}
            onToggleGroupSelected={toggleAdoptGroupSelected}
            onToggleCandidateSelected={toggleAdoptCandidateSelected}
            onClearSelection={() => selectedAdoptPaths.set({})}
            onConfirmAdopt={() => {
              void confirmAndAdoptSelected();
            }}
          />
        }
      >
        <MarketplaceTab
          locale={props.locale}
          theme={props.theme}
          searchQuery={marketSearchInput()}
          searchActive={marketSearchActive()}
          board={marketBoard()}
          sourceFilter={marketSourceFilter()}
          sourceSelectItems={marketSourceSelectItems()}
          totalItemsCount={marketItems().length}
          entries={filteredMarketItems()}
          countBadge={marketCountBadge()}
          loading={marketLoading()}
          loadingMore={marketLoadingMore()}
          loadMoreFailed={marketLoadMoreFailed()}
          showFooter={showMarketFooter()}
          error={marketError()}
          installedSourceRefs={installedSourceRefs()}
          installedSkillNames={installedSkillNames()}
          installingRefs={installingRefs()}
          onSearchInputRef={(ref) => {
            marketSearchRef = ref;
          }}
          onListRef={(ref) => {
            marketListRef = ref;
          }}
          onSearchChange={(value) => {
            marketSearchInput.set(value);
            if (marketSearchActive() && value.trim().length === 0) {
              void loadMarketBoard(marketBoard(), false);
            }
          }}
          onSearchSubmit={(query) => {
            void runMarketSearch(query);
          }}
          onClearSearch={() => {
            marketSearchInput.set("");
            if (marketSearchActive()) {
              void loadMarketBoard(marketBoard(), false);
            }
          }}
          onSelectBoard={(board) => {
            void loadMarketBoard(board, false);
          }}
          onSelectSourceFilter={(source) => {
            marketSourceFilter.set(source);
            resetMarketScroll();
          }}
          onRefresh={() => {
            if (marketSearchActive()) {
              void runMarketSearch(marketSearchInput());
            } else {
              void loadMarketBoard(marketBoard(), true);
            }
          }}
          onLoadMore={() => {
            void loadNextMarketPage();
          }}
          onItemAction={(item) => {
            void handleInstallMarketItem(item);
          }}
        />
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
