import { List, Scroll, View, signal } from "nsh";
import { Action, CheckControl, Choice, Label, Modal } from "../components";
import type { Locale, Theme } from "../shared";
import type { AdoptFailure, AdoptionMergeChoice, AdoptionMergePreview, AdoptionMergeRequest, AdoptionMergeResult, AdoptionMergeTarget } from "./types";

const messages = {
  review: ["Review and merge", "合并并纳管", "確認して統合"],
  confirm: ["Merge and manage", "确认合并", "統合して管理"],
  identical: ["Merge identical copies", "合并相同的副本", "同一のコピーを統合"],
  existing: ["Existing deployment", "现有部署", "現在の配備"],
  missingFolder: ["(folder missing)", "（目录已不存在）", "（フォルダーなし）"],
  detected: ["Detected folder", "本次发现", "検出したフォルダー"],
  library: ["Library location", "技能库位置", "ライブラリの場所"],
  target: ["Path to keep in use", "保留使用的路径", "使い続けるパス"],
  chooseTarget: ["Choose the deployment", "选择保留的位置", "残す配備先を選択"],
  remove: ["Path to remove from the Agent's scan", "从 Agent 扫描范围移除", "エージェントの検出対象から除外"],
  source: ["Version to keep", "以哪份内容为准", "残すバージョン"],
  libraryVersion: ["Keep library version", "保留技能库版本", "ライブラリ版を残す"],
  localVersion: ["Use detected local version", "采用本机版本", "検出したローカル版を使う"],
  fileDiff: ["Detected folder compared with the library", "本次发现与技能库的差异", "検出したフォルダーとライブラリの差分"],
  existingDiff: ["Existing deployment compared with the library", "现有部署与技能库的差异", "現在の配備とライブラリの差分"],
  impact: ["Other deployments affected by changing the library version", "采用本机版本会影响这些部署", "ライブラリ版の変更が影響する他の配備"],
  backup: ["Different versions will be saved in a recovery folder.", "不同的内容会存到恢复目录。", "内容が異なるバージョンは復元用フォルダーに保存されます。"],
  copy: ["Allow a managed copy", "允许使用受管副本", "管理対象のコピーを許可"],
  copyDetail: ["If a directory link is unavailable, keep a copy in the Agent’s folder.", "无法创建目录链接时，在 Agent 目录保留一份副本。", "ディレクトリリンクを作成できない場合は、エージェントのフォルダーにコピーを残します。"],
  shared: ["Also used by", "此路径也被这些 Agent 使用", "このパスを併用するエージェント"],
  cancel: ["Cancel", "取消", "キャンセル"],
  close: ["Close", "关闭", "閉じる"],
  done: ["Local skills merged.", "已合并并纳管。", "ローカルスキルを統合しました。"],
  results: ["View merge results", "查看合并结果", "統合結果を表示"],
  recovery: ["Show recovery folder", "查看恢复目录", "復元用フォルダーを表示"],
  recoveryNeeded: ["Some originals need to be recovered from the paths below.", "以下原文件需要从恢复路径取回。", "次のパスから元のファイルを復元してください。"],
  linkResult: ["Directory link · One file set in the library", "目录链接 · 文件存放在中央技能库", "ディレクトリリンク · ライブラリで一元管理"],
  copyResult: ["Managed copy · One file set in the library and one at this path", "受管副本 · 中央技能库和此路径各有一份", "管理対象のコピー · ライブラリとこのパスに各 1 つ"],
  issues: ["Deployments needing attention", "需要检查的部署", "確認が必要な配備"],
  added: ["Added", "新增", "追加"],
  modified: ["Changed", "有改动", "変更"],
  removed: ["Removed", "移除", "削除"],
  linked: ["Follows library", "跟随技能库", "ライブラリに追従"],
  current: ["Already current", "已经一致", "変更済み"],
  refresh_copy: ["Copy will refresh", "更新受管副本", "コピーを更新"],
  missing: ["Folder is missing", "目录不存在", "フォルダーなし"],
  conflict: ["Local changes kept", "保留本地改动", "ローカル変更を保持"],
  refresh_failed: ["Could not refresh copy", "更新副本失败", "コピーの更新に失敗"],
  duplicate: ["This Agent has another deployment of this skill. Local files were kept.", "这个 Agent 在另一位置有部署记录，本机文件已保留。", "このエージェントには別の配備先があります。ローカルファイルは保持されました。"],
  libraryConflict: ["The library already has a skill with this folder name. Review the two versions before merging.", "技能库已有同名技能。请先查看两份内容的差异，再确认合并。", "ライブラリに同名のスキルがあります。統合前に両方のバージョンを確認してください。"],
  protected: ["A shared or protected path prevents this merge. Local files were kept.", "这里涉及共享或受保护的目录，文件已保留。", "共有または保護されたパスのため統合できません。ローカルファイルは保持されました。"],
  stale: ["The files or deployment changed. Review the current state before merging.", "文件或部署已变化，请重新查看后再合并。", "ファイルまたは配備が変更されました。確認し直してください。"],
  unavailable: ["A link could not be created. Allow a managed copy to proceed.", "无法创建链接。允许受管副本后可继续。", "リンクを作れませんでした。続けるには管理対象のコピーを許可してください。"],
} as const;

