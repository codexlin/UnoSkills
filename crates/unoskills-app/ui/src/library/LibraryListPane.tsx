import { List, Text, View } from "nsh";
import { Button } from "nsh/base";
import { Label } from "../controls";
import type { Theme } from "../shared";
import { ActionButton, AlertBanner, EmptyState, TokenBadge } from "./controls";
import { compactSource } from "./format";
import { type Locale, getStrings } from "./strings";
import type { AgentInfo, Skill } from "./types";

export function LibraryListPane(props: {
  locale: Locale;
  theme: Theme;
  skills: Skill[];
  filteredSkills: Skill[];
  activeAgents: AgentInfo[];
  selectedSkillId: string | null;
  projectScope: boolean;
  hasProjectStatuses: boolean;
  projectStatusLoading: boolean;
  projectStatusError: string | null;
  agentFilter: string | null;
  isAvailableForAgent: (skill: Skill, agentKey: string) => boolean | null;
  onSelectSkill: (skillId: string) => void;
  onOpenInstall: () => void;
  onOpenDiscover: () => void;
}) {
  const s = () => getStrings(props.locale);

  return (
    <If
      when={Boolean(
        props.projectScope &&
          props.agentFilter &&
          props.skills.length > 0 &&
          (props.projectStatusLoading || props.projectStatusError),
      )}
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
                  fontSize: 12,
                  color: props.theme.muted,
                  fontFamily: props.theme.fontFamily,
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
                  fontSize: 12,
                  color: props.theme.muted,
                  fontFamily: props.theme.fontFamily,
                  flexShrink: 0,
                }}
              >
                {props.projectScope ? s().library_project_available : s().library_deployment}
              </Text>
              <Text
                style={{
                  numberOfLines: 1,
                  textOverflow: "ellipsis",
                  width: 96,
                  textAlign: "right",
                  fontSize: 12,
                  color: props.theme.muted,
                  fontFamily: props.theme.fontFamily,
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
                estimatedItemSize={52}
                contentStyle={{ paddingVertical: 6 }}
                style={{ flex: 1, minHeight: 0 }}
              >
                {(skill) => (
                  <View style={{ paddingHorizontal: 8, paddingVertical: 2 }}>
                    <Button
                      selected={props.selectedSkillId === skill().id}
                      accessibilityLabel={skill().name}
                      onPress={() => props.onSelectSkill(skill().id)}
                      style={{
                        height: 48,
                        flexDirection: "row",
                        alignItems: "center",
                        gap: 8,
                        paddingHorizontal: 10,
                        borderRadius: props.theme.radius ?? 8,
                        backgroundColor:
                          props.selectedSkillId === skill().id
                            ? props.theme.accent
                            : props.theme.background,
                      }}
                      hoverStyle={{ backgroundColor: props.theme.accent }}
                      selectedStyle={{ backgroundColor: props.theme.accent }}
                    >
                      <View style={{ flex: 1, minWidth: 0, gap: 2 }}>
                        <Text
                          style={{
                            numberOfLines: 1,
                            textOverflow: "ellipsis",
                            fontSize: 13,
                            fontWeight: "600",
                            color: props.theme.foreground,
                            fontFamily: props.theme.fontFamily,
                          }}
                        >
                          {skill().name}
                        </Text>
                        <Text
                          style={{
                            numberOfLines: 1,
                            textOverflow: "ellipsis",
                            fontSize: 11,
                            color: props.theme.muted,
                            fontFamily: props.theme.fontFamily,
                          }}
                        >
                          {compactSource(
                            skill().source_ref ?? skill().source_type,
                            skill().source_type,
                          )}
                        </Text>
                      </View>

                      <Text
                        style={{
                          numberOfLines: 1,
                          textOverflow: "ellipsis",
                          width: 76,
                          textAlign: "right",
                          fontSize: 13,
                          color: props.theme.foreground,
                          fontFamily: props.theme.fontFamily,
                          flexShrink: 0,
                        }}
                      >
                        {props.projectScope && !props.hasProjectStatuses
                          ? "—"
                          : `${
                              props.activeAgents.filter(
                                (agent) => props.isAvailableForAgent(skill(), agent.key) === true,
                              ).length
                            } / ${props.activeAgents.length}`}
                      </Text>

                      <View
                        style={{
                          width: 96,
                          flexDirection: "row",
                          justifyContent: "flex-end",
                          flexShrink: 0,
                        }}
                      >
                        <TokenBadge theme={props.theme} tokens={skill().estimated_tokens} />
                      </View>
                    </Button>
                  </View>
                )}
              </List>
            }
          >
            <EmptyState
              theme={props.theme}
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
      <View style={{ padding: 16 }}>
        <If
          when={Boolean(props.projectStatusError)}
          else={
            <View role="summary" accessibilityState={{ busy: true }}>
              <Label theme={props.theme} muted size={13}>
                {s().stack_project_checking}
              </Label>
            </View>
          }
        >
          <AlertBanner theme={props.theme} tone="error" message={props.projectStatusError ?? ""} />
        </If>
      </View>
    </If>
  );
}
