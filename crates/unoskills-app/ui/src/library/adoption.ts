import { batch, cleanup, computed, signal } from "nsh";
import { AlertDialog } from "nsh/base";
import type { ScreenProps } from "../shared";
import { adoptLocalSkills, errorMessage, loadLibraryState, scanAdoptableSkills } from "./host";
import { adoptConfirmTitle, getStrings, toastAdoptedAll } from "./strings";
import type { AdoptBatchResponse, AdoptCandidate, Skill } from "./types";

export interface AdoptGroup {
  key: string;
  name: string;
  candidates: AdoptCandidate[];
  expanded: boolean;
}

export type AdoptRow =
  | { kind: "group"; id: string; group: AdoptGroup }
  | { kind: "candidate"; id: string; candidate: AdoptCandidate };

const MAX_INITIALLY_EXPANDED_SKILLS = 8;

export function createAdoption(
  screen: Pick<ScreenProps, "locale" | "notify" | "refresh">,
  onInventory: (skills: Skill[]) => void,
) {
  const s = () => getStrings(screen.locale);
  const adoptCandidates = signal<AdoptCandidate[]>([]);
  const adoptSearch = signal("");
  const selectedAdoptPaths = signal<Record<string, boolean>>({});
  const adoptExpansion = signal<Record<string, boolean>>({});
  const adopting = signal(false);
  const scanningAdopt = signal(false);
  const adoptError = signal<string | null>(null);
  const failedBatch = signal<AdoptBatchResponse | null>(null);

  let scanSequence = 0;
  cleanup(() => { scanSequence += 1; });

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

  const scan = async () => {
    const request = ++scanSequence;
    scanningAdopt.set(true);
    adoptError.set(null);
    try {
      const [libState, scanResult] = await Promise.all([
        loadLibraryState(null),
        scanAdoptableSkills(),
      ]);
      if (request !== scanSequence) {
        return;
      }
      const validPaths = new Set(scanResult.candidates.map((candidate) => candidate.path));
      batch(() => {
        onInventory(libState.skills);
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
      if (request !== scanSequence) {
        return;
      }
      const message = errorMessage(error);
      adoptError.set(message);
      screen.notify(message, "error");
    } finally {
      if (request === scanSequence) {
        scanningAdopt.set(false);
      }
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
      title: adoptConfirmTitle(screen.locale, chosen.length),
      description: `${s().adopt_confirm_benefit}\n\n${s().adopt_confirm_description}`,
      actions: [
        { id: "cancel", label: s().btn_cancel, role: "cancel" },
        { id: "adopt", label: s().btn_adopt, role: "default" },
      ],
    });
    if (action !== "adopt") {
      return;
    }

    adopting.set(true);
    failedBatch.set(null);
    try {
      const result = await adoptLocalSkills(chosen);
      const failedPaths = new Set(result.failed.map((failure) => failure.path));
      selectedAdoptPaths.set(
        Object.fromEntries(chosen.filter(({ path }) => failedPaths.has(path)).map(({ path }) => [path, true])),
      );
      if (result.failed.length > 0) {
        failedBatch.set(result);
      } else {
        screen.notify(toastAdoptedAll(screen.locale, result.adopted_count, 0), "success");
      }
      screen.refresh();
      await scan();
    } catch (error) {
      screen.notify(errorMessage(error), "error");
    } finally {
      adopting.set(false);
    }
  };

  return {
    searchQuery: () => adoptSearch(),
    totalCandidates: () => adoptCandidates().length,
    rows: adoptRows,
    selectedPaths: () => selectedAdoptPaths(),
    selectedCount: selectedAdoptCount,
    scanning: () => scanningAdopt(),
    adopting: () => adopting(),
    error: () => adoptError(),
    failedBatch: () => failedBatch(),
    changeSearch: (value: string) => adoptSearch.set(value),
    clearSelection: () => selectedAdoptPaths.set({}),
    scan,
    toggleGroupExpanded: toggleAdoptGroupExpanded,
    toggleGroupSelected: toggleAdoptGroupSelected,
    toggleCandidateSelected: toggleAdoptCandidateSelected,
    confirmAdopt: confirmAndAdoptSelected,
  };
}
