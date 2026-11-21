import { Scroll, View, cleanup, effect, signal, untracked } from "nsh";
import { Action, Label, Modal } from "../components";
import type { ScreenProps } from "../shared";
import { applySkillRevision, cancelSkillRevision, errorMessage, prepareSkillRevision } from "./host";
import { toastSkillUpToDate, toastSkillUpdated } from "./strings";
import type { AgentInfo, DeploymentImpact, Skill, SkillRevisionPreview } from "./types";

const translations = {
  en: {
    preview: "Review skill update", restore: "Restore previous version",
    files: "File changes", deployments: "Deployment impact",
    snapshot: "UnoSkills keeps the current skill files as the previous version.",
    empty: "No recorded deployments.", update: "Apply update", cancel: "Cancel", close: "Close",
    noPrevious: "There is no different previous version to restore.",
    restored: "Previous version restored.", issues: "Deployments to review",
    stale: "The skill or its deployments changed. Check again before applying.",
    added: "Added", modified: "Modified", removed: "Removed",
    linked: "Follows the library", current: "Already current", refresh_copy: "Copy will be refreshed",
    missing: "Missing deployment. Re-deploy it from Deployment targets.",
    conflict: "Content or location changed. Back up your edits before replacing this deployment.",
    refresh_failed: "Copy refresh failed. Check this path and try deploying again.",
  },
  "zh-CN": {
    preview: "预览技能更新", restore: "恢复上一个版本",
    files: "文件变更", deployments: "部署影响",
    snapshot: "UnoSkills 会把当前技能文件保存为上一个版本。",
    empty: "没有已记录的部署。", update: "确认更新", cancel: "取消", close: "关闭",
    noPrevious: "没有可恢复的不同版本。",
    restored: "已恢复上一个版本。", issues: "需要核对的部署",
    stale: "技能或部署已变化，请重新检查后再应用。",
    added: "新增", modified: "修改", removed: "删除",
    linked: "跟随技能库更新", current: "已是目标版本", refresh_copy: "将刷新副本",
    missing: "部署已缺失。可在「部署目标」重新部署。",
    conflict: "内容或位置已变化。替换此部署前，请先备份手工修改的文件。",
    refresh_failed: "刷新副本失败。请检查路径后重试部署。",
  },
  ja: {
    preview: "スキル更新を確認", restore: "前のバージョンを復元",
    files: "ファイルの変更", deployments: "配備への影響",
    snapshot: "現在のスキルファイルを前のバージョンとして保存します。",
    empty: "記録された配備はありません。", update: "更新を適用", cancel: "キャンセル", close: "閉じる",
    noPrevious: "復元できる異なるバージョンがありません。",
    restored: "前のバージョンを復元しました。", issues: "確認が必要な配備",
    stale: "スキルまたは配備が変更されました。適用前に確認し直してください。",
    added: "追加", modified: "変更", removed: "削除",
    linked: "ライブラリに追従", current: "対象バージョンと一致", refresh_copy: "コピーを更新",
    missing: "配備が見つかりません。配備先から再配備できます。",
    conflict: "内容または場所が変更されました。置き換える前に手動編集したファイルをバックアップしてください。",
    refresh_failed: "コピーを更新できませんでした。パスを確認して再配備してください。",
  },
} as const;

export function skillRevisionStrings(locale: ScreenProps["locale"]) {
  return translations[locale];
}

type RevisionDialogState = { preview: SkillRevisionPreview } | { issues: DeploymentImpact[]; skillId: string };

