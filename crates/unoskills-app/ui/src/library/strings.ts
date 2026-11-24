import type { DeployMode, MarketBoard } from "./types";

export type Locale = "en" | "zh-CN" | "ja";

export interface LibraryStrings {
  nav_library: string;
  project_overview: string;
  btn_install: string;
  ph_search_skills: string;
  library_scope: string;
  scope_global: string;
  btn_add_project: string;
  library_agent_filter: string;
  library_all_agents: string;
  tooltip_refresh: string;
  toast_refreshed: string;
  stack_project_checking: string;
  library_skill_column: string;
  library_deployment: string;
  library_project_available: string;
  library_token_column: string;
  empty_library_title: string;
  empty_library_desc: string;
  empty_search_title: string;
  empty_search_desc: string;
  btn_discover_adopt: string;
  empty_select_skill_title: string;
  empty_select_skill_desc: string;
  btn_cancel: string;
  btn_save: string;
  btn_edit_skill: string;
  library_preview: string;
  ph_skill_md: string;
  library_skill_actions: string;
  btn_update_skill: string;
  tooltip_reveal_folder: string;
  library_location: string;
  library_source: string;
  library_copy_path: string;
  library_path_copied: string;
  library_discard_title: string;
  library_discard_action: string;
  library_deployment_targets: string;
  library_deployed: string;
  library_not_deployed: string;
  library_project_deployed: string;
  library_project_local: string;
  library_project_conflict: string;
  library_project_unavailable: string;
  library_global_available: string;
  library_resolve_project_conflict: string;
  stack_deploy_project: string;
  btn_detach_project: string;
  confirm_detach_project_title: string;
  prompt_pick_project_folder: string;
  toast_project_detached: string;
  no_installed_agents: string;
  confirm_conflict_title: string;
  btn_replace: string;
  toast_removed_skill: string;

  // Discover, Marketplace & Local skills (Adopt)
  discover_title: string;
  discover_market_tab: string;
  discover_adopt_tab: string;
  btn_install_from_link: string;
  quick_install_title: string;
  ph_install_source: string;
  btn_browse_folder: string;
  prompt_pick_skill_dir: string;
  toast_enter_source: string;
  ph_market_search: string;
  market_search_hint: string;
  btn_search: string;
  btn_trending: string;
  board_trending: string;
  board_hot: string;
  board_all_time: string;
  market_search_results: string;
  market_sort: string;
  market_all_sources: string;
  market_source_label: string;
  market_source_search: string;
  market_source_filter: string;
  market_clear_source_filter: string;
  market_filter_scope: string;
  market_official: string;
  market_add_to_library: string;
  market_open_skill: string;
  already_in_library: string;
  market_same_name: string;
  market_view_details: string;
  market_loading: string;
  market_searching: string;
  market_loading_more: string;
  btn_load_more: string;
  empty_market_title: string;
  empty_market_desc: string;
  installs_unit: string;
  ph_adopt_search: string;
  btn_rescan: string;
  adopt_scanning: string;
  btn_adopt: string;
  adopt_select_group: string;
  adopt_clear_selection: string;
  adopt_confirm_benefit: string;
  adopt_confirm_description: string;
  adopt_view_failures: string;
  adopt_failure_title: string;
  adopt_close_failures: string;
  adopt_slug_conflict: string;
  empty_adopt_done_title: string;
  empty_adopt_done_desc: string;
  empty_adopt_filter_title: string;
  empty_adopt_filter_desc: string;
}

