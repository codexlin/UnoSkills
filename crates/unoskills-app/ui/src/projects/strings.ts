import type { Locale } from "../shared";
import type { DeployResult, DeployStatus, RecommendationPurpose } from "./host";

const rows = {
  projects: ["Projects", "项目", "プロジェクト"],
  attach: ["Add Project", "添加项目", "プロジェクト追加"],
  chooseFolder: ["Choose a project folder", "选择项目文件夹", "プロジェクトフォルダーを選択"],
  attached: ["Project attached", "已添加项目", "プロジェクトを追加しました"],
  detach: ["Detach project", "移除项目", "プロジェクトの登録を解除"],
  detachDescription: ["Managed deployments will be removed from this project.", "将移除此项目中由 UnoSkills 管理的部署。", "このプロジェクトの管理対象の配備が削除されます。"],
  detached: ["Project detached", "已移除项目", "プロジェクトの登録を解除しました"],
  noProjects: ["Attach a folder to scan its technologies and skills.", "添加文件夹以扫描其中的技术和技能。", "フォルダーを追加して技術とスキルをスキャンできます。"],
  selectProject: ["Select a project", "选择项目", "プロジェクトを選択"],
  scope: ["Scan Scope", "识别范围", "スキャン範囲"],
  root: ["Project Root", "项目根目录", "プロジェクトルート"],
  agent: ["Agent", "Agent", "エージェント"],
  projectUnavailable: ["Choose an attached project.", "请选择已添加的项目。", "登録済みのプロジェクトを選択してください。"],
  scopeUnavailable: ["Choose a scope in this project.", "请选择此项目中的作用域。", "このプロジェクトで適用範囲を選択してください。"],
  agentUnavailable: ["Choose an enabled agent.", "请选择已启用的 Agent。", "有効なエージェントを選択してください。"],
  noAgents: ["No enabled agents were detected.", "未检测到已启用的 Agent。", "有効なエージェントが検出されませんでした。"],
  rescan: ["Rescan", "重新扫描", "再スキャン"],
  technologies: ["Detected Stack", "检测到的技术栈", "検出した技術"],
  noTechnologies: ["No supported project configuration was found.", "没有找到支持的项目配置。", "対応するプロジェクト設定が見つかりません。"],
  newTechnology: ["New", "新增", "新規"],
  recommendations: ["Recommended Skills", "推荐技能", "おすすめのスキル"],
  noRecommendations: ["No curated skills match this stack yet.", "暂无符合此技术栈的精选技能。", "この技術構成に合うおすすめのスキルはありません。"],
  saveRecommendations: ["Save as Stack", "保存为组合", "スタックとして保存"],
  react_performance: ["React performance", "React 性能", "React のパフォーマンス"],
  accessibility: ["Accessibility", "无障碍", "アクセシビリティ"],
  testing: ["Testing", "测试", "テスト"],
  debugging: ["Debugging", "调试", "デバッグ"],
  inventory: ["Current Skills", "当前技能", "現在のスキル"],
  emptyInventory: ["No skills found in the project or global directories.", "项目及全局技能目录中暂无技能。", "プロジェクトとグローバルのディレクトリにスキルがありません。"],
  project_deployment: ["Project deployment", "项目部署", "プロジェクト配備"],
  project_changed: ["Project files changed", "项目文件有差异", "プロジェクト内に差分あり"],
  project_local: ["Added in project", "项目手动添加", "プロジェクトで手動追加"],
  global: ["Global", "全局", "グローバル"],
  openLibrary: ["Library", "技能库", "ライブラリ"],
  openProject: ["Open project", "打开项目", "プロジェクトを開く"],
  stacks: ["Skill Stacks", "技能组合", "スキルスタック"],
  Featured: ["Featured", "社区推荐", "おすすめ"],
  Community: ["Community", "社区", "コミュニティ"],
  Local: ["My Stacks", "我的组合", "マイスタック"],
  all: ["All stacks", "全部组合", "すべてのスタック"],
  catalogSource: ["Source", "来源", "ソース"],
  filterTag: ["Filter stacks by tag", "按标签筛选组合", "タグでスタックを絞り込む"],
  allTags: ["All tags", "全部标签", "すべてのタグ"],
  searchStacks: ["Search stacks by name, author, or tag...", "搜索组合名称、作者或标签...", "名前・作成者・タグでスタックを検索..."],
  selectStack: ["Select a Stack", "选择一个组合", "スタックを選択"],
  noStacks: ["No matching stacks.", "没有匹配的组合。", "該当するスタックがありません。"],
  verified: ["Verified", "官方精选", "公式厳選"],
  includedSkills: ["Included Skills", "包含的技能", "含まれるスキル"],
  sourceConflict: ["Same name, different source", "同名技能来源不同", "同名の別ソース"],
  inLibrary: ["Already in Library", "已在库中", "導入済み"],
  missing: ["Missing", "待安装", "未インストール"],
  install: ["Install", "安装", "インストール"],
  installMissing: ["Install all skills", "安装全部技能", "すべてのスキルをインストール"],
  installed: ["Installed", "已安装", "インストール済み"],
  skipped: ["Already in library", "已存在于技能库", "ライブラリに追加済み"],
  failures: ["Failed", "失败", "失敗"],
  installPreview: ["Install and preview deployment", "安装并预览部署", "インストールして配備をプレビュー"],
  saveStack: ["Save to My Stacks", "保存到我的组合", "マイスタックに保存"],
  saved: ["Stack saved", "已保存组合", "スタックを保存しました"],
  create: ["New stack", "新建组合", "スタックを作成"],
  edit: ["Edit stack", "编辑组合", "スタックを編集"],
  name: ["Stack Name", "组合名称", "スタック名"],
  author: ["Author (GitHub handle)", "作者（GitHub 用户名）", "作成者（GitHub ユーザー名）"],
  description: ["Description / Workflow Note", "组合说明 / 搭配场景", "説明 / 活用シナリオ"],
  tags: ["Tags (comma-separated)", "标签（用逗号隔开）", "タグ（カンマ区切り）"],
  selectSkills: ["Select Skills from Library", "选择技能", "ライブラリからスキルを選択"],
  searchSkills: ["Search skills", "搜索技能", "スキルを検索"],
  noSkills: ["No matching library skills.", "技能库中没有匹配的技能。", "ライブラリに該当するスキルがありません。"],
  keepMembers: ["Keep existing stack members", "保留现有组合成员", "既存のスキルを残す"],
  delete: ["Delete stack", "删除组合", "スタックを削除"],
  deleted: ["Stack deleted", "已删除组合", "スタックを削除しました"],
  import: ["Import TOML", "导入 TOML", "TOML をインポート"],
  openFile: ["Open TOML file", "打开 TOML 文件", "TOML ファイルを開く"],
  pasteToml: ["Paste a .unoskills-stack.toml manifest here...", "在此粘贴 .unoskills-stack.toml 内容...", ".unoskills-stack.toml の内容をここに貼り付け..."],
  previewImport: ["Preview import", "预览导入", "インポートをプレビュー"],
  replaceImport: ["This import will replace the local stack with the same ID.", "此导入将替换相同 ID 的本地组合。", "同じ ID のローカルスタックが置き換わります。"],
  imported: ["Stack imported", "已导入组合", "スタックをインポートしました"],
  copyToml: ["Copy Share TOML", "复制分享 TOML", "共有用 TOML をコピー"],
  export: ["Export TOML", "导出 TOML", "TOML を書き出す"],
  copied: ["Copied to clipboard", "已复制到剪贴板", "クリップボードにコピーしました"],
  exported: ["Stack exported", "已导出组合", "スタックを書き出しました"],
  share: ["Share to Gist", "分享到 Gist", "Gist で共有"],
  shareDescription: ["Upload this stack manifest to a secret Gist using the GitHub token in Settings?", "使用设置中的 GitHub 令牌将此组合清单上传到私密 Gist？", "設定の GitHub トークンを使って、このスタックを非公開 Gist にアップロードしますか？"],
  deploy: ["Deploy to Project", "部署到项目", "プロジェクトに配備"],
  previewDeploy: ["Deployment preview", "部署预览", "配備プレビュー"],
  confirmDeploy: ["Deploy Skills", "部署技能", "スキルを配備"],
  deployed: ["Deployed", "已部署", "配備済み"],
  alreadyDeployed: ["Already deployed", "已完成部署", "すでに配備済み"],
  pending: ["Ready to deploy", "可部署", "配備可能"],
  conflicts: ["Conflicting targets", "目标冲突", "配備先の競合"],
  unavailable: ["Unavailable", "不可用", "利用不可"],
  includeGlobal: ["Also deploy globally available skills", "同时部署全局可用的技能", "グローバルで利用可能なスキルも配備"],
  conflictHelp: ["Existing project skills and conflicting files will be kept.", "项目已有技能和冲突文件会保留。", "プロジェクトに既存のスキルや競合ファイルは保持されます。"],
  recommendedAgents: ["Recommended agents", "推荐 Agent", "推奨エージェント"],
  loading: ["Loading…", "正在加载…", "読み込み中…"],
  working: ["Working…", "正在处理…", "処理中…"],
  refresh: ["Refresh", "刷新", "更新"],
  cancel: ["Cancel", "取消", "キャンセル"],
  back: ["Back", "返回", "戻る"],
  defaultFolder: ["Default Deployment Folder", "默认部署目录", "既定の配備先"],
  selectAgent: ["Select an agent", "选择 Agent", "エージェントを選択"],
  installFirst: ["Install first", "需先安装", "要インストール"],
  stackActions: ["Stack actions", "组合操作", "スタックの操作"],
  createFirst: ["Create Your First Stack", "创建第一个组合", "最初のスタックを作成"],
  noLocalStacks: ["No local stacks yet", "还没有保存的组合", "保存済みスタックがありません"],
  noFeaturedStacks: ["No featured stacks", "暂无推荐组合", "おすすめスタックがありません"],
  emptyStacksDescription: ["Bundle your favorite skills into a reusable TOML stack.", "把你常用的几个技能打包成一个声明式 TOML 组合，随时复用或分享给朋友。", "よく使うスキルを宣言的な TOML スタックにまとめて共有できます。"],
  selectStackDescription: ["Preview included skills, pairing notes, and install in one click.", "查看包含的技能、作者搭配说明，并支持一键增量安装。", "含まれるスキルと組み合わせメモを確認し、ワンクリックで導入できます。"],
  localBadge: ["Local", "本地组合", "ローカル"],
  close: ["Close", "关闭", "閉じる"],
  refreshCommunity: ["Refresh community stacks", "刷新社区组合", "コミュニティスタックを更新"],
  deploymentFolder: ["Deployment Folder", "部署目录", "配備先フォルダー"],
  checking: ["Checking project skills…", "正在检查项目里的技能…", "プロジェクトのスキルを確認中…"],
  refreshDeployment: ["Refresh deployment status", "刷新部署状态", "配備状態を更新"],
  targetConflict: ["The target is occupied or has changed. Its files were kept.", "目标目录已被占用或内容已变更，原文件已保留。", "配備先が使用中か、内容が変更されています。既存のファイルを保持しました。"],
  createDescription: ["Bundle your favorite skills into a declarative TOML recipe to share via GitHub Gist or PR.", "从技能库勾选搭配，生成轻量声明式 TOML 清单，可通过 GitHub Gist 或 PR 分享。", "ライブラリからスキルを選んで TOML レシピを生成し、GitHub Gist や PR で共有できます。"],
  importDescription: ["Paste any `.unoskills-stack.toml` shared by a teammate or community author.", "粘贴团队成员或社区作者分享的 `.unoskills-stack.toml` 清单即可导入。", "共有された `.unoskills-stack.toml` を貼り付けて取り込みます。"],
  namePlaceholder: ["e.g. Rust + GPUI Native Stack", "例如：Rust + GPUI 原生开发全家桶", "例：Rust + GPUI ネイティブ開発セット"],
  authorPlaceholder: ["Your GitHub username (optional)", "你的 GitHub 用户名（选填）", "GitHub ユーザー名（任意）"],
  descriptionPlaceholder: ["When and how should agents use this stack together?", "说明这套技能适合什么场景、彼此怎么配合...", "どのような場面で各スキルを組み合わせるか..."],
  noLibrarySkills: ["Install skills or add local skills to the library before creating a stack.", "先安装技能或将本机技能纳入技能库，再创建组合。", "先にスキルを導入または取り込んでからスタックを作成してください。"],
  projectActions: ["Project actions", "项目操作", "プロジェクトの操作"],
  switchProject: ["Switch project", "切换项目", "プロジェクトを切り替え"],
} as const;

