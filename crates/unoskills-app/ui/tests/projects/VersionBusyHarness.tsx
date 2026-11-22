import { mount, signal, View } from "nsh";
import { Action, Label, Modal } from "../../src/components";
import { defaults, makeTheme } from "../../src/theme";

function Harness() {
  const visible = signal(true);
  const busy = signal(true);
  const theme = makeTheme(defaults);
  function openReview() {
    busy.set(true);
    visible.set(true);
  }
  return <View style={{ flex: 1, padding: 24, gap: 12, backgroundColor: theme.background }}>
    <If when={!visible()}>
      <Label theme={theme}>Review closed</Label>
      <Action theme={theme} label="Open review" onPress={openReview} />
    </If>
    <Modal theme={theme} label="Version review" open={visible()}
      closeOnEscape={!busy()} closeOnBackdropPress={!busy()}
      onClose={() => { if (!busy()) visible.set(false); }}>
      <Label theme={theme}>{busy() ? "Review pending" : "Review ready"}</Label>
      <Action theme={theme} label="Finish work" disabled={!busy()} onPress={() => busy.set(false)} />
    </Modal>
  </View>;
}

mount(Harness);