const EN: LibraryStrings = {
  nav_library: "Library",
  project_overview: "Project Overview",
  btn_install: "Install",
  ph_search_skills: "Search skills...",
  library_scope: "Deployment scope",
  scope_global: "Global",
  btn_add_project: "Add Project",
  library_agent_filter: "Filter by Agent",
  library_all_agents: "All agents",
  tooltip_refresh: "Refresh library & agents",
  toast_refreshed: "Refreshed library and agents.",
  stack_project_checking: "Checking project skills…",
  library_skill_column: "Skill",
  library_deployment: "Deployments",
  library_project_available: "Available",
  library_token_column: "Est. tokens",
  empty_library_title: "Library is empty",
  empty_library_desc: "Install a curated stack or add your agents’ local skills to the library.",
  empty_search_title: "No matching skills",
  empty_search_desc: "Try a different search keyword.",
  btn_discover_adopt: "Discover skills",
  empty_select_skill_title: "No skill selected",
  empty_select_skill_desc: "Pick a skill on the left to inspect SKILL.md and deploy to agents.",
  btn_cancel: "Cancel",
  btn_save: "Save",
  btn_edit_skill: "Edit SKILL.md",
  library_preview: "Preview",
  ph_skill_md: "SKILL.md content...",
  library_skill_actions: "Skill actions",
  btn_update_skill: "Update",
  tooltip_reveal_folder: "Reveal in File Manager",
  library_location: "Library location",
  library_source: "Source",
  library_copy_path: "Copy library path",
  library_path_copied: "Library path copied.",
  library_discard_title: "Discard unsaved changes?",
  library_discard_action: "Discard changes",
  library_deployment_targets: "Deployment targets",
  library_deployed: "Deployed",
  library_not_deployed: "Not deployed",
  library_project_deployed: "Deployed to project",
  library_project_local: "Present in project",
  library_project_conflict: "Project folder conflict",
  library_project_unavailable: "Skill files unavailable",
  library_global_available: "Available globally",
  library_resolve_project_conflict: "Resolve conflict",
  stack_deploy_project: "Deploy to Project",
  btn_detach_project: "Detach",
  confirm_detach_project_title: "Detach Project Workspace",
  prompt_pick_project_folder: "Select a local project directory to attach",
  toast_project_detached: "Detached project workspace and cleaned up managed skills.",
  no_installed_agents: "No installed agents detected yet.",
  confirm_conflict_title: "Target directory already exists",
  btn_replace: "Replace",
  toast_removed_skill: "Skill deleted from Library.",

  discover_title: "Discover",
  discover_market_tab: "Marketplace",
  discover_adopt_tab: "Local skills",
  btn_install_from_link: "Install from Link",
  quick_install_title: "Quick Install",
  ph_install_source: "owner/repo@skill, https://github.com/.../tree/main/..., or ./local-dir",
  btn_browse_folder: "Browse Folder",
  prompt_pick_skill_dir: "Select a local skill folder containing SKILL.md",
  toast_enter_source: "Enter a local path, Git URL, or owner/repo@skill first.",
  ph_market_search: "Search skills.sh",
  market_search_hint: "Press Enter to search, for example react, rust, or review",
  btn_search: "Search",
  btn_trending: "Refresh",
  board_trending: "Trending",
  board_hot: "Hot",
  board_all_time: "All Time",
  market_search_results: "Search results",
  market_sort: "Leaderboard",
  market_all_sources: "All sources",
  market_source_label: "Source",
  market_source_search: "Search sources",
  market_source_filter: "Loaded results",
  market_clear_source_filter: "Clear source filter",
  market_filter_scope: "Filter loaded results",
  market_official: "Official",
  market_add_to_library: "Add to library",
  market_open_skill: "View skill",
  already_in_library: "Already in Library",
  market_same_name: "Name in library",
  market_view_details: "View on skills.sh",
  market_loading: "Loading leaderboard from skills.sh...",
  market_searching: "Searching skills.sh...",
  market_loading_more: "Loading next page...",
  btn_load_more: "Load More",
  empty_market_title: "No skills found",
  empty_market_desc: "Try another keyword or click Refresh to reload the leaderboard.",
  installs_unit: "installs",
  ph_adopt_search: "Filter local skills by name, agent, or path…",
  btn_rescan: "Rescan",
  adopt_scanning: "Scanning for local skills...",
  btn_adopt: "Add to library",
  adopt_select_group: "Select this group",
  adopt_clear_selection: "Clear selection",
  adopt_confirm_benefit: "View and edit these skills in UnoSkills, then deploy them to other agents as needed.",
  adopt_confirm_description:
    "UnoSkills copies new skills to the library and replaces the original folders with links or managed copies. An identical local copy is kept as is and linked to the existing library entry for management.",
  adopt_view_failures: "View failures",
  adopt_failure_title: "Skills that could not be added",
  adopt_close_failures: "Close",
  adopt_slug_conflict: "This folder name is already in use in the library. The local skill was kept.",
  empty_adopt_done_title: "Everything is managed",
  empty_adopt_done_desc: "All detected agent skills are already in your UnoSkills library.",
  empty_adopt_filter_title: "No matching local skills",
  empty_adopt_filter_desc: "Try clearing your filter keyword.",
};

