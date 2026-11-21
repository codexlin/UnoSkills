import { Clipboard, Scroll, Text, View, openUrl } from "nsh";
import { Editor, Switch, TextView } from "nsh/base";
import { ActionMenu, AlertBanner, Label, LoadingLine, MenuAction, MenuDivider, PageTabs, controlMotionClass } from "../components";
import type { Theme } from "../shared";
import { ActionButton, EmptyState, hintTooltip } from "./controls";
import { isSkillDeployedTo, joinPath } from "./format";
import { errorMessage, revealPath } from "./host";
import { type Locale, busyUpdatingSkill, getStrings } from "./strings";
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

  const inspectorTabs = () => [
    { id: "deployments", glyph: "network", label: s().library_deployment },
    { id: "preview", glyph: "file-text", label: s().library_preview },
  ] as const;

  return (
    <If
      when={props.skill}
      else={
        <EmptyState
          theme={props.theme}
          glyph="file-text"
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
              gap: 8,
              paddingHorizontal: 16,
              paddingVertical: 12,
              flexShrink: 0,
            }}
          >
            <View style={{ flex: 1, minWidth: 0, gap: 4 }}>
              <Label theme={props.theme} heading size={18} truncate>
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
              <If when={props.updatingSkillId === skill().id}>
                <LoadingLine theme={props.theme} compact label={busyUpdatingSkill(props.locale, skill().name)} />
              </If>
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

                  <ActionMenu theme={props.theme} label={s().library_skill_actions}>
                    <If when={skill().source_type !== "adopted" && Boolean(skill().source_ref?.trim())}>
                      <MenuAction theme={props.theme} label={s().btn_update_skill}
                        disabled={props.updatingSkillId === skill().id} onSelect={() => props.onUpdateSkill(skill())} />
                    </If>
                    <MenuAction theme={props.theme} label={s().tooltip_reveal_folder} onSelect={() => {
                      void revealPath(skill().central_path).catch((error) => {
                        props.onNotify(errorMessage(error), "error");
                      });
                    }} />
                    <MenuAction theme={props.theme} label={s().library_copy_path} onSelect={() => {
                      void Clipboard.writeText(skill().central_path)
                        .then(() => props.onNotify(s().library_copy_path, "info"))
                        .catch((error) => props.onNotify(errorMessage(error), "error"));
                    }} />
                    <MenuDivider theme={props.theme} />
                    <MenuAction theme={props.theme} label={s().btn_delete} danger onSelect={() => props.onDeleteSkill(skill())} />
                  </ActionMenu>
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
            <PageTabs
              theme={props.theme}
              accessibilityLabel={s().nav_library}
              tabs={inspectorTabs()}
              selectedId={props.inspectorTab}
              paddingHorizontal={16}
              backgroundColor={props.theme.surface}
              onSelect={props.onTabChange}
            />
          </If>

          {/* Inspector Body */}
          <If
            when={props.editing}
            else={
              <Scroll style={{ flex: 1, minHeight: 0 }} contentStyle={{ padding: 16, gap: 12 }}>
                <If
                  when={props.inspectorTab === "preview"}
                  else={
                    <View style={{ gap: 8 }}>
                      <View style={{ flexDirection: "row", alignItems: "center", gap: 8 }}>
                        <Label theme={props.theme} heading size={14}>
                          {s().library_deployment_targets}
                        </Label>
                        <View style={{ flex: 1 }} />
                        <Text
                          style={{
                            numberOfLines: 1,
                            textOverflow: "ellipsis",
                            maxWidth: 240,
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
                              glyph="folder-open"
                              accessibilityLabel={s().tooltip_reveal_folder}
                              tooltip={s().tooltip_reveal_folder}
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
                        <Label theme={props.theme} muted size={14}>
                          {s().no_installed_agents}
                        </Label>
                      </If>

                      <If when={Boolean(props.activeProject && props.projectStatusLoading)}>
                        <LoadingLine theme={props.theme} label={s().stack_project_checking} />
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
                                minHeight: 36,
                                flexDirection: "row",
                                alignItems: "center",
                                gap: 8,
                                borderBottomWidth: 1,
                                borderColor: props.theme.border,
                                paddingVertical: 4,
                              }}
                            >
                              <Text
                                style={{
                                  numberOfLines: 1,
                                  textOverflow: "ellipsis",
                                  flex: 1,
                                  minWidth: 0,
                                  fontSize: 14,
                                  color: props.theme.foreground,
                                  fontFamily: props.theme.fontFamily,
                                }}
                              >
                                {agent().display_name}
                              </Text>

                              <If
                                when={props.activeProject}
                                else={
                                  <Switch
                                    checked={isSkillDeployedTo(skill(), agent().key)}
                                    disabled={props.deploying}
                                    accessibilityLabel={`${s().library_deployment} · ${agent().display_name} · ${s().scope_global}`}
                                    tooltip={hintTooltip(
                                      () => props.theme,
                                      () =>
                                        `${
                                          isSkillDeployedTo(skill(), agent().key)
                                            ? s().library_deployed
                                            : s().library_not_deployed
                                        } · ${s().scope_global}`,
                                    )}
                                    onChange={({ checked }) => {
                                      const current = isSkillDeployedTo(skill(), agent().key);
                                      if (checked !== current) {
                                        props.onGlobalDeployToggle(skill(), agent(), current);
                                      }
                                    }}
                                    trackClass={controlMotionClass}
                                    trackStyle={{
                                      width: 36,
                                      height: 20,
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
                                        ? [{ translateX: 16 }]
                                        : [{ translateX: 0 }],
                                    }}
                                  />
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
                                            trackClass={controlMotionClass}
                                            trackStyle={{
                                              width: 36,
                                              height: 20,
                                              padding: 2,
                                              borderRadius: 999,
                                              backgroundColor: props.theme.primary,
                                            }}
                                            thumbStyle={{
                                              width: 16,
                                              height: 16,
                                              borderRadius: 999,
                                              backgroundColor: props.theme.background,
                                              transform: [{ translateX: 16 }],
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

                    <If when={props.markdownLoading && props.previewMarkdown.length === 0}>
                      <LoadingLine theme={props.theme} label={s().library_preview} />
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
                            fontSize: 14,
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
              <Editor
                language="markdown"
                autoFocus
                softWrap
                scrollBeyondLastLine={false}
                value={props.draftMarkdown}
                onChange={({ value }) => props.onDraftChange(value)}
                placeholder={s().ph_skill_md}
                accessibilityLabel={s().btn_edit_skill}
                readOnly={props.savingMarkdown}
                fontFamily={props.theme.monoFamily ?? props.theme.fontFamily}
                fontSize={props.theme.fontSize}
                lineHeight={Math.ceil(props.theme.fontSize * 1.5)}
                styles={{ content: { paddingRight: 12 }, line: { color: props.theme.foreground },
                  gutter: { backgroundColor: props.theme.surface, borderRightWidth: 1, borderColor: props.theme.border } }}
                focusStyle={{ borderColor: props.theme.primary }}
                style={{
                  flex: 1,
                  minHeight: 0,
                  borderRadius: props.theme.radius ?? 8,
                  borderWidth: 1,
                  borderColor: props.theme.border,
                  backgroundColor: props.theme.background,
                }}
              />
            </View>
          </If>
        </View>
      )}
    </If>
  );
}
