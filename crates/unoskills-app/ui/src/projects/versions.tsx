import { cleanup, Scroll, signal, View } from "nsh";
import { Action as Button, AlertBanner, Label, Modal } from "../components";
import { HostError, type ScreenProps } from "../shared";
import { managementStrings } from "../library/managementStrings";
import { Action, Feedback, Label as ProjectLabel, actions } from "./controls";
import { request, type InventoryRow, type VersionIntent, type VersionReview, type VersionTarget } from "./host";
import { operation } from "./state";
import { strings } from "./strings";

export function ProjectVersionDialog(props: {
  screen: ScreenProps; skill: InventoryRow; target: VersionTarget; scopeLabel: string; close: () => void;
}) {
  const screen = props.screen;
  const t = () => strings(screen.locale);
  const op = operation(screen);
  const review = signal<VersionReview | undefined>(undefined);
  const clearing = signal(false);
  const stalePin = signal(false);
  const retained = () => props.skill.location === "retained_pin";
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
    props.close();
    screen.refresh();
    screen.notify(message, "success");
  }

  function apply(plan: VersionReview) {
    return op.run(async () => {
      try {
        await request("project.apply_version", { preview_id: plan.id });
      } finally {
        if (active) review.set(undefined);
      }
    }, () => applied(t().versionApplied));
  }

  function clearPin() {
    const { project_id, scope_path, skill_id } = props.target;
    const expected_hash = props.skill.locked_hash;
    if (!expected_hash || !clearing() || stalePin()) return;
    return op.run(async () => {
      try {
        await request("project.clear_retained_pin", { project_id, scope_path, skill_id, expected_hash });
        return true;
      } catch (error) {
        if (!(error instanceof HostError) || error.code !== "STALE_PROJECT_VERSION") throw error;
        if (active) stalePin.set(true);
        return false;
      }
    }, (cleared) => { if (cleared) applied(t().pinCleared); });
  }

  return <Modal theme={screen.theme} label={retained()
    ? clearing() ? t().clearPin : managementStrings(screen.locale).viewPin : t().version} width={640} open
    closeOnEscape={!op.busy()} closeOnBackdropPress={!op.busy()}
    onClose={() => { if (!op.busy()) props.close(); }}>
    <View style={{ gap: 4 }}>
      <Label theme={screen.theme} heading>{props.skill.name}</Label>
      <Label theme={screen.theme} muted>{props.scopeLabel}</Label>
      <ProjectLabel theme={screen.theme} muted lines={1} label={props.skill.path} title={props.skill.path} />
    </View>
    <If when={review()} else={
      <View style={{ gap: 8 }}>
        <Label theme={screen.theme}>{retained() ? t().retained_pin : props.skill.locked_hash ? t().locked : t().followingLibrary}</Label>
        <If when={props.skill.locked_hash} else={<Label theme={screen.theme} muted>{t().lockExplanation}</Label>}>
          {(hash) => <ProjectLabel theme={screen.theme} muted label={hash().slice(0, 12)} title={hash()} />}
        </If>
        <If when={!retained() && props.skill.locked_hash && props.skill.library_different == null}>
          <Label theme={screen.theme} muted>{t().versionUnavailable}</Label>
        </If>
        <If when={clearing()}><Label theme={screen.theme}>{t().clearPinDescription}</Label></If>
        <If when={stalePin()}>
          <AlertBanner theme={screen.theme} tone="warning" message={managementStrings(screen.locale).stale} />
        </If>
      </View>
    }>
      {(reviewed) => {
        // A review is immutable for the lifetime of this conditional branch.
        const plan = reviewed();
        return <>
          <View style={{ ...actions, gap: 16 }}>
            <Label theme={screen.theme} muted>{t().versionCurrent} {plan.current_hash.slice(0, 12)}</Label>
            <Label theme={screen.theme}>{t().versionCandidate} {plan.candidate_hash.slice(0, 12)}</Label>
          </View>
          <If when={plan.files.length > 0} else={<Label theme={screen.theme}>{t().noVersionChanges}</Label>}>
            <Scroll style={{ minHeight: 0, height: 300, flexShrink: 1 }} contentStyle={{ gap: 12, paddingRight: 8 }}>
              <Each of={plan.files} key={(file) => file.path}>
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
        </>;
      }}
    </If>
    <Feedback theme={screen.theme} label={t().working} pending={op.busy()} error={op.error()} />
    <View style={{ ...actions, justifyContent: "flex-end" }}>
      <Action theme={screen.theme} label={t().cancel} disabled={op.busy()} run={props.close} />
      <If when={retained()} else={<If when={review()} else={
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
        {(reviewed) => {
          const plan = reviewed();
          return <Action theme={screen.theme} label={plan.intent === "upgrade" ? t().applyUpgrade : t().applyFollow}
            primary disabled={op.busy()} run={() => apply(plan)} />;
        }}
      </If>}>
        <If when={stalePin()} else={
          <If when={clearing()} else={
            <Action theme={screen.theme} label={t().clearPin} disabled={op.busy()} run={() => clearing.set(true)} />
          }>
            <Button theme={screen.theme} label={t().clearPin} accessibilityLabel={t().confirmClearPin}
              variant="danger" pending={op.busy()}
              onPress={() => void clearPin()} />
          </If>
        }>
          <Action theme={screen.theme} label={managementStrings(screen.locale).closeRefresh}
            run={() => { props.close(); screen.refresh(); }} />
        </If>
      </If>
    </View>
  </Modal>;
}