type Copy = { [K in keyof typeof rows]: string };

function dictionary(index: 0 | 1 | 2): Copy {
  return Object.fromEntries(Object.entries(rows).map(([key, values]) => [key, values[index]])) as Copy;
}

const dictionaries = { en: dictionary(0), "zh-CN": dictionary(1), ja: dictionary(2) };

export function strings(locale: Locale): Copy {
  return dictionaries[locale];
}

export function stackSize(locale: Locale, count: number): string {
  return { en: `${count} ${count === 1 ? "skill" : "skills"}`, "zh-CN": `${count} 个技能`, ja: `${count} 件のスキル` }[locale];
}

export function installCounts(locale: Locale, missing: number, existing: number): string {
  if (missing === 0) return {
    en: `${existing} already in the library`, "zh-CN": `已在库中 ${existing} 项`, ja: `導入済み ${existing} 件`,
  }[locale];
  return {
    en: `${missing} to install, ${existing} already in the library`,
    "zh-CN": `安装 ${missing} 项，已在库中 ${existing} 项`,
    ja: `${missing} 件をインストール、導入済み ${existing} 件`,
  }[locale];
}

export function projectSelectionCount(locale: Locale, count: number): string {
  return { en: `${count} selected`, "zh-CN": `已选择 ${count} 项技能`, ja: `${count} 件のスキルを選択済み` }[locale];
}