const ZH_CN: LibraryStrings = {
  nav_library: "技能库",
  project_overview: "项目概览",
  btn_install: "安装",
  ph_search_skills: "搜索技能名称、标签或描述...",
  library_scope: "部署范围",
  scope_global: "全局",
  btn_add_project: "添加项目",
  library_agent_filter: "按 Agent 筛选",
  library_all_agents: "所有 Agent",
  tooltip_refresh: "刷新技能库与 Agent 状态",
  toast_refreshed: "已刷新技能库与 Agent 状态。",
  stack_project_checking: "正在检查项目里的技能…",
  library_skill_column: "技能",
  library_deployment: "部署",
  library_project_available: "项目可用",
  library_token_column: "估算 tokens",
  empty_library_title: "技能库还是空的",
  empty_library_desc: "可以先安装一套推荐组合，或将本机已有的 Agent 技能纳入技能库。",
  empty_search_title: "未找到匹配的技能",
  empty_search_desc: "换个关键词试试吧。",
  btn_discover_adopt: "发现技能",
  empty_select_skill_title: "未选择技能",
  empty_select_skill_desc: "在左侧选择一个技能，即可预览 SKILL.md 并一键部署到各 Agent。",
  btn_cancel: "取消",
  btn_save: "保存",
  btn_edit_skill: "编辑 SKILL.md",
  library_preview: "预览",
  ph_skill_md: "SKILL.md 内容...",
  library_skill_actions: "技能操作",
  btn_update_skill: "检查更新",
  tooltip_reveal_folder: "在文件管理器中打开目录",
  library_location: "技能库位置",
  library_source: "来源",
  library_copy_path: "复制技能库路径",
  library_path_copied: "已复制技能库路径。",
  library_discard_title: "放弃未保存的修改？",
  library_discard_action: "放弃修改",
  library_deployment_targets: "部署目标",
  library_deployed: "已部署",
  library_not_deployed: "未部署",
  library_project_deployed: "已部署到项目",
  library_project_local: "项目已有",
  library_project_conflict: "项目目录有冲突",
  library_project_unavailable: "技能文件不可用",
  library_global_available: "全局可用",
  library_resolve_project_conflict: "处理冲突",
  stack_deploy_project: "部署到项目",
  btn_detach_project: "移除项目",
  confirm_detach_project_title: "移除项目目录",
  prompt_pick_project_folder: "选择要关联的本地项目目录",
  toast_project_detached: "已移除项目关联并清理受管技能部署。",
  no_installed_agents: "暂未检测到已安装的 AI Agent。",
  confirm_conflict_title: "目标目录已存在未托管文件",
  btn_replace: "覆盖替换",
  toast_removed_skill: "已从技能库删除该技能。",

  discover_title: "发现",
  discover_market_tab: "市场",
  discover_adopt_tab: "本机技能",
  btn_install_from_link: "从链接安装",
  quick_install_title: "快速安装",
  ph_install_source: "owner/repo@skill、GitHub 链接或 ./本地目录",
  btn_browse_folder: "选择目录",
  prompt_pick_skill_dir: "选择包含 SKILL.md 的本地技能目录",
  toast_enter_source: "请先输入本地路径、Git 链接或 owner/repo@skill。",
  ph_market_search: "搜索 skills.sh",
  market_search_hint: "按回车搜索，例如 react、rust、review",
  btn_search: "搜索",
  btn_trending: "刷新榜单",
  board_trending: "热门趋势",
  board_hot: "飙升榜",
  board_all_time: "总榜",
  market_search_results: "搜索结果",
  market_sort: "榜单",
  market_all_sources: "所有来源",
  market_source_label: "来源",
  market_source_search: "搜索来源",
  market_source_filter: "已加载结果",
  market_clear_source_filter: "清除来源筛选",
  market_filter_scope: "筛选已加载结果",
  market_official: "官方",
  market_add_to_library: "加入技能库",
  market_open_skill: "查看技能",
  already_in_library: "已在库中",
  market_same_name: "同名已在库",
  market_view_details: "在 skills.sh 查看",
  market_loading: "正在从 skills.sh 加载榜单...",
  market_searching: "正在搜索 skills.sh...",
  market_loading_more: "正在加载下一页...",
  btn_load_more: "加载更多",
  empty_market_title: "暂无技能结果",
  empty_market_desc: "换个关键词搜索，或点击右上角刷新重新拉取榜单。",
  installs_unit: "次安装",
  ph_adopt_search: "按名称、Agent 或路径筛选本机技能…",
  btn_rescan: "重新扫描",
  adopt_scanning: "正在扫描本机技能...",
  btn_adopt: "纳入技能库",
  adopt_select_group: "选中此组",
  adopt_clear_selection: "清除选择",
  adopt_confirm_benefit: "纳入后，你可以在 UnoSkills 里统一查看和编辑这些技能，也可以把它们部署给其他 Agent。",
  adopt_confirm_description:
    "新技能会复制到技能库，原目录会替换为链接或受管理的副本。内容完全相同的本机副本会留在原处，关联到已有的技能库条目。",
  adopt_view_failures: "查看失败详情",
  adopt_failure_title: "未能纳入的技能",
  adopt_close_failures: "关闭",
  adopt_slug_conflict: "技能库已使用这个目录名。本机技能已保留。",
  empty_adopt_done_title: "全部已托管",
  empty_adopt_done_desc: "本机已检测到的 Agent 技能都已在 UnoSkills 技能库中统一管理。",
  empty_adopt_filter_title: "没有匹配的本机技能",
  empty_adopt_filter_desc: "清空筛选词再试试。",
};

