import { List, Text, View } from "nsh";
import { Button } from "nsh/base";
import { AlertBanner, Label, LoadingLine, controlMotionClass, focusOutline, interactionStyles, textSize, textStyle } from "../components";
import type { Theme } from "../shared";
import { ActionButton, EmptyState, TokenBadge } from "./controls";
import { compactSource } from "./format";
import { managementStrings } from "./managementStrings";
import { type Locale, getStrings } from "./strings";
import type { AgentInfo, ProjectSkillStatus, Skill } from "./types";

export function LibraryListPane(props: {
  locale: Locale;
  theme: Theme;
  skills: Skill[];
  filteredSkills: Skill[];
  activeAgents: AgentInfo[];
  selectedSkillId: string | null;
  projectScope: boolean;
  hasProjectStatuses: boolean;
  scopeBlocksList: boolean;
  projectStatusError: string | null;
  isAvailableForAgent: (skill: Skill, agentKey: string) => boolean | null;
  agentFilter: string | null;
  projectStatusFor: (skillId: string, agentKey: string) => ProjectSkillStatus | undefined;
  onSelectSkill: (skillId: string) => void;
  onOpenInstall: () => void;
  onOpenDiscover: () => void;
}) {
  const s = () => getStrings(props.locale);
  const m = () => managementStrings(props.locale);
  const rowHeight = () => Math.max(48, textSize(props.theme, 48));

  function projectLabel(skill: Skill) {
    if (!props.projectScope) return undefined;
    const key = props.agentFilter;
    const status = key ? props.projectStatusFor(skill.id, key) : undefined;
    if (status?.placement === "conflict") return m().conflict;
    if (status?.placement === "managed") return m().managed;
    if (status?.placement === "local") return m().local;
    if (props.activeAgents.some((agent) => props.projectStatusFor(skill.id, agent.key)?.retained_pin)) return m().pinLabel;
    return undefined;
  }

  function availability(skill: Skill) {
    if (props.projectScope && !props.hasProjectStatuses) return "—";
    const available = props.activeAgents.filter((agent) => props.isAvailableForAgent(skill, agent.key) === true).length;
    return available + " / " + props.activeAgents.length;
  }

  return (
    <If
      when={props.scopeBlocksList}
      else={
        <View style={{ flex: 1, minHeight: 0, minWidth: 0 }}>
          <If when={props.filteredSkills.length > 0}>
            <View
              style={{
                height: 32,
                flexDirection: "row",
                alignItems: "center",
                paddingHorizontal: 16,
                gap: 8,
                borderBottomWidth: 1,
                borderColor: props.theme.border,
                flexShrink: 0,
              }}
            >
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
                {s().library_skill_column}
              </Text>
              <Text
                style={{
                  numberOfLines: 1,
                  textOverflow: "ellipsis",
                  width: 76,
                  textAlign: "right",
                  ...textStyle(props.theme, { size: textSize(props.theme, 12) }),
                  color: props.theme.muted,
                  flexShrink: 0,
                }}
              >
                {props.projectScope && !props.agentFilter ? s().library_project_available : s().library_deployment}
              </Text>
              <Text
                style={{
                  numberOfLines: 1,
                  textOverflow: "ellipsis",
                  width: 96,
                  textAlign: "right",
                  ...textStyle(props.theme, { size: textSize(props.theme, 12) }),
                  color: props.theme.muted,
                  flexShrink: 0,
                }}
              >
                {s().library_token_column}
              </Text>
            </View>
          </If>

          <If
            when={props.filteredSkills.length === 0}
            else={
              <List
                each={props.filteredSkills}
                keyExtractor={(skill) => skill.id}
                estimatedItemSize={rowHeight() + 4}
                contentStyle={{ paddingVertical: 4 }}
                style={{ flex: 1, minHeight: 0 }}
              >
                {(skill) => {
                  const selected = () => props.selectedSkillId === skill().id;
                  const states = () => interactionStyles(props.theme, { selectedFill: selected() ? props.theme.accent : undefined });
                  return <View style={{ paddingHorizontal: 8, paddingVertical: 2 }}>
                    <Button
                      selected={selected()}
                      accessibilityLabel={skill().name}
                      onPress={() => props.onSelectSkill(skill().id)}
                      class={controlMotionClass}
                      style={{
                        height: rowHeight(),
                        flexDirection: "row",
                        alignItems: "center",
                        gap: 8,
                        paddingHorizontal: 8,
                        borderRadius: props.theme.control.radius,
                        borderWidth: 1,
                        borderColor:
                          selected()
                            ? props.theme.primary
                            : "transparent",
                        backgroundColor:
                          selected()
                            ? props.theme.accent
                            : props.theme.background,
                      }}
                      hoverStyle={states().hoverStyle}
                      activeStyle={states().activeStyle}
                      focusVisibleStyle={focusOutline(props.theme)}
                      selectedStyle={{
                        backgroundColor: props.theme.accent,
                        borderColor: props.theme.primary,
                      }}
                    >
                      <View style={{ flex: 1, minWidth: 0, gap: 2 }}>
                        <Text
                          style={{
                            numberOfLines: 1,
                            textOverflow: "ellipsis",
                            ...textStyle(props.theme),
                            fontWeight: "600",
                            color: selected() ? props.theme.onAccent : props.theme.foreground,
                          }}
                        >
                          {skill().name}
                        </Text>
                        <Text
                          style={{
                            numberOfLines: 1,
                            textOverflow: "ellipsis",
                            ...textStyle(props.theme, { size: textSize(props.theme, 12) }),
                            color: selected() ? props.theme.onAccent : props.theme.muted,
                          }}
                        >
                          {compactSource(
                            skill().source_ref ?? skill().source_type,
                            skill().source_type,
                          )}
                        </Text>
                      </View>

                      <View style={{ width: 76, flexShrink: 0, alignItems: "flex-end" }}>
                        <Label theme={props.theme} size={12} truncate
                          color={selected() ? props.theme.onAccent : props.theme.foreground}>
                          {projectLabel(skill()) ?? availability(skill())}
                        </Label>
                      </View>

                      <View
                        style={{
                          width: 96,
                          flexDirection: "row",
                          justifyContent: "flex-end",
                          flexShrink: 0,
                        }}
                      >
                        <TokenBadge theme={props.theme} tokens={skill().estimated_tokens} selected={selected()} />
                      </View>
                    </Button>
                  </View>;
                }}
              </List>
            }
          >
            <EmptyState
              theme={props.theme}
              glyph="book-open"
              title={
                props.skills.length === 0 ? s().empty_library_title : s().empty_search_title
              }
              description={
                props.skills.length === 0 ? s().empty_library_desc : s().empty_search_desc
              }
              actions={
                props.skills.length === 0 ? (
                  <>
                    <ActionButton
                      theme={props.theme}
                      kind="primary"
                      label={s().btn_install}
                      onPress={props.onOpenInstall}
                    />
                    <ActionButton
                      theme={props.theme}
                      kind="outline"
                      label={s().btn_discover_adopt}
                      onPress={props.onOpenDiscover}
                    />
                  </>
                ) : undefined
              }
            />
          </If>
        </View>
      }
    >
      <If
        when={Boolean(props.projectStatusError)}
        else={
          <View
            style={{
              flex: 1,
              alignItems: "center",
              justifyContent: "center",
              padding: 16,
            }}
          >
            <LoadingLine theme={props.theme} label={s().stack_project_checking} />
          </View>
        }
      >
        <View style={{ padding: 16 }}>
          <AlertBanner theme={props.theme} tone="error" message={props.projectStatusError ?? ""} />
        </View>
      </If>
    </If>
  );
}