export function projectRecommendationReason(locale: Locale, technology: string, purpose: RecommendationPurpose): string {
  return {
    en: {
      react_performance: `Review rendering and loading performance in ${technology}.`,
      accessibility: `Review interface and accessibility in ${technology}.`,
      testing: `Write regression tests for ${technology}.`,
      debugging: `Diagnose issues in your ${technology} project.`,
    },
    "zh-CN": {
      react_performance: `检测到 ${technology}，适合检查渲染与加载性能。`,
      accessibility: `检测到 ${technology}，适合检查界面和可访问性。`,
      testing: `检测到 ${technology}，可用来编写回归测试。`,
      debugging: `检测到 ${technology}，可用来排查项目故障。`,
    },
    ja: {
      react_performance: `${technology} の描画と読み込み性能を確認できます。`,
      accessibility: `${technology} の UI とアクセシビリティを確認できます。`,
      testing: `${technology} の回帰テストの作成に役立ちます。`,
      debugging: `${technology} プロジェクトの問題調査に役立ちます。`,
    },
  }[locale][purpose];
}

export function deploymentProgress(locale: Locale, status: DeployStatus, total: number, pending: number): string {
  const global = status.global_available;
  const available = status.deployed + status.project_local + global;
  return {
    en: `${available}/${total} available · ${global} global · ${pending} to deploy`,
    "zh-CN": `可用 ${available}/${total} · 全局可用 ${global} · 待部署 ${pending}`,
    ja: `利用可能 ${available}/${total} · グローバル ${global} · 配備待ち ${pending}`,
  }[locale];
}