export function createSkillRevisions(
  props: ScreenProps,
  selectedSkillId: () => string | null,
  active: () => boolean,
  onApplied: () => void,
) {
  const dialog = signal<RevisionDialogState | null>(null);
  const pending = signal<string | null>(null);
  const applying = signal(false);
  const t = () => skillRevisionStrings(props.locale);
  let disposed = false;
  let viewRevision = 0;

  function release(preview: SkillRevisionPreview) {
    void cancelSkillRevision(preview.id).catch(() => {});
  }

  function close() {
    const current = dialog();
    dialog.set(null);
    if (current && "preview" in current && !applying()) release(current.preview);
  }

  effect(() => {
    const currentSkill = selectedSkillId();
    const visible = active();
    viewRevision += 1;
    const current = untracked(dialog);
    const dialogSkill = current && ("preview" in current ? current.preview.skill_id : current.skillId);
    if (current && (!visible || dialogSkill !== currentSkill)) untracked(close);
  });

  cleanup(() => {
    disposed = true;
    close();
  });

  async function prepare(skill: Skill, rollback: boolean) {
    if (pending() || dialog()) return;
    const revision = viewRevision;
    pending.set(skill.id);
    try {
      const preview = await prepareSkillRevision(skill.id, rollback);
      if (disposed || viewRevision !== revision || !active() || selectedSkillId() !== skill.id) {
        if (preview) release(preview);
        return;
      }
      if (preview) dialog.set({ preview });
      else props.notify(rollback ? t().noPrevious : toastSkillUpToDate(props.locale, skill.name), "info");
    } catch (error) {
      if (!disposed && viewRevision === revision) props.notify(message(error), "error");
    } finally {
      pending.set(null);
    }
  }

  function message(error: unknown): string {
    if ((error as { code?: string } | null)?.code === "STALE_SKILL_REVISION") return t().stale;
    return errorMessage(error);
  }

  async function confirm(preview: SkillRevisionPreview) {
    const current = dialog();
    if (applying() || !active() || selectedSkillId() !== preview.skill_id ||
      !current || !("preview" in current) || current.preview.id !== preview.id) return;
    applying.set(true);
    pending.set(preview.skill_id);
    try {
      const result = await applySkillRevision(preview.id);
      if (disposed) return;
      if (active() && selectedSkillId() === preview.skill_id) {
        const issues = result.deployment_issues ?? [];
        dialog.set(issues.length ? { issues, skillId: preview.skill_id } : null);
      }
      props.notify(preview.rollback ? t().restored : toastSkillUpdated(
        props.locale, result.skill.name, result.skill.estimated_tokens, result.refreshed_copies,
      ), "success");
      onApplied();
    } catch (error) {
      if (!disposed) props.notify(message(error), "error");
      dialog.set(null);
    } finally {
      applying.set(false);
      pending.set(null);
    }
  }

  return { dialog, pending, applying, prepare, confirm, close: () => { if (!applying()) close(); } };
}

export function SkillRevisionDialog(props: {
  theme: ScreenProps["theme"];
  locale: ScreenProps["locale"];
  agents: AgentInfo[];
  state: RevisionDialogState;
  applying: boolean;
  onClose: () => void;
  onConfirm: (preview: SkillRevisionPreview) => void;
}) {
  const t = () => skillRevisionStrings(props.locale);
  const preview = () => "preview" in props.state ? props.state.preview : null;
  const deployments = () => "issues" in props.state ? props.state.issues : props.state.preview.deployments;
  const agentName = (key: string) => props.agents.find((agent) => agent.key === key)?.display_name ?? key;
  const title = () => {
    const plan = preview();
    return plan ? plan.rollback ? t().restore : t().preview : t().issues;
  };
  return <Modal theme={props.theme} label={title()} open width={650} onClose={props.onClose}>
    <If when={preview()}>{(plan) => <View style={{ gap: 5 }}>
      <Label theme={props.theme} weight="600">{plan().skill_name}</Label>
      <Label theme={props.theme} muted>{t().snapshot}</Label>
    </View>}</If>
    <Scroll style={{ height: 280, minHeight: 0 }} contentStyle={{ gap: 9, paddingRight: 8 }}>
      <If when={preview()}>{(plan) => <>
        <Label theme={props.theme} heading>{t().files}</Label>
        <Each of={plan().files} key={(file) => file.path}>
          {(file) => <View style={{ flexDirection: "row", gap: 10, alignItems: "flex-start" }}>
            <View style={{ width: 80, flexShrink: 0 }}>
              <Label theme={props.theme} muted>{t()[file().kind]}</Label>
            </View>
            <View style={{ flex: 1, minWidth: 0 }}>
              <Label theme={props.theme} mono>{file().path}</Label>
            </View>
          </View>}
        </Each>
        <Label theme={props.theme} heading>{t().deployments}</Label>
        {deployments().length === 0 ? <Label theme={props.theme} muted>{t().empty}</Label> : null}
      </>}</If>
      <Each of={deployments()} key={(deployment) => deployment.path}>
        {(deployment) => <View style={{ gap: 4, minWidth: 0 }}>
          <Label theme={props.theme} weight="500">{deployment().agents.map(agentName).join(", ")}</Label>
          <Label theme={props.theme} mono>{deployment().path}</Label>
          <Label theme={props.theme} muted>{t()[deployment().kind]}</Label>
        </View>}
      </Each>
    </Scroll>
    <View style={{ flexDirection: "row", justifyContent: "flex-end", gap: 8 }}>
      <Action theme={props.theme} label={preview() ? t().cancel : t().close}
        disabled={props.applying} onPress={props.onClose} />
      <If when={preview()}>{(plan) => <Action theme={props.theme} variant="primary"
        label={plan().rollback ? t().restore : t().update} pending={props.applying}
        onPress={() => props.onConfirm(plan())} />}</If>
    </View>
  </Modal>;
}
