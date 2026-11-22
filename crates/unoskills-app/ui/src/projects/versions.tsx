import { cleanup, Scroll, signal, View } from "nsh";
import { Label, Modal } from "../components";
import type { ScreenProps } from "../shared";
import { Action, Feedback, Label as ProjectLabel, actions } from "./controls";
import { request, type InventoryRow, type VersionIntent, type VersionReview, type VersionTarget } from "./host";
import { operation } from "./state";
import { strings } from "./strings";

export function ProjectVersionDialog(props: {
  screen: ScreenProps; skill: InventoryRow; target: VersionTarget; close: () => void;
}) {
  const screen = props.screen;
  const t = () => strings(screen.locale);
  const op = operation(screen);
  const review = signal<VersionReview | undefined>(undefined);
  let active = true;
  cleanup(() => {
    active = false;
    const pending = review();
    if (pending) void cancel(pending.id);
  });

  function cancel(id: string) {
    return request("project.cancel_version", { preview_id: id }).catch(() => {});
  }

  function prepare(intent: VersionIntent) {
    const target = props.target;
    return op.run(async () => {
      const plan = await request("project.prepare_version", { ...target, intent });
      if (!active) await cancel(plan.id);
      return plan;
    }, review.set);
  }

  function applied(message: string) {
    review.set(undefined);
    props.close();
    screen.refresh();
    screen.notify(message, "success");
  }

  function apply(plan: VersionReview) {
    return op.run(async () => {
      review.set(undefined);
      await request("project.apply_version", { preview_id: plan.id });
    }, () => applied(t().versionApplied));
  }

  return <Modal theme={screen.theme} label={t().version} width={640} open
    closeOnEscape={!op.busy()} closeOnBackdropPress={!op.busy()}
    onClose={() => { if (!op.busy()) props.close(); }}>
    <View style={{ gap: 4 }}>
      <Label theme={screen.theme} heading>{props.skill.name}</Label>
      <ProjectLabel theme={screen.theme} muted lines={1} label={props.skill.path} title={props.skill.path} />
    </View>
    <If when={review()} else={
      <View style={{ gap: 8 }}>
        <Label theme={screen.theme}>{props.skill.locked_hash ? t().locked : t().followingLibrary}</Label>
        <If when={props.skill.locked_hash} else={<Label theme={screen.theme} muted>{t().lockExplanation}</Label>}>
          {(hash) => <ProjectLabel theme={screen.theme} muted label={hash().slice(0, 12)} title={hash()} />}
        </If>
        <If when={props.skill.locked_hash && props.skill.library_different == null}>
          <Label theme={screen.theme} muted>{t().versionUnavailable}</Label>
        </If>
      </View>
    }>
      {(plan) => <>
        <View style={{ ...actions, gap: 16 }}>
          <Label theme={screen.theme} muted>{t().versionCurrent} {plan().current_hash.slice(0, 12)}</Label>
          <Label theme={screen.theme}>{t().versionCandidate} {plan().candidate_hash.slice(0, 12)}</Label>
        </View>
        <If when={plan().files.length > 0} else={<Label theme={screen.theme}>{t().noVersionChanges}</Label>}>
          <Scroll style={{ minHeight: 0, height: 300, flexShrink: 1 }} contentStyle={{ gap: 12, paddingRight: 8 }}>
            <Each of={plan().files} key={(file) => file.path}>
              {(file) => <View style={{ gap: 4, minWidth: 0 }}>
                <Label theme={screen.theme} mono weight="600">{file().path}</Label>
                <Label theme={screen.theme} muted>{file().kind === "added" ? t().versionAdded
                  : file().kind === "removed" ? t().versionRemoved : t().versionModified}</Label>
                <If when={file().diff}>
                  {(diff) => <Label theme={screen.theme} mono>{diff()}</Label>}
                </If>
              </View>}
            </Each>
          </Scroll>
        </If>
      </>}
    </If>
    <Feedback theme={screen.theme} label={t().working} pending={op.busy()} error={op.error()} />
    <View style={{ ...actions, justifyContent: "flex-end" }}>
      <Action theme={screen.theme} label={t().cancel} disabled={op.busy()} run={props.close} />
      <If when={review()} else={
        <If when={props.skill.locked_hash} else={
          <Action theme={screen.theme} label={t().lockSkill} primary disabled={op.busy()}
            run={() => op.run(() => request("project.lock_skill", props.target), () => applied(t().locked))} />
        }>
          <Action theme={screen.theme} label={t().reviewFollow} disabled={op.busy() || props.skill.library_different == null}
            run={() => prepare("follow")} />
          <If when={props.skill.library_different}>
            <Action theme={screen.theme} label={t().reviewUpgrade} primary disabled={op.busy()} run={() => prepare("upgrade")} />
          </If>
        </If>
      }>
        {(plan) => <Action theme={screen.theme} label={plan().intent === "upgrade" ? t().applyUpgrade : t().applyFollow}
          primary disabled={op.busy()} run={() => apply(plan())} />}
      </If>
    </View>
  </Modal>;
}