const JA: LibraryStrings = {
  nav_library: "ライブラリ",
  project_overview: "プロジェクト概要",
  btn_install: "追加",
  ph_search_skills: "スキル名・タグ・説明で検索...",
  library_scope: "デプロイ範囲",
  scope_global: "グローバル",
  btn_add_project: "プロジェクト追加",
  library_agent_filter: "Agent で絞り込む",
  library_all_agents: "すべての Agent",
  tooltip_refresh: "ライブラリとエージェント状態を更新",
  toast_refreshed: "ライブラリとエージェント状態を更新しました。",
  stack_project_checking: "プロジェクトのスキルを確認中…",
  library_skill_column: "スキル",
  library_deployment: "デプロイ",
  library_project_available: "利用可能",
  library_token_column: "推定トークン",
  empty_library_title: "スキルがまだありません",
  empty_library_desc: "おすすめスタックを導入するか、エージェントのローカルスキルをライブラリに追加しましょう。",
  empty_search_title: "一致するスキルがありません",
  empty_search_desc: "別のキーワードをお試しください。",
  btn_discover_adopt: "スキルを探す",
  empty_select_skill_title: "スキル未選択",
  empty_select_skill_desc: "左側のリストからスキルを選ぶと SKILL.md の確認と配備ができます。",
  btn_cancel: "キャンセル",
  btn_save: "保存",
  btn_edit_skill: "SKILL.md を編集",
  library_preview: "プレビュー",
  ph_skill_md: "SKILL.md の内容...",
  library_skill_actions: "スキルの操作",
  btn_update_skill: "更新確認",
  tooltip_reveal_folder: "ファイルマネージャで開く",
  library_location: "ライブラリの場所",
  library_source: "提供元",
  library_copy_path: "ライブラリのパスをコピー",
  library_path_copied: "ライブラリのパスをコピーしました。",
  library_discard_title: "未保存の変更を破棄しますか？",
  library_discard_action: "変更を破棄",
  library_deployment_targets: "配備先",
  library_deployed: "デプロイ済み",
  library_not_deployed: "未デプロイ",
  library_project_deployed: "プロジェクトに配備済み",
  library_project_local: "プロジェクトに追加済み",
  library_project_conflict: "プロジェクトのフォルダーが競合",
  library_project_unavailable: "スキルファイルを利用できません",
  library_global_available: "グローバルで利用可能",
  library_resolve_project_conflict: "競合を解決",
  stack_deploy_project: "プロジェクトに配備",
  btn_detach_project: "解除",
  confirm_detach_project_title: "プロジェクトを解除",
  prompt_pick_project_folder: "関連付けるローカルプロジェクトを選択",
  toast_project_detached: "プロジェクトを解除し、管理対象スキルの配備を整理しました。",
  no_installed_agents: "インストール済みの AI エージェントが見つかりません。",
  confirm_conflict_title: "配備先に未管理のフォルダが存在します",
  btn_replace: "上書き置換",
  toast_removed_skill: "スキルをライブラリから削除しました。",

  discover_title: "探す",
  discover_market_tab: "マーケット",
  discover_adopt_tab: "ローカルスキル",
  btn_install_from_link: "リンクから導入",
  quick_install_title: "クイック導入",
  ph_install_source: "owner/repo@skill、GitHub URL、または ./ローカルパス",
  btn_browse_folder: "フォルダ選択",
  prompt_pick_skill_dir: "SKILL.md を含むローカルスキルフォルダを選択",
  toast_enter_source: "ローカルパス・Git URL または owner/repo@skill を入力してください。",
  ph_market_search: "skills.sh を検索",
  market_search_hint: "Enter で検索（例：react、rust、review）",
  btn_search: "検索",
  btn_trending: "更新",
  board_trending: "トレンド",
  board_hot: "急上昇",
  board_all_time: "総合",
  market_search_results: "検索結果",
  market_sort: "ランキング",
  market_all_sources: "すべての提供元",
  market_source_label: "提供元",
  market_source_search: "提供元を検索",
  market_source_filter: "読み込み済みの結果",
  market_clear_source_filter: "提供元の絞り込みを解除",
  market_filter_scope: "読み込み済みの結果を絞り込む",
  market_official: "公式",
  market_add_to_library: "ライブラリに追加",
  market_open_skill: "スキルを表示",
  already_in_library: "導入済み",
  market_same_name: "同名が導入済み",
  market_view_details: "skills.sh で見る",
  market_loading: "skills.sh からランキングを読み込み中...",
  market_searching: "skills.sh を検索中...",
  market_loading_more: "次のページを読み込み中...",
  btn_load_more: "もっと読み込む",
  empty_market_title: "スキルが見つかりません",
  empty_market_desc: "キーワードを変えるか、更新ボタンで再読み込みしてください。",
  installs_unit: "導入",
  ph_adopt_search: "名前・エージェント・パスで絞り込み...",
  btn_rescan: "再スキャン",
  adopt_scanning: "ローカルスキルをスキャン中...",
  btn_adopt: "ライブラリに追加",
  adopt_select_group: "このグループを選択",
  adopt_clear_selection: "選択を解除",
  adopt_confirm_benefit: "追加すると、UnoSkills でこれらのスキルをまとめて確認・編集し、ほかのエージェントにも配備できます。",
  adopt_confirm_description:
    "新しいスキルはライブラリにコピーされ、元のフォルダーはリンクまたは管理対象のコピーに置き換わります。内容が完全に一致するローカルコピーはそのまま残し、既存のライブラリ項目に関連付けます。",
  adopt_view_failures: "失敗の詳細を表示",
  adopt_failure_title: "追加できなかったスキル",
  adopt_close_failures: "閉じる",
  adopt_slug_conflict: "このフォルダー名はライブラリで使用されています。ローカルのスキルは保持されました。",
  empty_adopt_done_title: "すべて管理済みです",
  empty_adopt_done_desc: "検出されたエージェントのスキルはすべて UnoSkills で管理されています。",
  empty_adopt_filter_title: "一致する未管理スキルがありません",
  empty_adopt_filter_desc: "絞り込み条件をクリアしてお試しください。",
};

