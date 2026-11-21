import { Text, View, keymap, mount, signal } from "nsh";
import { App } from "../../src/Shell";

function Harness() {
  const lastShortcut = signal("");
  keymap.bind({
    "mod+s": () => lastShortcut.set("save"),
    "escape": () => lastShortcut.set("cancel"),
  }, { when: "always" });
  return (
    <View style={{ flex: 1, minHeight: 0 }}>
      <App />
      <Text>{lastShortcut()}</Text>
    </View>
  );
}

mount(Harness);