export function mergeText(locale: Locale, key: keyof typeof messages) {
  return messages[key][locale === "zh-CN" ? 1 : locale === "ja" ? 2 : 0];
}

export function adoptionMergeError(locale: Locale, failure: AdoptFailure): string {
  if (failure.recovery_paths?.length) return mergeText(locale, "recoveryNeeded");
  if (failure.error.startsWith("ADOPTION_LINK_UNAVAILABLE:")) return mergeText(locale, "unavailable");
  if (failure.error.startsWith("STALE_ADOPTION_MERGE:")) return mergeText(locale, "stale");
  if (failure.merge_issue || failure.error.startsWith("ADOPTION_MERGE_PROTECTED:") ||
      (failure.merge && failure.merge.targets.length === 0)) return mergeText(locale, "protected");
  if (failure.merge && failure.error.startsWith("TARGET_CONFLICT:")) return mergeText(locale, "duplicate");
  if (failure.merge && failure.error.startsWith("SKILL_SLUG_CONFLICT:")) return mergeText(locale, "libraryConflict");
  return failure.error;
}

export function AdoptionMergeDialog(props: {
  theme: Theme;
  locale: Locale;
  reviews: AdoptionMergePreview[];
  busy: boolean;
  onClose: () => void;
  onConfirm: (requests: AdoptionMergeRequest[]) => void;
}) {
  const t = (key: keyof typeof messages) => mergeText(props.locale, key);
  const target = signal<AdoptionMergeTarget>(props.reviews[0].targets[0]);
  const content = signal<AdoptionMergeChoice>("library");
  const allowCopy = signal(false);
  const single = () => props.reviews.length === 1;
  const review = () => props.reviews[0];
  const chosenPath = (plan: AdoptionMergePreview) =>
    ((single() ? target() : plan.targets[0]) === "existing" ? plan.existing_path : null) ?? plan.candidate.path;
  const retiredPath = (plan: AdoptionMergePreview) =>
    chosenPath(plan) === plan.existing_path ? plan.candidate.path
      : plan.existing_present && !plan.keep_existing_for_peers ? plan.existing_path : null;
  const submit = () => props.onConfirm(props.reviews.map((plan) => ({
    candidate: plan.candidate,
    token: plan.token,
    target: single() ? target() : plan.targets[0],
    content: single() ? content() : "library",
    allow_copy: allowCopy(),
  })));
  const paths = (plan: () => AdoptionMergePreview, outcome: boolean) => <View style={{ gap: 7, minWidth: 0 }}>
    <Label theme={props.theme} weight="600">{plan().candidate.name} · {plan().candidate.agent_name}</Label>
    <PathLine theme={props.theme} title={t("library")} path={plan().library_path} />
    <If when={plan().existing_path}>{(path) =>
      <PathLine theme={props.theme} title={t("existing")} path={path()}
        note={plan().existing_present ? undefined : t("missingFolder")} />
    }</If>
    <If when={plan().shared_existing.length > 0}>
      <Label theme={props.theme} muted>{t("shared")} · {plan().shared_existing.join(", ")}</Label>
    </If>
    <PathLine theme={props.theme} title={t(plan().existing_path ? "detected" : "target")} path={plan().candidate.path} />
    <If when={plan().shared_detected.length > 0}>
      <Label theme={props.theme} muted>{t("shared")} · {plan().shared_detected.join(", ")}</Label>
    </If>
    <If when={outcome && plan().existing_path !== null}>
      <PathLine theme={props.theme} title={t("target")} path={chosenPath(plan())} />
      <If when={retiredPath(plan())}>{(path) =>
        <PathLine theme={props.theme} title={t("remove")} path={path()} />
      }</If>
    </If>
  </View>;

  return <Modal theme={props.theme} label={single() ? t("review") : t("identical")} open width={680}
    closeOnBackdropPress={!props.busy} closeOnEscape={!props.busy} onClose={props.onClose}>
    <If when={single()} else={
      <List each={props.reviews} keyExtractor={(plan) => plan.candidate.path} estimatedItemSize={190}
        style={{ height: 342, minHeight: 0, flexShrink: 1 }}>
        {(plan) => <View style={{ paddingVertical: 10, borderBottomWidth: 1, borderColor: props.theme.border }}>
          {paths(plan, true)}
        </View>}
      </List>
    }>
      <Scroll style={{ height: 342, minHeight: 0, flexShrink: 1 }} contentStyle={{ gap: 13, paddingRight: 8 }}>
        {paths(review, false)}
        <If when={review().targets.length > 1}>
          <Choice theme={props.theme} title={t("chooseTarget")} value={target()}
            disabled={props.busy}
            items={review().targets.map((value) => ({ value, title: t(value) }))}
            onChange={(value) => target.set(value as AdoptionMergeTarget)} />
        </If>
        <If when={review().existing_path !== null}>
          <PathLine theme={props.theme} title={t("target")} path={chosenPath(review())} />
        </If>
        <If when={retiredPath(review())}>{(path) =>
          <PathLine theme={props.theme} title={t("remove")} path={path()} />
        }</If>
        <If when={!review().identical}>
          <Choice theme={props.theme} title={t("source")} value={content()}
            disabled={props.busy}
            items={[{ value: "library", title: t("libraryVersion") }, { value: "local", title: t("localVersion") }]}
            onChange={(value) => content.set(value as AdoptionMergeChoice)} />
          <Label theme={props.theme} muted>{t("backup")}</Label>
          <If when={review().library_changes.length > 0}>
            <Label theme={props.theme} weight="600">{t("fileDiff")}</Label>
            <Each of={review().library_changes} key={(file) => file.path}>{(file) =>
              <PathLine theme={props.theme} title={t(file().kind)} path={file().path} />
            }</Each>
            <If when={review().skill_diff}>{(diff) => <Label theme={props.theme} mono>{diff()}</Label>}</If>
          </If>
          <If when={review().existing_changes.length > 0}>
            <Label theme={props.theme} weight="600">{t("existingDiff")}</Label>
            <Each of={review().existing_changes} key={(file) => file.path}>{(file) =>
              <PathLine theme={props.theme} title={t(file().kind)} path={file().path} />
            }</Each>
          </If>
          <If when={content() === "local" && review().local_version_impacts.length > 0}>
            <Label theme={props.theme} weight="600">{t("impact")}</Label>
            <Each of={review().local_version_impacts} key={(item) => item.path}>{(item) =>
              <PathLine theme={props.theme} title={t(item().kind)} path={item().path} />
            }</Each>
          </If>
        </If>
      </Scroll>
    </If>
    <View style={{ gap: 2, paddingTop: 8, borderTopWidth: 1, borderColor: props.theme.border }}>
      <CheckControl theme={props.theme} checked={allowCopy()} disabled={props.busy}
        style={{ flexDirection: "row", alignItems: "center", gap: 8, minHeight: 32, minWidth: 0,
          paddingVertical: 4, paddingHorizontal: 8, borderRadius: props.theme.control.radius }}
        accessibilityLabel={t("copy")} onChange={(checked) => allowCopy.set(checked)}>
        <View style={{ flex: 1, minWidth: 0 }}>
          <Label theme={props.theme} color={props.theme.popup.foreground}>{t("copy")}</Label>
        </View>
      </CheckControl>
      <View style={{ paddingLeft: 32 }}>
        <Label theme={props.theme} muted>{t("copyDetail")}</Label>
      </View>
    </View>
    <View style={{ flexDirection: "row", justifyContent: "flex-end", gap: 8 }}>
      <Action theme={props.theme} label={t("cancel")} disabled={props.busy} onPress={props.onClose} />
      <Action theme={props.theme} label={t("confirm")} variant="primary" pending={props.busy} onPress={submit} />
    </View>
  </Modal>;
}

