import { Text, View, mount, signal, type ViewRef } from "nsh";
import { Action, Modal } from "../../src/components";
import { defaults, makeTheme } from "../../src/theme";

function App() {
  const theme = makeTheme(defaults);
  const shown = signal(true);
  const measurement = signal("");
  let body!: ViewRef;
  return <Modal theme={theme} label="Fixture dialog" open={shown()} onClose={() => shown.set(false)}>
    <View ref={body} style={{ height: 80, minHeight: 0, flexShrink: 1 }}>
      <Action theme={theme} label="Measure dialog" onPress={async () => {
        const result = (await body.measure())!;
        const { x: inset, y: top } = result.bounds.origin;
        const { width, height } = result.bounds.size;
        // The body stretches between the card's equal side insets. The heading sits above it.
        measurement.set(`Bounds: ${JSON.stringify({
          x: result.pagePosition.x - inset,
          y: result.pagePosition.y - top,
          width: width + 2 * inset,
          height: top + height + inset,
        })}`);
      }} />
      <Text>{measurement()}</Text>
    </View>
  </Modal>;
}

mount(App);
