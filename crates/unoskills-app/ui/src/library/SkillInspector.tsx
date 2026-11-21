import { Clipboard, Scroll, Text, View, openUrl } from "nsh";
import { Input, Menu, MenuItem, MenuSeparator, Switch, Tab, Tabs, TextView } from "nsh/base";
import { Label } from "../controls";
import type { Theme } from "../shared";
import { ActionButton, AlertBanner, EmptyState, hintTooltip } from "./controls";
import { isSkillDeployedTo, joinPath } from "./format";
import { errorMessage, revealPath } from "./host";
import { type Locale, getStrings } from "./strings";
import type {
  AgentInfo,
  ProjectSkillStatus,
  ProjectWorkspace,
  Skill,
  SkillLintReport,
} from "./types";

export function SkillInspector(props: {
  locale: Locale;
  theme: Theme;
  skill: Skill | null;
  activeAgents: AgentInfo[];
  activeProject: ProjectWorkspace | null;
  scopePath: string;
  scopeLabel: string;
  projectStatusLoading: boolean;
  projectStatusError: string | null;
  projectStatusFor: (skillId: string, agentKey: string) => ProjectSkillStatus | undefined;
  inspectorTab: "deployments" | "preview";
  editing: boolean;
  canEdit: boolean;
  markdownLoading: boolean;
  markdownError: string | null;
  savingMarkdown: boolean;
  draftMarkdown: string;
  previewMarkdown: string;
  selectedLint: SkillLintReport | null;
  updatingSkillId: string | null;
  deploying: boolean;
  onTabChange: (tab: "deployments" | "preview") => void;
  onStartEdit: () => void;
  onCancelEdit: () => void;
  onSaveEdit: () => void;
  onDraftChange: (value: string) => void;
  onUpdateSkill: (skill: Skill) => void;
  onDeleteSkill: (skill: Skill) => void;
  onGlobalDeployToggle: (skill: Skill, agent: AgentInfo, currentlyDeployed: boolean) => void;
  onProjectDeployToggle: (
    skill: Skill,
    project: ProjectWorkspace,
    agent: AgentInfo,
    currentlyDeployed: boolean,
  ) => void;
  onDetachProject: (project: ProjectWorkspace) => void;
  onNotify: (message: string, tone?: "info" | "success" | "error") => void;
}) {
  const s = () => getStrings(props.locale);

  const projectDisplayPath = () => {
    const project = props.activeProject;
    if (!project) {
      return "";
    }
    return props.scopePath ? joinPath(project.path, props.scopePath) : project.path;
  };

  return (
    <If
      when={props.skill}
      else={
        <EmptyState
          theme={props.theme}
          title={s().empty_select_skill_title}
          description={s().empty_select_skill_desc}
        />
      }
    >
      {(skill) => (
        <View style={{ flex: 1, minHeight: 0, minWidth: 0 }}>
          {/* Inspector Header */}
          <View
            style={{
              flexDirection: "row",
              alignItems: "center",
              gap: 10,
              paddingHorizontal: 16,
              paddingVertical: 12,
              flexShrink: 0,
            }}
          >
            <View style={{ flex: 1, minWidth: 0, gap: 3 }}>
              <Label theme={props.theme} heading size={17} truncate>
                {skill().name}
              </Label>
              <Text
                tooltip={hintTooltip(
                  () => props.theme,
                  () =>
                    `${skill().source_ref ?? skill().source_type} · ~${skill().estimated_tokens} tokens`,
                )}
                style={{
                  numberOfLines: 1,
                  textOverflow: "ellipsis",
                  fontSize: 12,
                  color: props.theme.muted,
                  fontFamily: props.theme.fontFamily,
                }}
              >
                {`${skill().source_ref ?? skill().source_type} · ~${skill().estimated_tokens} tokens`}
              </Text>
            </View>

            <If
              when={props.editing}
              else={
                <View style={{ flexDirection: "row", alignItems: "center", gap: 6, flexShrink: 0 }}>
                  <If when={props.inspectorTab === "preview"}>
                    <ActionButton
                      theme={props.theme}
                      kind="outline"
                      label={s().btn_edit_skill}
                      disabled={!props.canEdit}
                      onPress={props.onStartEdit}
                    />
                  </If>

                  <Menu
                    offset={4}
                    align="end"
                    popupStyle={{
                      minWidth: 200,
                      borderRadius: 10,
                      borderWidth: 1,
                      borderColor: props.theme.border,
                      backgroundColor: props.theme.surface,
                      padding: 4,
                    }}
                    trigger={
                      <View
                        accessibilityLabel={s().library_skill_actions}
                        tooltip={hintTooltip(
                          () => props.theme,
                          () => s().library_skill_actions,
                        )}
                        style={{
                          paddingHorizontal: 10,
                          paddingVertical: 6,
                          borderRadius: props.theme.radius ?? 8,
                          borderWidth: 1,
                          borderColor: props.theme.border,
                          backgroundColor: props.theme.background,
                        }}
                      >
                        <Text style={{ fontSize: 14, color: props.theme.foreground }}>⋯</Text>
                      </View>
                    }
                  >
                    <If
                      when={
                        skill().source_type !== "adopted" &&
                        Boolean(skill().source_ref && skill().source_ref!.trim().length > 0)
                      }
                    >
                      <MenuItem
                        disabled={props.updatingSkillId === skill().id}
                        onSelect={() => props.onUpdateSkill(skill())}
                        style={{ paddingHorizontal: 10, paddingVertical: 7, borderRadius: 6 }}
                        focusStyle={{ backgroundColor: props.theme.accent }}
                        hoverStyle={{ backgroundColor: props.theme.accent }}
                        disabledStyle={{ opacity: 0.45 }}
                      >
                        <Label theme={props.theme} size={13}>
                          {s().btn_update_skill}
                        </Label>
                      </MenuItem>
                    </If>

                    <MenuItem
                      onSelect={() => {
                        void revealPath(skill().central_path).catch((error) => {
                          props.onNotify(errorMessage(error), "error");
                        });
                      }}
                      style={{ paddingHorizontal: 10, paddingVertical: 7, borderRadius: 6 }}
                      focusStyle={{ backgroundColor: props.theme.accent }}
                      hoverStyle={{ backgroundColor: props.theme.accent }}
                    >
                      <Label theme={props.theme} size={13}>
                        {s().tooltip_reveal_folder}
                      </Label>
                    </MenuItem>

                    <MenuItem
                      onSelect={() => {
                        void Clipboard.writeText(skill().central_path)
                          .then(() => props.onNotify(s().library_copy_path, "info"))
                          .catch((error) => props.onNotify(errorMessage(error), "error"));
                      }}
                      style={{ paddingHorizontal: 10, paddingVertical: 7, borderRadius: 6 }}
                      focusStyle={{ backgroundColor: props.theme.accent }}
                      hoverStyle={{ backgroundColor: props.theme.accent }}
                    >
                      <Label theme={props.theme} size={13}>
                        {s().library_copy_path}
                      </Label>
                    </MenuItem>

                    <MenuSeparator
                      style={{ height: 1, marginVertical: 4, backgroundColor: props.theme.border }}
                    />

                    <MenuItem
                      onSelect={() => props.onDeleteSkill(skill())}
                      style={{ paddingHorizontal: 10, paddingVertical: 7, borderRadius: 6 }}
                      focusStyle={{ backgroundColor: props.theme.accent }}
                      hoverStyle={{ backgroundColor: props.theme.accent }}
                    >
                      <Label theme={props.theme} size={13} color={props.theme.danger}>
                        {s().btn_delete}
                      </Label>
                    </MenuItem>
                  </Menu>
                </View>
              }
            >
              <View style={{ flexDirection: "row", alignItems: "center", gap: 6, flexShrink: 0 }}>
                <ActionButton
                  theme={props.theme}
                  kind="ghost"
                  label={s().btn_cancel}
                  disabled={props.savingMarkdown}
                  onPress={props.onCancelEdit}
                />
                <ActionButton
                  theme={props.theme}
                  kind="primary"
                  label={s().btn_save}
                  loading={props.savingMarkdown}
                  onPress={props.onSaveEdit}
                />
              </View>
            </If>
          </View>

          {/* Sub-tabs: Deployments | Preview */}
          <If when={!props.editing}>
            <View
              style={{
                paddingHorizontal: 16,
                borderBottomWidth: 1,
                borderColor: props.theme.border,
                flexShrink: 0,
              }}
            >
              <Tabs
                accessibilityLabel={s().nav_library}
                style={{ flexDirection: "row", gap: 16 }}
                indicatorStyle={{ bottom: 0, height: 2, backgroundColor: props.theme.primary }}
              >
                <Tab
                  selected={props.inspectorTab === "deployments"}
                  onPress={() => props.onTabChange("deployments")}
                  style={{ paddingVertical: 9 }}
                >
                  <Text
                    style={{
                      fontSize: 13,
                      fontWeight: props.inspectorTab === "deployments" ? "600" : "500",
                      color:
                        props.inspectorTab === "deployments"
                          ? props.theme.foreground
                          : props.theme.muted,
                      fontFamily: props.theme.fontFamily,
                    }}
                  >
                    {s().library_deployment}
                  </Text>
                </Tab>
                <Tab
                  selected={props.inspectorTab === "preview"}
                  onPress={() => props.onTabChange("preview")}
                  style={{ paddingVertical: 9 }}
                >
                  <Text
                    style={{
                      fontSize: 13,
                      fontWeight: props.inspectorTab === "preview" ? "600" : "500",
                      color:
                        props.inspectorTab === "preview"
                          ? props.theme.foreground
                          : props.theme.muted,
                      fontFamily: props.theme.fontFamily,
                    }}
                  >
                    {s().library_preview}
                  </Text>
                </Tab>
              </Tabs>
            </View>
          </If>

          {/* Inspector Body */}
          <If
            when={props.editing}
            else={
              <Scroll style={{ flex: 1, minHeight: 0 }} contentStyle={{ padding: 16, gap: 12 }}>
                <If
                  when={props.inspectorTab === "preview"}
                  else={
                    <View style={{ gap: 10 }}>
                      <View style={{ flexDirection: "row", alignItems: "center", gap: 8 }}>
                        <Label theme={props.theme} heading size={14}>
                          {s().library_deployment_targets}
                        </Label>
                        <View style={{ flex: 1 }} />
                        <Text
                          style={{
                            numberOfLines: 1,
                            textOverflow: "ellipsis",
                            maxWidth: 260,
                            fontSize: 12,
                            color: props.theme.muted,
                            fontFamily: props.theme.fontFamily,
                            flexShrink: 0,
                          }}
                        >
                          {props.scopeLabel}
                        </Text>
                      </View>

                      <If when={props.activeProject}>
                        {(project) => (
                          <View
                            style={{
                              flexDirection: "row",
                              alignItems: "center",
                              gap: 8,
                              paddingVertical: 4,
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
                                fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                              }}
                            >
                              {projectDisplayPath()}
                            </Text>
                            <ActionButton
                              theme={props.theme}
                              kind="ghost"
                              compact
                              label={s().tooltip_reveal_folder}
                              onPress={() => {
                                void revealPath(projectDisplayPath()).catch((error) => {
                                  props.onNotify(errorMessage(error), "error");
                                });
                              }}
                            />
                            <ActionButton
                              theme={props.theme}
                              kind="ghost"
                              compact
                              label={s().btn_detach_project}
                              onPress={() => props.onDetachProject(project())}
                            />
                          </View>
                        )}
                      </If>

                      <If when={props.activeAgents.length === 0}>
                        <Label theme={props.theme} muted size={13}>
                          {s().no_installed_agents}
                        </Label>
                      </If>

                      <If when={Boolean(props.activeProject && props.projectStatusLoading)}>
                        <View role="summary" accessibilityState={{ busy: true }}>
                          <Label theme={props.theme} muted size={12}>
                            {s().stack_project_checking}
                          </Label>
                        </View>
                      </If>

                      <If when={Boolean(props.activeProject && props.projectStatusError)}>
                        <AlertBanner
                          theme={props.theme}
                          tone="error"
                          message={props.projectStatusError ?? ""}
                        />
                      </If>

                      <View style={{ borderTopWidth: 1, borderColor: props.theme.border }}>
                        <Each of={props.activeAgents} key={(agent) => agent.key}>
                          {(agent) => (
                            <View
                              style={{
                                minHeight: 42,
                                flexDirection: "row",
                                alignItems: "center",
                                gap: 10,
                                borderBottomWidth: 1,
                                borderColor: props.theme.border,
                                paddingVertical: 6,
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
                                {agent().display_name}
                              </Text>

                              <If
                                when={props.activeProject}
                                else={
                                  <View
                                    style={{
                                      flexDirection: "row",
                                      alignItems: "center",
                                      gap: 8,
                                      flexShrink: 0,
                                    }}
                                  >
                                    <Label
                                      theme={props.theme}
                                      size={12}
                                      muted={!isSkillDeployedTo(skill(), agent().key)}
                                    >
                                      {isSkillDeployedTo(skill(), agent().key)
                                        ? s().library_deployed
                                        : s().library_not_deployed}
                                    </Label>
                                    <Switch
                                      checked={isSkillDeployedTo(skill(), agent().key)}
                                      disabled={props.deploying}
                                      accessibilityLabel={`${s().library_deployment} · ${agent().display_name} · ${s().scope_global}`}
                                      onChange={({ checked }) => {
                                        const current = isSkillDeployedTo(skill(), agent().key);
                                        if (checked !== current) {
                                          props.onGlobalDeployToggle(skill(), agent(), current);
                                        }
                                      }}
                                      trackStyle={{
                                        width: 38,
                                        padding: 2,
                                        borderRadius: 999,
                                        backgroundColor: isSkillDeployedTo(skill(), agent().key)
                                          ? props.theme.primary
                                          : props.theme.border,
                                      }}
                                      thumbStyle={{
                                        width: 16,
                                        height: 16,
                                        borderRadius: 999,
                                        backgroundColor: props.theme.background,
                                        transform: isSkillDeployedTo(skill(), agent().key)
                                          ? [{ translateX: 18 }]
                                          : [{ translateX: 0 }],
                                      }}
                                    />
                                  </View>
                                }
                              >
                                {(project) => (
                                  <If when={props.projectStatusFor(skill().id, agent().key)}>
                                    {(status) => (
                                      <View
                                        style={{
                                          flexDirection: "row",
                                          alignItems: "center",
                                          gap: 8,
                                          flexShrink: 0,
                                        }}
                                      >
                                        <Label
                                          theme={props.theme}
                                          size={12}
                                          color={
                                            status().placement === "conflict" ||
                                            status().placement === "unavailable"
                                              ? props.theme.warning
                                              : status().placement === "managed" ||
                                                  status().placement === "local"
                                                ? props.theme.foreground
                                                : props.theme.muted
                                          }
                                        >
                                          {status().placement === "managed"
                                            ? s().library_project_deployed
                                            : status().placement === "local"
                                              ? s().library_project_local
                                              : status().placement === "conflict"
                                                ? s().library_project_conflict
                                                : status().placement === "unavailable"
                                                  ? s().library_project_unavailable
                                                  : status().global_available
                                                    ? s().library_global_available
                                                    : s().library_not_deployed}
                                        </Label>

                                        <If when={status().placement === "managed"}>
                                          <Switch
                                            checked
                                            disabled={props.deploying}
                                            accessibilityLabel={`${s().library_project_deployed} · ${agent().display_name}`}
                                            tooltip={hintTooltip(
                                              () => props.theme,
                                              () => s().library_remove_project_deployment,
                                            )}
                                            onChange={({ checked }) => {
                                              if (!checked) {
                                                props.onProjectDeployToggle(
                                                  skill(),
                                                  project(),
                                                  agent(),
                                                  true,
                                                );
                                              }
                                            }}
                                            trackStyle={{
                                              width: 38,
                                              padding: 2,
                                              borderRadius: 999,
                                              backgroundColor: props.theme.primary,
                                            }}
                                            thumbStyle={{
                                              width: 16,
                                              height: 16,
                                              borderRadius: 999,
                                              backgroundColor: props.theme.background,
                                              transform: [{ translateX: 18 }],
                                            }}
                                          />
                                        </If>

                                        <If
                                          when={
                                            status().placement === "missing" ||
                                            status().placement === "conflict"
                                          }
                                        >
                                          <ActionButton
                                            theme={props.theme}
                                            kind="ghost"
                                            compact
                                            disabled={props.deploying}
                                            label={
                                              status().placement === "conflict"
                                                ? s().library_resolve_project_conflict
                                                : s().stack_deploy_project
                                            }
                                            accessibilityLabel={`${
                                              status().placement === "conflict"
                                                ? s().library_resolve_project_conflict
                                                : s().stack_deploy_project
                                            } · ${agent().display_name}`}
                                            onPress={() =>
                                              props.onProjectDeployToggle(
                                                skill(),
                                                project(),
                                                agent(),
                                                false,
                                              )
                                            }
                                          />
                                        </If>
                                      </View>
                                    )}
                                  </If>
                                )}
                              </If>
                            </View>
                          )}
                        </Each>
                      </View>
                    </View>
                  }
                >
                  {/* SKILL.md Preview + Lint / Read Error */}
                  <View style={{ maxWidth: 720, gap: 12 }}>
                    <If when={Boolean(props.markdownError)}>
                      <AlertBanner
                        theme={props.theme}
                        tone="error"
                        message={props.markdownError ?? ""}
                      />
                    </If>

                    <If
                      when={Boolean(
                        props.selectedLint && props.selectedLint.warnings.length > 0,
                      )}
                    >
                      <AlertBanner
                        theme={props.theme}
                        tone="warning"
                        message={(props.selectedLint?.warnings ?? []).join(" · ")}
                      />
                    </If>

                    <If
                      when={Boolean(
                        skill().description && skill().description!.trim().length > 0,
                      )}
                    >
                      <View
                        style={{
                          paddingBottom: 12,
                          borderBottomWidth: 1,
                          borderColor: props.theme.border,
                        }}
                      >
                        <Text
                          style={{
                            fontSize: 13,
                            lineHeight: 20,
                            color: props.theme.foreground,
                            fontFamily: props.theme.fontFamily,
                          }}
                        >
                          {skill().description ?? ""}
                        </Text>
                      </View>
                    </If>

                    <TextView
                      markdown={props.previewMarkdown}
                      selectable
                      onLinkPress={({ url }) => {
                        void openUrl(url);
                      }}
                      styles={{
                        root: {
                          color: props.theme.foreground,
                          fontFamily: props.theme.fontFamily,
                          fontSize: 14,
                          lineHeight: 22,
                        },
                        link: { color: props.theme.primary },
                        code: {
                          fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                          backgroundColor: props.theme.background,
                        },
                        codeBlock: {
                          backgroundColor: props.theme.background,
                          borderColor: props.theme.border,
                        },
                        codeBlockCode: {
                          fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                        },
                      }}
                    />
                  </View>
                </If>
              </Scroll>
            }
          >
            {/* Quick Edit Mode (raw frontmatter preserved, tied to loaded skill ID) */}
            <View style={{ flex: 1, minHeight: 0, padding: 16 }}>
              <Input
                multiline
                submitBehavior="newline"
                autoCorrect={false}
                value={props.draftMarkdown}
                onChange={({ value }) => props.onDraftChange(value)}
                placeholder={s().ph_skill_md}
                accessibilityLabel={s().btn_edit_skill}
                disabled={props.savingMarkdown}
                style={{
                  flex: 1,
                  minHeight: 0,
                  fontSize: 13,
                  lineHeight: 20,
                  padding: 12,
                  borderRadius: props.theme.radius ?? 8,
                  borderWidth: 1,
                  borderColor: props.theme.border,
                  backgroundColor: props.theme.background,
                  color: props.theme.foreground,
                  placeholderColor: props.theme.muted,
                  fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
                }}
              />
            </View>
          </If>
        </View>
      )}
    </If>
  );
}