function PathLine(props: { theme: Theme; title: string; path: string; note?: string }) {
  return <View style={{ gap: 2, minWidth: 0 }}>
    <Label theme={props.theme} muted>{props.title}{props.note ? ` ${props.note}` : ""}</Label>
    <Label theme={props.theme} mono>{props.path}</Label>
  </View>;
}

export function AdoptionMergeResults(props: {
  theme: Theme;
  locale: Locale;
  results: AdoptionMergeResult[];
  onClose: () => void;
  onReveal: (path: string) => void;
}) {
  const t = (key: keyof typeof messages) => mergeText(props.locale, key);
  return <Modal theme={props.theme} label={t("results")} open width={650} onClose={props.onClose}>
    <List each={props.results} keyExtractor={(item) => item.path} estimatedItemSize={78}
      style={{ height: 260, minHeight: 0, flexShrink: 1 }}>
      {(item) => <View style={{ gap: 6, paddingVertical: 8, alignItems: "flex-start" }}>
        <PathLine theme={props.theme} title={t("target")} path={item().deployment.target_path} />
        <Label theme={props.theme} muted>{t(item().deployment.mode === "copy" ? "copyResult" : "linkResult")}</Label>
        <If when={item().backup}>{(path) =>
          <View style={{ gap: 4, alignItems: "flex-start" }}>
            <Label theme={props.theme} mono>{path()}</Label>
            <Action theme={props.theme} label={t("recovery")} onPress={() => props.onReveal(path())} />
          </View>
        }</If>
        <If when={item().deployment_issues.length > 0}>
          <Label theme={props.theme} weight="600">{t("issues")}</Label>
          <Each of={item().deployment_issues} key={(issue) => issue.path}>{(issue) =>
            <PathLine theme={props.theme} title={t(issue().kind)} path={issue().path} />
          }</Each>
        </If>
      </View>}
    </List>
    <View style={{ flexDirection: "row", justifyContent: "flex-end" }}>
      <Action theme={props.theme} label={t("close")} onPress={props.onClose} />
    </View>
  </Modal>;
}