export function getStrings(locale: Locale): LibraryStrings {
  switch (locale) {
    case "zh-CN":
      return ZH_CN;
    case "ja":
      return JA;
    case "en":
    default:
      return EN;
  }
}

export function boardLabel(locale: Locale, board: MarketBoard): string {
  const s = getStrings(locale);
  switch (board) {
    case "trending":
      return s.board_trending;
    case "hot":
      return s.board_hot;
    case "all-time":
      return s.board_all_time;
  }
}

function deployModeLabel(mode: DeployMode): string {
  return mode === "copy" ? "Copy" : "Symlink";
}

export function labelWithCount(locale: Locale, label: string, count: number): string {
  return locale === "en" ? `${label} (${count})` : `${label}（${count}）`;
}

export function toastSavedSkill(locale: Locale, name: string, tokens: number): string {
  switch (locale) {
    case "zh-CN":
      return `已保存 ${name}（约 ${tokens} tokens）`;
    case "ja":
      return `${name} を保存しました（約 ${tokens} tokens）`;
    default:
      return `Saved ${name} (~${tokens} tokens)`;
  }
}

export function toastUndeployed(locale: Locale, agentKey: string): string {
  switch (locale) {
    case "zh-CN":
      return `已从 ${agentKey} 取消部署`;
    case "ja":
      return `${agentKey} からデプロイ解除しました`;
    default:
      return `Undeployed from ${agentKey}`;
  }
}

