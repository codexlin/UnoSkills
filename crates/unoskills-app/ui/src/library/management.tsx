import { cleanup, Clipboard, effect, Scroll, type ScrollRef, signal, View, windowSize } from "nsh";
import { Action, AlertBanner, Label, LoadingLine, Modal } from "../components";
import { HostError, call, type ScreenProps } from "../shared";
import { request, type ConflictReview } from "../projects/host";
import { errorMessage, removeSkill, undeployProjectSkill } from "./host";
import { managementStrings, projectScopeLabel } from "./managementStrings";
import { getStrings, toastProjectUndeployed } from "./strings";
import type { AgentInfo, ProjectWorkspace, ReviewedDeployment, Skill } from "./types";

type ProjectDestination = { id: string; name: string };
type AgentDestination = { key: string; display_name: string };
type DeploymentRow = ReviewedDeployment & { scope: string; agent: string };
type Review =
  | { kind: "delete" | "undeploy"; name: string; rows: DeploymentRow[]; apply: () => Promise<string> }
  | { kind: "conflict"; name: string; scope: string; agent: string };

export function createSkillManagement(screen: ScreenProps) {
  const review = signal<Review | undefined>(undefined);
  const conflict = signal<ConflictReview | undefined>(undefined);
  const pending = signal(false);
  const overwrite = signal(false);
  const problem = signal<HostError | undefined>(undefined);
  let disposed = false;

  function cancelPreview(id: string | null | undefined) {
    if (id) void request("project.cancel_conflict", { preview_id: id }).catch(() => {});
  }
  cleanup(() => {
    disposed = true;
    cancelPreview(conflict()?.preview_id);
  });

  function open(next: Review) {
    if (disposed || pending() || review()) return false;
    problem.set(undefined);
    overwrite.set(false);
    review.set(next);
    return true;
  }

  function close(refresh = false) {
    if (pending()) return;
    cancelPreview(conflict()?.preview_id);
    conflict.set(undefined);
    review.set(undefined);
    if (refresh) screen.refresh();
  }

  function describe(error: unknown) {
    return error instanceof HostError ? error : new HostError("HOST_ERROR", errorMessage(error));
  }

  function deleteSkill(skill: Skill, agents: AgentInfo[], projects: ProjectWorkspace[]) {
    const deployments: ReviewedDeployment[] = [...skill.deployments, ...skill.project_deployments]
      .map(({ id, target_path }) => ({ id, target_path }));
    const agentName = (key: string) => agents.find((agent) => agent.key === key)?.display_name ?? key;
    const rows = [
      ...skill.deployments.map((dep) => ({
        id: dep.id, target_path: dep.target_path, agent: agentName(dep.agent_key),
        scope: getStrings(screen.locale).scope_global,
      })),
      ...skill.project_deployments.map((dep) => ({
        id: dep.id, target_path: dep.target_path, agent: agentName(dep.agent_key),
        scope: projectScopeLabel(screen.locale, projects.find((project) => project.id === dep.project_id)?.name
          ?? dep.project_id, dep.scope_path ?? ""),
      })),
    ];
    open({ kind: "delete", name: skill.name, rows, async apply() {
      await removeSkill(skill.id, deployments);
      return getStrings(screen.locale).toast_removed_skill;
    } });
  }

  function removeProjectSkill(skillId: string, name: string, project: ProjectDestination,
    scopePath: string, agent: AgentDestination, deployment: ReviewedDeployment) {
    const scope = projectScopeLabel(screen.locale, project.name, scopePath);
    open({ kind: "undeploy", name, rows: [{ ...deployment, scope, agent: agent.display_name }], async apply() {
      await undeployProjectSkill(skillId, project.id, agent.key, scopePath, deployment);
      return toastProjectUndeployed(screen.locale, scope, agent.key);
    } });
  }

  async function reviewConflict(skillId: string, name: string, project: ProjectDestination,
    scopePath: string, agent: AgentDestination) {
    const target = { skill_id: skillId, project_id: project.id, scope_path: scopePath, agent_key: agent.key };
    if (!open({ kind: "conflict", name, agent: agent.display_name,
      scope: projectScopeLabel(screen.locale, project.name, scopePath) })) return;
    pending.set(true);
    try {
      const checked = await request("project.review_conflict", target);
      if (disposed) cancelPreview(checked.preview_id);
      else conflict.set(checked);
    } catch (error) {
      if (!disposed) problem.set(describe(error));
    } finally {
      if (!disposed) pending.set(false);
    }
  }

  async function openFolder() {
    const checked = conflict();
    if (!checked?.open_folder || pending()) return;
    problem.set(undefined);
    try {
      await call("dialog.open_folder", { path: checked.path });
    } catch (error) {
      if (checked === conflict()) problem.set(describe(error));
    }
  }

  async function apply() {
    const current = review();
    const previewId = conflict()?.preview_id;
    if (!current || pending() || (current.kind === "conflict" && (!overwrite() || !previewId))) return;
    pending.set(true);
    problem.set(undefined);
    try {
      let message: string;
      if (current.kind === "conflict" && previewId) {
        await request("project.apply_conflict", { preview_id: previewId });
        conflict.set(undefined);
        message = managementStrings(screen.locale).overwritten;
      } else if (current.kind !== "conflict") {
        message = await current.apply();
      } else {
        return;
      }
      if (!disposed) {
        pending.set(false);
        close(true);
        screen.notify(message, "success");
      }
    } catch (error) {
      if (!disposed) {
        if (current.kind === "conflict") conflict.update((checked) => checked && { ...checked, preview_id: null });
        problem.set(describe(error));
      }
    } finally {
      if (!disposed) pending.set(false);
    }
  }

  return { review, conflict, pending, problem, overwrite, openFolder, close, apply,
    deleteSkill, removeProjectSkill, reviewConflict };
}

