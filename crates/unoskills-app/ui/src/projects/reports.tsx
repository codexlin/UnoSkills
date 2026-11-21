import { View } from "nsh";
import type { ScreenProps } from "../shared";
import { Label } from "./controls";
import type { Failure, InstallResult } from "./host";
import { strings } from "./strings";

export function Failures(props: { screen: ScreenProps; failures: Failure[] }) {
  return <If when={props.failures.length > 0}>
    <Each of={props.failures}>
      {(failure) => <Label theme={props.screen.theme} label={`${failure()[0]} — ${failure()[1]}`} danger />}
    </Each>
  </If>;
}

export function InstallReport(props: { screen: ScreenProps; result: InstallResult }) {
  const t = () => strings(props.screen.locale);
  return <View style={{ gap: 4 }}>
    <Label theme={props.screen.theme} label={`${t().installed} ${props.result.installed.length} · ${t().skipped} ${props.result.skipped_existing.length} · ${t().failures} ${props.result.failed.length}`} />
    <Failures screen={props.screen} failures={props.result.failed} />
  </View>;
}