export function toastDeployed(locale: Locale, agentKey: string, mode: DeployMode): string {
  const label = deployModeLabel(mode);
  switch (locale) {
    case "zh-CN":
      return `已部署到 ${agentKey}（${label}）`;
    case "ja":
      return `${agentKey} にデプロイしました（${label}）`;
    default:
      return `Deployed to ${agentKey} (${label})`;
  }
}

export function toastProjectAttached(locale: Locale, projectName: string): string {
  switch (locale) {
    case "zh-CN":
      return `已关联项目「${projectName}」，可单独勾选部署到该项目。`;
    case "ja":
      return `プロジェクト「${projectName}」を関連付けました。`;
    default:
      return `Attached project '${projectName}' for repo-local skills.`;
  }
}

export function toastProjectDeployed(
  locale: Locale,
  projectName: string,
  agentKey: string,
  mode: DeployMode,
): string {
  const label = deployModeLabel(mode);
  switch (locale) {
    case "zh-CN":
      return `已部署到项目「${projectName}」的 ${agentKey}（${label}）`;
    case "ja":
      return `「${projectName}」の ${agentKey} に配備しました（${label}）`;
    default:
      return `Deployed to '${projectName}' -> ${agentKey} (${label})`;
  }
}

export function toastProjectUndeployed(locale: Locale, projectName: string, agentKey: string): string {
  switch (locale) {
    case "zh-CN":
      return `已从项目「${projectName}」的 ${agentKey} 取消部署`;
    case "ja":
      return `「${projectName}」の ${agentKey} から配備解除しました`;
    default:
      return `Undeployed from '${projectName}' (${agentKey})`;
  }
}

export function toastSkillUpdated(locale: Locale, name: string, tokens: number, copies: number): string {
  switch (locale) {
    case "zh-CN":
      return copies > 0
        ? `已更新「${name}」（约 ${tokens} tokens，已同步刷新 ${copies} 个副本部署）`
        : `已更新「${name}」（约 ${tokens} tokens）`;
    case "ja":
      return copies > 0
        ? `「${name}」を更新しました（約 ${tokens} tokens・コピー配備 ${copies} 件を同期）`
        : `「${name}」を更新しました（約 ${tokens} tokens）`;
    default:
      return copies > 0
        ? `Updated '${name}' (~${tokens} tokens, refreshed ${copies} copy deployment(s)).`
        : `Updated '${name}' (~${tokens} tokens).`;
  }
}

