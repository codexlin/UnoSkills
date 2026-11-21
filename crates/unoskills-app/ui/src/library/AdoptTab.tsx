import { type InputRef, List, type ListRef, Text, View } from "nsh";
import { Checkbox } from "nsh/base";
import { ActionControl, AlertBanner, Label, focusOutline, interactionStyles } from "../components";
import type { Theme } from "../shared";
import { ActionButton, EmptyState, SearchField, TokenBadge, hintTooltip } from "./controls";
import { type Locale, adoptSelectedCount, getStrings, labelWithCount } from "./strings";
import type { AdoptCandidate } from "./types";

export interface AdoptGroup {
  key: string;
  name: string;
  candidates: AdoptCandidate[];
  expanded: boolean;
}

export type AdoptRow =
  | { kind: "group"; id: string; group: AdoptGroup }
  | { kind: "candidate"; id: string; candidate: AdoptCandidate };

export function AdoptTab(props: {
  locale: Locale;
  theme: Theme;
  searchQuery: string;
  totalCandidates: number;
  rows: AdoptRow[];
  selectedPaths: Record<string, boolean>;
  selectedCount: number;
  scanning: boolean;
  adopting: boolean;
  error: string | null;
  onSearchInputRef: (ref: InputRef) => void;
  onListRef: (ref: ListRef) => void;
  onSearchChange: (value: string) => void;
  onRescan: () => void;
  onToggleGroupExpanded: (groupKey: string) => void;
  onToggleGroupSelected: (group: AdoptGroup) => void;
  onToggleCandidateSelected: (path: string, checked: boolean) => void;
  onClearSelection: () => void;
  onConfirmAdopt: () => void;
}) {
  const s = () => getStrings(props.locale);

  return (
    <View style={{ flex: 1, minHeight: 0, minWidth: 0 }}>
      <View
        style={{
          minHeight: 48,
          flexDirection: "row",
          flexWrap: "wrap",
          alignItems: "center",
          gap: 8,
          paddingHorizontal: 20,
          paddingVertical: 8,
          borderBottomWidth: 1,
          borderColor: props.theme.border,
          flexShrink: 0,
        }}
      >
        <SearchField
          theme={props.theme}
          inputRef={props.onSearchInputRef}
          value={props.searchQuery}
          placeholder={s().ph_adopt_search}
          accessibilityLabel={s().ph_adopt_search}
          clearLabel={s().adopt_clear_selection}
          minWidth={180}
          maxWidth={360}
          onChange={props.onSearchChange}
        />

        <ActionButton
          theme={props.theme}
          kind="outline"
          icon="↻"
          label={s().btn_rescan}
          loading={props.scanning}
          disabled={props.adopting}
          onPress={props.onRescan}
        />
      </View>

      <If when={Boolean(props.error)}>
        <View style={{ paddingHorizontal: 20, paddingTop: 10 }}>
          <AlertBanner theme={props.theme} tone="error" message={props.error ?? ""} />
        </View>
      </If>

      <If
        when={props.rows.length === 0}
        else={
          <List
            ref={props.onListRef}
            each={props.rows}
            keyExtractor={(row) => row.id}
            estimatedItemSize={40}
            style={{ flex: 1, minHeight: 0 }}
          >
            {(row) => (
              <View style={{ paddingHorizontal: 20 }}>
                <If
                  when={row().kind === "group"}
                  else={
                    <View
                      style={{
                        height: 40,
                        flexDirection: "row",
                        alignItems: "center",
                        gap: 8,
                        paddingHorizontal: 8,
                        borderBottomWidth: 1,
                        borderColor: props.theme.border,
                      }}
                    >
                      <Checkbox
                        hitSlop={8}
                        checked={Boolean(
                          row().kind === "candidate" &&
                            props.selectedPaths[
                              (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.path
                            ],
                        )}
                        disabled={props.adopting}
                        accessibilityLabel={
                          row().kind === "candidate"
                            ? (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.name
                            : ""
                        }
                        onChange={({ checked }) => {
                          const current = row();
                          if (current.kind === "candidate") {
                            props.onToggleCandidateSelected(current.candidate.path, checked);
                          }
                        }}
                        hoverStyle={interactionStyles(props.theme).hoverStyle}
                        activeStyle={interactionStyles(props.theme).activeStyle}
                        focusVisibleStyle={focusOutline(props.theme)}
                        indicatorClass="transition-colors"
                        indicatorStyle={{
                          width: 16,
                          height: 16,
                          borderRadius: 4,
                          borderWidth: 1,
                          borderColor:
                            row().kind === "candidate" &&
                            props.selectedPaths[
                              (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.path
                            ]
                              ? props.theme.primary
                              : props.theme.border,
                          backgroundColor:
                            row().kind === "candidate" &&
                            props.selectedPaths[
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
                            props.selectedPaths[
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
                          width: "33%",
                          minWidth: 0,
                          fontSize: 14,
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
                          fontFamily: props.theme.fontFamily,
                        }}
                      >
                        {row().kind === "candidate"
                          ? (row() as Extract<AdoptRow, { kind: "candidate" }>).candidate.path
                          : ""}
                      </Text>

                      <View style={{ flexShrink: 0 }}>
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
                    </View>
                  }
                >
                  <View
                    style={{
                      height: 40,
                      flexDirection: "row",
                      alignItems: "center",
                      justifyContent: "space-between",
                      gap: 8,
                      paddingHorizontal: 8,
                      borderRadius: 6,
                      backgroundColor: props.theme.surface,
                    }}
                  >
                    <ActionControl theme={props.theme}
                      onPress={() => {
                        const current = row();
                        if (current.kind === "group") {
                          props.onToggleGroupExpanded(current.group.key);
                        }
                      }}
                      class="flex-row items-center gap-1.5 rounded-md px-1.5 py-1 transition-colors"
                      style={{
                        flexShrink: 1,
                        minWidth: 0,
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
                          flexShrink: 1,
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
                    </ActionControl>

                    <View style={{ flex: 1 }} />

                    <ActionButton
                      theme={props.theme}
                      kind="ghost"
                      compact
                      disabled={props.adopting}
                      label={
                        row().kind === "group" &&
                        (row() as Extract<AdoptRow, { kind: "group" }>).group.candidates.every(
                          (candidate) => Boolean(props.selectedPaths[candidate.path]),
                        )
                          ? s().adopt_clear_selection
                          : s().adopt_select_group
                      }
                      onPress={() => {
                        const current = row();
                        if (current.kind === "group") {
                          props.onToggleGroupSelected(current.group);
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
          icon="✓"
          title={
            props.totalCandidates === 0 ? s().empty_adopt_done_title : s().empty_adopt_filter_title
          }
          description={
            props.totalCandidates === 0 ? s().empty_adopt_done_desc : s().empty_adopt_filter_desc
          }
        />
      </If>

      <If when={props.selectedCount > 0}>
        <View
          style={{
            minHeight: 48,
            flexDirection: "row",
            alignItems: "center",
            gap: 8,
            paddingHorizontal: 20,
            paddingVertical: 8,
            borderTopWidth: 1,
            borderColor: props.theme.border,
            backgroundColor: props.theme.background,
            flexShrink: 0,
          }}
        >
          <View style={{ flex: 1, minWidth: 0 }}>
            <Label theme={props.theme} size={14}>
              {adoptSelectedCount(props.locale, props.selectedCount)}
            </Label>
          </View>
          <ActionButton
            theme={props.theme}
            kind="ghost"
            disabled={props.adopting}
            label={s().adopt_clear_selection}
            onPress={props.onClearSelection}
          />
          <ActionButton
            theme={props.theme}
            kind="primary"
            loading={props.adopting}
            label={labelWithCount(props.locale, s().btn_adopt, props.selectedCount)}
            onPress={props.onConfirmAdopt}
          />
        </View>
      </If>
    </View>
  );
}
