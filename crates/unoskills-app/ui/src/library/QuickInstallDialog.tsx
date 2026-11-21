import { type InputRef, Text, View, effect, signal } from "nsh";
import { Dialog, Input } from "nsh/base";
import type { Theme } from "../shared";
import { ActionButton, AlertBanner } from "./controls";
import { errorMessage, installFromSource, pickFolder } from "./host";
import { type Locale, busyInstalling, getStrings, toastInstalled } from "./strings";
import type { Skill } from "./types";

export function QuickInstallDialog(props: {
  open: boolean;
  initialSource?: string;
  locale: Locale;
  theme: Theme;
  onOpenChange: (open: boolean) => void;
  onInstalled: (skill: Skill) => void;
  onNotify: (message: string, tone?: "info" | "success" | "error") => void;
}) {
  const source = signal("");
  const installing = signal(false);
  const inlineError = signal<string | null>(null);
  let inputRef: InputRef | undefined;

  const s = () => getStrings(props.locale);

  effect(() => {
    if (props.open) {
      inlineError.set(null);
      if (props.initialSource && props.initialSource !== "install") {
        source.set(props.initialSource);
      }
      queueMicrotask(() => inputRef?.focus());
    }
  });

  const submitInstall = async (overrideSource?: string) => {
    if (installing()) {
      return;
    }
    const target = (overrideSource ?? source()).trim();
    if (!target) {
      inlineError.set(s().toast_enter_source);
      props.onNotify(s().toast_enter_source, "error");
      return;
    }

    inlineError.set(null);
    installing.set(true);
    try {
      const skill = await installFromSource(target);
      source.set("");
      props.onOpenChange(false);
      props.onNotify(toastInstalled(props.locale, skill.name, skill.estimated_tokens), "success");
      props.onInstalled(skill);
    } catch (error) {
      const message = errorMessage(error);
      inlineError.set(message);
      props.onNotify(message, "error");
    } finally {
      installing.set(false);
    }
  };

  const browseFolder = async () => {
    if (installing()) {
      return;
    }
    try {
      const chosen = await pickFolder(s().prompt_pick_skill_dir);
      if (!chosen) {
        return;
      }
      source.set(chosen);
      await submitInstall(chosen);
    } catch (error) {
      const message = errorMessage(error);
      inlineError.set(message);
      props.onNotify(message, "error");
    }
  };

  return (
    <Dialog
      open={props.open}
      animation="fade"
      role="dialog"
      accessibilityLabel={s().quick_install_title}
      closeOnEscape={!installing()}
      closeOnBackdropPress={!installing()}
      onOpenChange={({ open }) => {
        if (!installing()) {
          props.onOpenChange(open);
        }
      }}
      backdropClass="bg-black/45"
      style={{
        margin: "auto",
        width: 500,
        maxWidth: "92%",
        borderRadius: 12,
        borderWidth: 1,
        borderColor: props.theme.border,
        backgroundColor: props.theme.surface,
        padding: 20,
        gap: 14,
      }}
    >
      <Text
        style={{
          fontSize: 16,
          fontWeight: "600",
          color: props.theme.foreground,
          fontFamily: props.theme.fontFamily,
        }}
      >
        {s().quick_install_title}
      </Text>

      <Input
        ref={inputRef}
        value={source()}
        onChange={({ value }) => {
          source.set(value);
          if (inlineError()) {
            inlineError.set(null);
          }
        }}
        onSubmit={() => {
          void submitInstall();
        }}
        placeholder={s().ph_install_source}
        accessibilityLabel={s().quick_install_title}
        disabled={installing()}
        autoCorrect={false}
        returnKeyType="done"
        submitBehavior="submit"
        style={{
          fontSize: 13,
          paddingHorizontal: 12,
          paddingVertical: 9,
          borderRadius: 8,
          borderWidth: 1,
          borderColor: props.theme.border,
          backgroundColor: props.theme.background,
          color: props.theme.foreground,
          placeholderColor: props.theme.muted,
          fontFamily: props.theme.monoFamily ?? props.theme.fontFamily,
        }}
      />

      <If when={installing()}>
        <Text
          role="summary"
            accessibilityState={{ busy: true }}
          style={{
            fontSize: 12,
            color: props.theme.muted,
            fontFamily: props.theme.fontFamily,
          }}
        >
          {busyInstalling(props.locale, source().trim())}
        </Text>
      </If>

      <If when={Boolean(inlineError())}>
        <AlertBanner theme={props.theme} tone="error" message={inlineError() ?? ""} />
      </If>

      <View
        style={{
          flexDirection: "row",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 8,
          marginTop: 4,
        }}
      >
        <ActionButton
          theme={props.theme}
          kind="outline"
          label={s().btn_browse_folder}
          disabled={installing()}
          onPress={() => {
            void browseFolder();
          }}
        />
        <View style={{ flexDirection: "row", alignItems: "center", gap: 8 }}>
          <ActionButton
            theme={props.theme}
            kind="ghost"
            label={s().btn_cancel}
            disabled={installing()}
            onPress={() => props.onOpenChange(false)}
          />
          <ActionButton
            theme={props.theme}
            kind="primary"
            label={s().btn_install}
            loading={installing()}
            onPress={() => {
              void submitInstall();
            }}
          />
        </View>
      </View>
    </Dialog>
  );
}
