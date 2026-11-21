import { batch, cleanup, computed, signal } from "nsh";
import type { ScreenProps } from "../shared";
import { marketInstallStatus } from "./format";
import { errorMessage, fetchLeaderboard, installFromSource, searchMarketplace } from "./host";
import { busyInstalling, busySearching, getStrings, toastInstalled } from "./strings";
import type { MarketBoard, MarketplaceSkillItem, Skill } from "./types";

export const ALL_SOURCES_VALUE = "__all_sources__";

export interface RankedMarketEntry {
  item: MarketplaceSkillItem;
  rankIndex: number;
}

interface CachedMarketBoard {
  items: MarketplaceSkillItem[];
  nextPage: number;
  total: number | null;
  hasMore: boolean;
}

export function createMarketplace(
  screen: Pick<ScreenProps, "locale" | "navigate" | "notify" | "refresh">,
  installedSkills: () => readonly Skill[],
  refreshInventory: () => Promise<void>,
  resetScroll: () => void,
) {
  const s = () => getStrings(screen.locale);
  const installingRefs = signal<Record<string, boolean>>({});
  const queryBusyText = signal<string | null>(null);
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

  let marketReqSeq = 0;
  cleanup(() => { marketReqSeq += 1; });

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
    const result: RankedMarketEntry[] = [];
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
        queryBusyText.set(null);
        marketItems.set(cached.items);
        marketNextPage.set(cached.nextPage);
        marketTotal.set(cached.total);
        marketHasMore.set(cached.hasMore);
      });
      resetScroll();
      return;
    }

    batch(() => {
      marketLoading.set(true);
      queryBusyText.set(s().busy_loading_market);
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
      resetScroll();
    } catch (error) {
      if (req !== marketReqSeq) {
        return;
      }
      const message = errorMessage(error);
      marketError.set(message);
      screen.notify(message, "error");
    } finally {
      if (req === marketReqSeq) {
        batch(() => {
          marketLoading.set(false);
          queryBusyText.set(null);
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
      screen.notify(errorMessage(error), "error");
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
      queryBusyText.set(busySearching(screen.locale, query));
    });

    try {
      const result = await searchMarketplace(query, 60);
      if (req !== marketReqSeq) {
        return;
      }
      marketItems.set(result.items);
      resetScroll();
    } catch (error) {
      if (req !== marketReqSeq) {
        return;
      }
      const message = errorMessage(error);
      marketError.set(message);
      screen.notify(message, "error");
    } finally {
      if (req === marketReqSeq) {
        batch(() => {
          marketLoading.set(false);
          queryBusyText.set(null);
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
        screen.navigate({ page: "library", skillId: match.id });
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
    try {
      const skill = await installFromSource(refKey);
      screen.notify(toastInstalled(screen.locale, skill.name, skill.estimated_tokens), "success");
      screen.refresh();
      await refreshInventory();
    } catch (error) {
      screen.notify(errorMessage(error), "error");
    } finally {
      installingRefs.update((prev) => {
        const next = { ...prev };
        delete next[refKey];
        return next;
      });
    }
  };

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

  const ensureBoard = () => {
    if (!marketLoading() && !marketSearchActive() && !marketCache.has(marketBoard())) {
      void loadMarketBoard(marketBoard(), false);
    }
  };

  const changeSearch = (value: string) => {
    marketSearchInput.set(value);
    if (marketSearchActive() && value.trim().length === 0) {
      void loadMarketBoard(marketBoard(), false);
    }
  };

  const refresh = () => marketSearchActive()
    ? runMarketSearch(marketSearchInput())
    : loadMarketBoard(marketBoard(), true);

  return {
    board: () => marketBoard(),
    searchQuery: () => marketSearchInput(),
    searchActive: () => marketSearchActive(),
    sourceFilter: () => marketSourceFilter(),
    sourceOptions: marketSourceSelectItems,
    itemCount: () => marketItems().length,
    entries: filteredMarketItems,
    countBadge: marketCountBadge,
    loading: () => marketLoading(),
    loadingMore: () => marketLoadingMore(),
    loadMoreFailed: () => marketLoadMoreFailed(),
    showFooter: showMarketFooter,
    error: () => marketError(),
    installedSourceRefs,
    installedSkillNames,
    installingRefs: () => installingRefs(),
    busyText: () => {
      const installing = Object.keys(installingRefs())[0];
      return installing ? busyInstalling(screen.locale, installing) : queryBusyText();
    },
    ensureBoard,
    selectBoard: (board: MarketBoard) => loadMarketBoard(board, false),
    changeSearch,
    search: runMarketSearch,
    selectSource: (source: string | null) => {
      marketSourceFilter.set(source);
      resetScroll();
    },
    refresh,
    loadMore: loadNextMarketPage,
    openOrInstall: handleInstallMarketItem,
  };
}