export function SkillManagementDialogs(props: {
  screen: ScreenProps; management: ReturnType<typeof createSkillManagement>;
}) {
  const screen = props.screen;
  const state = props.management;
  const t = () => managementStrings(screen.locale);
  const s = () => getStrings(screen.locale);
  const showDetails = signal(false);
  let body: ScrollRef | undefined;
  effect(() => {
    void state.review();
    showDetails.set(false);
  });
  effect(() => {
    if (state.problem()) body?.scrollTo({ y: 0, animated: false });
  });
  const color = () => screen.theme.popup.foreground;
  const failure = () => state.problem();
  const recoveries = () => failure()?.recoveryPaths ?? [];
  const affectedPath = () => failure()?.affectedPath;
  const stale = () => ["STALE_REMOVAL", "STALE_PROJECT_VERSION"].includes(failure()?.code ?? "");
  const blocked = () => failure()?.code === "TARGET_CONFLICT";

  return <If when={state.review()}>
    {(current) => <Modal theme={screen.theme} open={screen.active} width={640}
      label={current().kind === "delete" ? t().deleteSkill : current().kind === "undeploy"
        ? t().removeDeployment : state.overwrite() ? t().overwrite : t().reviewConflict}
      closeOnEscape={!state.pending()} closeOnBackdropPress={false}
      onClose={() => { showDetails.set(false); state.close(); }}>
      <Scroll ref={(ref) => { body = ref; }} style={{ minHeight: 0, flexShrink: 1, maxHeight: Math.max(140, windowSize().height - 260) }}
        contentStyle={{ gap: 10, paddingRight: 8 }}>
        <Label theme={screen.theme} color={color()} weight="600">{current().name}</Label>
        <If when={failure()}>
          <AlertBanner theme={screen.theme} tone="error" message={stale() ? t().stale : blocked() ? t().blocked : t().failed} />
          <If when={affectedPath()}>
            {(path) => <View style={{ gap: 4 }}>
              <Label theme={screen.theme} mono>{path()}</Label>
              <View style={{ alignItems: "flex-start" }}>
                <Action theme={screen.theme} variant="ghost" size="compact" label={s().library_copy_path}
                  onPress={() => void Clipboard.writeText(path()).then(() => screen.notify(s().library_copy_path, "info"))
                    .catch((error) => screen.notify(errorMessage(error), "error"))} />
              </View>
            </View>}
          </If>
          <If when={recoveries().length > 0} else={
            <If when={blocked()}><Label theme={screen.theme} color={color()}>{t().backUp}</Label></If>
          }>
            <Label theme={screen.theme} color={color()}>{t().recovery}</Label>
            <Each of={recoveries()}>{(path) => <Label theme={screen.theme} mono>{path()}</Label>}</Each>
          </If>
        </If>
        <If when={current().kind === "conflict"} else={
          <If when={current().kind !== "conflict" ? current() as Extract<Review, { rows: DeploymentRow[] }> : undefined}>
            {(removal) => <>
              <Label theme={screen.theme} color={color()}>{removal().kind === "delete" ? t().deleteDescription : t().removeDescription}</Label>
              <If when={removal().kind === "delete" && removal().rows.length > 0}>
                <Label theme={screen.theme} color={color()}>{t().deleteDeployments}</Label>
              </If>
              <If when={removal().rows.length === 0}><Label theme={screen.theme} muted>{t().noDeployments}</Label></If>
              <Each of={removal().rows} key={(row) => row.id}>
                {(row) => <TargetDetails screen={screen} agent={row().agent} scope={row().scope} path={row().target_path} />}
              </Each>
            </>}
          </If>
        }>
          <If when={current().kind === "conflict" ? current() as Extract<Review, { kind: "conflict" }> : undefined}>
            {(selection) => <TargetDetails screen={screen} agent={selection().agent} scope={selection().scope}
              path={state.conflict()?.path} />}
          </If>
          <If when={state.pending() && !state.conflict()}><LoadingLine theme={screen.theme} label={s().stack_project_checking} /></If>
          <If when={state.conflict()}>
            {(checked) => <>
              <Label theme={screen.theme} color={color()}>{state.overwrite() ? t().overwriteDescription : t()[checked().kind]}</Label>
              <If when={!state.overwrite() && checked().files !== null}>
                <Label theme={screen.theme} color={color()} weight="600">{checked().pinned ? t().pinnedFileChanges : t().fileChanges}</Label>
                <If when={!checked().files?.length}>
                  <Label theme={screen.theme} muted>{t().noFileChanges}</Label>
                </If>
                <Each of={checked().files ?? []} key={(file) => file.path}>
                  {(file) => <View style={{ gap: 4, minWidth: 0, paddingVertical: 4 }}>
                    <Label theme={screen.theme} mono weight="600">{file().path}</Label>
                    <Label theme={screen.theme} muted>{file().kind === "modified" ? t().modifiedFile : t()[file().kind]}</Label>
                    <Label theme={screen.theme} mono>{file().diff ?? t().noTextDiff}</Label>
                  </View>}
                </Each>
              </If>
              <If when={!state.overwrite() && !checked().preview_id && checked().kind === "modified"}>
                <Label theme={screen.theme} muted>{checked().pinned ? t().pinnedConflict : t().protected}</Label>
              </If>
            </>}
          </If>
        </If>
        <If when={failure()?.message ?? state.conflict()?.detail}>
          {(details) => <View style={{ gap: 4, alignItems: "flex-start" }}>
            <Action theme={screen.theme} variant="ghost" size="compact" label={t().details} selected={showDetails()}
              onPress={() => showDetails.update((visible) => !visible)} />
            <If when={showDetails()}><Label theme={screen.theme} muted mono>{details()}</Label></If>
          </View>}
        </If>
      </Scroll>
      <View style={{ flexDirection: "row", flexWrap: "wrap", justifyContent: "flex-end", alignItems: "center", gap: 8 }}>
        <If when={state.conflict()?.open_folder && !state.overwrite()}>
          <Action theme={screen.theme} variant="ghost" label={t().openFolder} disabled={state.pending()}
            onPress={() => void state.openFolder()} />
        </If>
        <If when={stale() || state.conflict()?.kind === "stale"}>
          <Action theme={screen.theme} label={t().closeRefresh} disabled={state.pending()}
            onPress={() => { showDetails.set(false); state.close(true); }} />
        </If>
        <Action theme={screen.theme} label={current().kind === "conflict" && !state.overwrite() ? t().keepFiles : s().btn_cancel}
          disabled={state.pending()} onPress={() => { showDetails.set(false); state.close(); }} />
        <If when={current().kind === "conflict"} else={
          <Action theme={screen.theme} variant="danger" label={current().kind === "delete" ? t().deleteSkill : t().removeDeployment}
            accessibilityLabel={current().kind === "delete" ? t().confirmDelete : t().confirmRemove}
            pending={state.pending()} disabled={stale() || recoveries().length > 0} onPress={() => void state.apply()} />
        }>
          <If when={state.conflict()?.preview_id}>
            <If when={state.overwrite()} else={
              <Action theme={screen.theme} label={t().reviewOverwrite} disabled={state.pending()}
                onPress={() => state.overwrite.set(true)} />
            }>
              <Action theme={screen.theme} variant="danger" label={t().overwrite} pending={state.pending()}
                onPress={() => void state.apply()} />
            </If>
          </If>
        </If>
      </View>
    </Modal>}
  </If>;
}

function TargetDetails(props: { screen: ScreenProps; agent: string; scope: string; path?: string }) {
  const theme = () => props.screen.theme;
  return <View style={{ gap: 2, minWidth: 0, paddingVertical: 6, borderTopWidth: 1, borderColor: theme().border }}>
    <Label theme={theme()} color={theme().popup.foreground} weight="500">{props.agent} · {props.scope}</Label>
    <If when={props.path}>{(path) => <Label theme={theme()} muted mono>{path()}</Label>}</If>
  </View>;
}
