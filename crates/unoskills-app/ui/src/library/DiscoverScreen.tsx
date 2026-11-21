import { View, cleanup, effect, keymap, signal, untracked, type InputRef, type ListRef } from "nsh";
import { PageTabs } from "../components";
import type { ScreenProps } from "../shared";
import { AdoptTab } from "./AdoptTab";
import { MarketplaceTab } from "./MarketplaceTab";
import { QuickInstallDialog } from "./QuickInstallDialog";
import { createAdoption } from "./adoption";
import { ActionButton, PageHeader } from "./controls";
import { createMarketplace } from "./marketplace";
import { getStrings, labelWithCount } from "./strings";
import type { MarketBoard, Skill } from "./types";

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
  let marketSearchRef: InputRef | undefined;
  let adoptSearchRef: InputRef | undefined;
  let marketListRef: ListRef | undefined;
  let adoptListRef: ListRef | undefined;
  let lastRouteSignature = "";
  let wasActive = false;

  const adoption = createAdoption(props, (skills) => installedSkills.set(skills));
  const marketplace = createMarketplace(props, installedSkills, adoption.scan, () => {
    marketListRef?.scrollToOffset({ offset: 0, animated: false });
  });

  const s = () => getStrings(props.locale);
  const isActive = () =>
    props.active ?? props.route.page === "discover";

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

    void adoption.scan();

    if (routeChanged) {
      const boardFromRoute = parseMarketBoard(route.tab);
      if (route.tab === "adopt" || route.tab === "local") {
        subTab.set("adopt");
      } else if (boardFromRoute) {
        subTab.set("marketplace");
        untracked(() => void marketplace.selectBoard(boardFromRoute));
      } else if (route.tab === "marketplace") {
        subTab.set("marketplace");
      }

      if (route.tab === "install" || route.source) {
        installDialogInitialSource.set(route.source);
        installDialogOpen.set(true);
      }
    }

    untracked(() => {
      if (subTab() === "marketplace") marketplace.ensureBoard();
    });

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
        busyText={adoption.busyText() ?? marketplace.busyText()}
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
            label: s().discover_adopt_tab,
            count: adoption.totalCandidates(),
            accessibilityLabel: labelWithCount(props.locale, s().discover_adopt_tab, adoption.totalCandidates()),
          },
        ]}
        onSelect={(id) => {
          subTab.set(id);
          if (id === "marketplace") marketplace.ensureBoard();
        }}
      />

      <If
        when={subTab() === "marketplace"}
        else={
          <AdoptTab
            locale={props.locale}
            theme={props.theme}
            searchQuery={adoption.searchQuery()}
            totalCandidates={adoption.totalCandidates()}
            rows={adoption.rows()}
            selectedPaths={adoption.selectedPaths()}
            selectedCount={adoption.selectedCount()}
            scanning={adoption.scanning()}
            adopting={adoption.adopting()}
            error={adoption.error()}
            onSearchInputRef={(ref) => {
              adoptSearchRef = ref;
            }}
            onListRef={(ref) => {
              adoptListRef = ref;
            }}
            onSearchChange={(value) => {
              adoption.changeSearch(value);
              adoptListRef?.scrollToOffset({ offset: 0, animated: false });
            }}
            onRescan={() => {
              props.refresh();
              void adoption.scan();
            }}
            onToggleGroupExpanded={adoption.toggleGroupExpanded}
            onToggleGroupSelected={adoption.toggleGroupSelected}
            onToggleCandidateSelected={adoption.toggleCandidateSelected}
            onClearSelection={adoption.clearSelection}
            onConfirmAdopt={() => {
              void adoption.confirmAdopt();
            }}
          />
        }
      >
        <MarketplaceTab
          locale={props.locale}
          theme={props.theme}
          searchQuery={marketplace.searchQuery()}
          searchActive={marketplace.searchActive()}
          board={marketplace.board()}
          sourceFilter={marketplace.sourceFilter()}
          sourceSelectItems={marketplace.sourceOptions()}
          totalItemsCount={marketplace.itemCount()}
          entries={marketplace.entries()}
          countBadge={marketplace.countBadge()}
          loading={marketplace.loading()}
          loadingMore={marketplace.loadingMore()}
          loadMoreFailed={marketplace.loadMoreFailed()}
          showFooter={marketplace.showFooter()}
          error={marketplace.error()}
          installedSourceRefs={marketplace.installedSourceRefs()}
          installedSkillNames={marketplace.installedSkillNames()}
          installingRefs={marketplace.installingRefs()}
          onSearchInputRef={(ref) => {
            marketSearchRef = ref;
          }}
          onListRef={(ref) => {
            marketListRef = ref;
          }}
          onSearchChange={marketplace.changeSearch}
          onSearchSubmit={(query) => void marketplace.search(query)}
          onClearSearch={() => marketplace.changeSearch("")}
          onSelectBoard={(board) => void marketplace.selectBoard(board)}
          onSelectSourceFilter={marketplace.selectSource}
          onRefresh={() => void marketplace.refresh()}
          onLoadMore={() => {
            void marketplace.loadMore();
          }}
          onItemAction={(item) => {
            void marketplace.openOrInstall(item);
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
          void adoption.scan();
          props.navigate({ page: "library", skillId: skill.id });
        }}
        onNotify={props.notify}
      />
    </View>
  );
}
