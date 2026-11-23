import type { Locale } from "./strings";

const copy = {
  deleteSkill: ["Permanently Delete Skill", "永久删除技能", "スキルを完全に削除"],
  confirmDelete: ["Confirm Permanent Skill Deletion", "确认永久删除技能", "スキルの完全削除を確認"],
  deleteDescription: [
    "This permanently deletes the skill files from your UnoSkills library. This action cannot be undone.",
    "此操作会永久删除 UnoSkills 技能库中的技能文件，无法撤销。",
    "UnoSkills ライブラリからスキルのファイルを完全に削除します。この操作は元に戻せません。",
  ],
  deleteDeployments: [
    "This also uninstalls the skill from the locations below by removing its managed copies or symlinks. Shared paths still in use will be kept.",
    "此技能也会从下列位置卸载，对应的受管副本或软链接会被移除。仍由其他部署使用的共享路径会保留。",
    "以下の配備先からもスキルをアンインストールし、管理対象のコピーやシンボリックリンクを削除します。他の配備が使用する共有先は保持されます。",
  ],
  noDeployments: ["No deployment entries", "没有部署记录", "配備の記録はありません"],
  removeDeployment: ["Remove Project Deployment", "取消项目部署", "プロジェクトの配備を解除"],
  confirmRemove: ["Confirm Remove Project Deployment", "确认取消项目部署", "プロジェクトの配備解除を確定"],
  removeDescription: [
    "Remove this Agent’s deployment from the selected project scope. Shared targets stay in place for other deployments.",
    "取消该 Agent 在所选项目范围内的部署。仍有其他部署使用的共享目录会保留。",
    "選択したプロジェクト範囲で、このエージェントの配備を解除します。他の配備が使用する共有先は保持されます。",
  ],
  root: ["Project Root", "项目根目录", "プロジェクトルート"],
  reviewConflict: ["Review Project Conflict", "处理冲突", "プロジェクトの競合を確認"],
  modified: ["UnoSkills manages this copy. Its files have changed.", "这是 UnoSkills 管理的副本，文件已在项目中修改。", "UnoSkills が管理するコピーです。プロジェクト内でファイルが変更されています。"],
  unmanaged: ["The target belongs to a local file or folder.", "目标位置是项目中的未托管文件或目录。", "配備先にはプロジェクト内の未管理のファイルまたはフォルダーがあります。"],
  unsafe: ["The target path or deployment record could not be verified.", "无法验证目标路径或部署记录。", "配備先のパスまたは配備の記録を確認できませんでした。"],
  unreadable: ["The comparison is unavailable. Files may be unreadable or unsupported.", "无法比较文件，可能存在无法读取或不受支持的项目。", "ファイルを比較できません。読み取れない項目や未対応の項目が含まれている可能性があります。"],
  stale: ["The reviewed state changed. Close and refresh to check it again.", "已核对的状态发生变化。请关闭并刷新后重新核对。", "確認した状態が変わりました。閉じて更新した後に再確認してください。"],
  blocked: ["The target changed or could not be verified for removal.", "目标内容或路径已变化，无法安全移除。", "配備先が変更されたか、安全に削除できることを確認できませんでした。"],
  failed: ["The operation could not be completed. See the diagnostic details.", "操作未能完成。请查看诊断详情。", "操作を完了できませんでした。診断の詳細を確認してください。"],
  backUp: [
    "Check the affected path. Back up and move the entire edited copy out of that path, or restore the original files, then try again.",
    "请检查受影响的路径。先备份并将整个修改过的副本移出该路径，或恢复原始文件，然后重试。",
    "対象のパスを確認してください。変更したコピー全体をバックアップして別の場所に移すか、元のファイルを復元してから再試行してください。",
  ],
  recovery: [
    "Automatic restoration was incomplete. Recover the original files from these locations before retrying.",
    "自动恢复未完成。请先从以下位置找回原始文件，再重试。",
    "自動復元は完了していません。再試行する前に、以下の場所から元のファイルを復元してください。",
  ],
  details: ["Diagnostic Details", "诊断详情", "診断の詳細"],
  closeRefresh: ["Close and Refresh", "关闭并刷新", "閉じて更新"],
  keepFiles: ["Keep Files", "保留文件", "ファイルを保持"],
  openFolder: ["Open Folder", "打开目录", "フォルダーを開く"],
  fileChanges: ["File Changes · Project → Library", "文件变更预览 · 项目副本 → 技能库", "ファイルの変更 · プロジェクト → ライブラリ"],
  pinnedFileChanges: ["File Changes · Project → Pinned Version", "文件变更预览 · 项目副本 → 锁定版本", "ファイルの変更 · プロジェクト → 固定バージョン"],
  noFileChanges: ["No file differences are visible in this comparison.", "本次比较未发现文件差异。", "この比較ではファイルの差分はありません。"],
  noTextDiff: ["Text comparison unavailable for this item.", "此项无法显示文本差异。", "この項目のテキスト差分は表示できません。"],
  added: ["Added", "新增", "追加"],
  removed: ["Removed", "删除", "削除"],
  modifiedFile: ["Modified", "修改", "変更"],
  protected: ["Replacement is unavailable for this target. Save your changes separately before resolving the conflict.", "此目标无法在此处覆盖。请先将修改另存，再处理冲突。", "この配備先には上書きできません。競合の解消前に、変更を別の場所へ保存してください。"],
  pinnedConflict: ["The project pin protects this version from being overwritten.", "项目版本已锁定，无法覆盖。", "このプロジェクトのバージョンは固定されているため、上書きできません。"],
  reviewOverwrite: ["Review Overwrite", "查看覆盖确认", "上書きの確認へ"],
  overwrite: ["Overwrite Project Copy", "覆盖项目副本", "プロジェクトのコピーを上書き"],
  overwriteDescription: [
    "This permanently replaces the reviewed project files with the library version.",
    "这会将已核对的项目文件替换为技能库版本，本地修改将永久丢失。",
    "確認したプロジェクトのファイルをライブラリ版で置き換えます。ローカルの変更は失われます。",
  ],
  overwritten: ["Project copy replaced", "已覆盖项目副本", "プロジェクトのコピーを置き換えました"],
  retainedPin: ["Pin retained", "项目锁定已保留", "固定バージョンを保持"],
  pinLabel: ["Pinned", "已锁定", "固定済み"],
  viewPin: ["View Pin", "查看锁定版本", "固定バージョンを表示"],
  managePin: ["Manage Project Pin", "管理项目锁定", "プロジェクトの固定を管理"],
  conflict: ["Conflict", "冲突", "競合"],
  managed: ["Deployed", "已部署", "配備済み"],
  local: ["Local", "项目本地", "ローカル"],
} as const;

export function managementStrings(locale: Locale): Record<keyof typeof copy, string> {
  const index = locale === "zh-CN" ? 1 : locale === "ja" ? 2 : 0;
  return Object.fromEntries(Object.entries(copy).map(([key, values]) => [key, values[index]])) as Record<keyof typeof copy, string>;
}

export function projectScopeLabel(locale: Locale, project: string, scope: string): string {
  return `${project} · ${scope || managementStrings(locale).root}`;
}