export function toastSkillUpToDate(locale: Locale, name: string): string {
  switch (locale) {
    case "zh-CN":
      return `「${name}」已是最新版本，无需更新。`;
    case "ja":
      return `「${name}」はすでに最新です。`;
    default:
      return `'${name}' is already up to date.`;
  }
}

export function toastInstalled(locale: Locale, name: string, tokens: number): string {
  switch (locale) {
    case "zh-CN":
      return `已安装「${name}」（约 ${tokens} tokens）`;
    case "ja":
      return `「${name}」をインストールしました（約 ${tokens} tokens）`;
    default:
      return `Installed '${name}' (~${tokens} tokens).`;
  }
}

export function toastAdoptedAll(locale: Locale, ok: number, failed: number): string {
  switch (locale) {
    case "zh-CN":
      return `已将 ${ok} 个技能纳入技能库（失败 ${failed} 个）`;
    case "ja":
      return `${ok} 件のスキルをライブラリに追加しました（失敗 ${failed} 件）`;
    default:
      return `Added ${ok} skills to the library (${failed} failed).`;
  }
}

export function busyUpdatingSkill(locale: Locale, name: string): string {
  switch (locale) {
    case "zh-CN":
      return `正在检查并更新「${name}」...`;
    case "ja":
      return `「${name}」を更新確認中...`;
    default:
      return `Updating '${name}'...`;
  }
}

export function busyInstalling(locale: Locale, source: string): string {
  switch (locale) {
    case "zh-CN":
      return `正在安装 ${source}...`;
    case "ja":
      return `${source} をインストール中...`;
    default:
      return `Installing ${source}...`;
  }
}

export function busyAdoptingAll(locale: Locale, count: number): string {
  switch (locale) {
    case "zh-CN":
      return `正在将 ${count} 个本机技能纳入技能库...`;
    case "ja":
      return `ローカルスキル ${count} 件をライブラリに追加中...`;
    default:
      return `Adding ${count} local skills to the library...`;
  }
}

export function adoptSelectedCount(locale: Locale, count: number): string {
  switch (locale) {
    case "zh-CN":
      return `已选择 ${count} 项`;
    case "ja":
      return `${count} 件を選択`;
    default:
      return `${count} selected`;
  }
}

export function adoptConfirmTitle(locale: Locale, count: number): string {
  switch (locale) {
    case "zh-CN":
      return `将 ${count} 个技能纳入技能库？`;
    case "ja":
      return `スキル ${count} 件をライブラリに追加しますか？`;
    default:
      return count === 1 ? "Add 1 skill to the library?" : `Add ${count} skills to the library?`;
  }
}

export function confirmConflictDesc(locale: Locale, agentName: string, path: string): string {
  switch (locale) {
    case "zh-CN":
      return `${agentName} 目标路径已存在未托管的目录或文件：\n${path}\n\n是否替换为 UnoSkills 受管部署？`;
    case "ja":
      return `${agentName} の配備先に未管理のフォルダまたはファイルが存在します：\n${path}\n\nUnoSkills の管理対象に置き換えますか？`;
    default:
      return `An unmanaged folder or file already exists for ${agentName} at:\n${path}\n\nReplace it with the UnoSkills managed deployment?`;
  }
}

export function confirmDetachProjectDesc(locale: Locale, projectName: string, projectPath: string): string {
  switch (locale) {
    case "zh-CN":
      return `将取消关联项目「${projectName}」（${projectPath}），并移除 UnoSkills 在该项目内创建的受管技能部署（不会影响全局技能库及项目源码）。`;
    case "ja":
      return `プロジェクト「${projectName}」（${projectPath}）の関連付けを解除し、プロジェクト内の UnoSkills 管理対象配備を削除します（中央ライブラリとソースコードは保持されます）。`;
    default:
      return `Detach project '${projectName}' (${projectPath}) and remove UnoSkills-managed skill deployments inside it (your global library and repo code are untouched).`;
  }
}