export function deploymentUnavailable(locale: Locale, count: number): string {
  return {
    en: `${count} skills are missing or have a different source. Check the stack details.`,
    "zh-CN": `${count} 项未入库或来源不符。可在组合详情检查。`,
    ja: `${count} 件はライブラリにないか、ソースが異なります。スタックの詳細を確認してください。`,
  }[locale];
}

export function deploymentConflicts(locale: Locale, count: number): string {
  return {
    en: `${count} deployment targets are occupied or have changed. Resolve them, then refresh.`,
    "zh-CN": `${count} 项的目标目录已被占用或内容已变更。处理后可刷新状态。`,
    ja: `${count} 件の配備先が使用中か、内容が変更されています。解決後に更新してください。`,
  }[locale];
}

export function deploymentResult(locale: Locale, result: DeployResult): string {
  const { deployed, failed, global_available, project_local } = result;
  const available = global_available + project_local;
  return {
    en: [`${deployed} deployed`, available ? `${available} already available` : "", `${failed.length} failed`].filter(Boolean).join(" · "),
    "zh-CN": [`本次部署 ${deployed} 项`, available ? `已有 ${available} 项可用` : "", `失败 ${failed.length} 项`].filter(Boolean).join("，"),
    ja: [`配備 ${deployed} 件`, available ? `利用可能 ${available} 件` : "", `失敗 ${failed.length} 件`].filter(Boolean).join("、"),
  }[locale];
}
