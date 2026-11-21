import { cleanup, effect, keymap, List, Scroll, signal, View, type InputRef } from "nsh";
import type { ScreenProps } from "../shared";
import { Action, Check, Feedback, Field, Header, Label, actions, confirm, load, operation, padding, toggle, viewport } from "./controls";
import { request, type Stack, type StackFormData } from "./host";
import { stackMemberRows } from "./members";
import { strings } from "./strings";

type FormProps = { screen: ScreenProps; saved: (stack: Stack) => void };

export function StackEditor(props: FormProps & { stack?: Stack }) {
  const screen = props.screen;
  const t = () => strings(screen.locale);
  const original = props.stack;
  const originalRows = stackMemberRows(original?.skills ?? [], (member) => member);
  const op = operation(screen);
  const library = load(() => request("stack.library", null));
  const name = signal(original?.name ?? "");
  const author = signal(original?.author ?? "");
  const description = signal(original?.description ?? "");
  const tags = signal(original?.tags.join(", ") ?? "");
  const query = signal("");
  const added = signal<ReadonlySet<string>>(new Set());
  const removed = signal<ReadonlySet<number>>(new Set());
  const skills = () => {
    const term = query().trim().toLocaleLowerCase();
    return (library().data ?? []).filter((skill) =>
      !term || [skill.name, skill.slug, skill.description ?? ""].some((value) => value.toLocaleLowerCase().includes(term)));
  };
  const count = () => (original?.skills.length ?? 0) - removed().size + added().size;
  const ready = () => name().trim().length > 0 && count() > 0 && !op.busy();

  function cancel() {
    if (op.busy()) return;
    screen.navigate(original ? {
      page: "stacks", stackId: original.id, source: "Local",
      projectId: screen.route.projectId, scopePath: screen.route.scopePath, agentKey: screen.route.agentKey,
    } : { page: "stacks", source: "Local" });
  }

  function save() {
    if (!ready()) return;
    const data: StackFormData = {
      name: name().trim(), author: author().trim(), description: description().trim(),
      tags: [...new Set(tags().split(/[,，、]/).map((tag) => tag.trim()).filter(Boolean))], skill_ids: [...added()],
    };
    return op.run(() => original
      ? request("stack.edit", { ...data, stack_id: original.id, previous: original, remove_indices: [...removed()] })
      : request("stack.create", data),
    (stack) => {
      props.saved(stack);
      screen.notify(t().saved, "success");
    });
  }

  let search: InputRef | undefined;
  effect(() => {
    if (!screen.active) return;
    cleanup(keymap.bind({
      "mod+s": () => { void save(); },
      "mod+k": () => search?.focus(),
    }, { when: "always" }));
    cleanup(keymap.bind("escape", cancel, { when: "notEditing" }));
  });

  return <>
    <Header theme={screen.theme} title={original ? t().edit : t().create}>
      <Action theme={screen.theme} label={t().cancel} ghost disabled={op.busy()} run={cancel} />
    </Header>
    <Scroll style={viewport}>
      <View style={{ width: "100%", maxWidth: 720, minWidth: 0, ...padding }}>
        <If when={!original}><Label theme={screen.theme} label={t().createDescription} muted size={screen.theme.fontSize} /></If>
        <Field theme={screen.theme} label={t().name} placeholder={t().namePlaceholder} value={name()} update={name.set} />
        <Field theme={screen.theme} label={t().author} placeholder={t().authorPlaceholder} value={author()} update={author.set} />
        <Field theme={screen.theme} label={t().description} placeholder={t().descriptionPlaceholder} value={description()} update={description.set} />
        <Field theme={screen.theme} label={t().tags} value={tags()} update={tags.set} />
        <If when={originalRows.length > 0}>
          <Label theme={screen.theme} label={t().keepMembers} weight="600" />
          <List each={originalRows} keyExtractor={(row) => row.key} viewport="parent" estimatedItemSize={46}>
            {(row) => <View style={{ gap: 2, paddingVertical: 4 }}>
              <Check theme={screen.theme} label={row().item.name} checked={!removed().has(row().originalIndex)}
                disabled={op.busy()} update={(checked) => removed.set(toggle(removed(), row().originalIndex, !checked))} />
              <Label theme={screen.theme} label={row().item.source} title={row().item.source} muted lines={1} />
            </View>}
          </List>
        </If>
        <View style={{ minWidth: 0, gap: 8 }}>
          <Label theme={screen.theme} label={`${t().selectSkills} (${added().size})`} weight="600" />
          <Field theme={screen.theme} label={t().searchSkills} value={query()} update={query.set} search
            receiveRef={(element) => { search = element; }} />
          <Feedback theme={screen.theme} label={t().working} pending={library().pending} error={op.error() ?? library().error} />
          <If when={skills().length > 0} else={
            <If when={!library().pending && !library().error}>
              <Label theme={screen.theme} label={library().data?.length ? t().noSkills : t().noLibrarySkills} muted />
            </If>
          }>
            <List each={skills()} estimatedItemSize={36} keyExtractor={(skill) => skill.id}
              style={{ height: Math.min(5, skills().length) * 36, minWidth: 0, minHeight: 0, overflow: "hidden" }}>
              {(skill) => <View style={{ height: 36, justifyContent: "center", paddingHorizontal: 8 }}>
                <Check theme={screen.theme} label={skill().name} compact checked={added().has(skill().id)}
                  disabled={op.busy()} update={(checked) => added.set(toggle(added(), skill().id, checked))} />
              </View>}
            </List>
          </If>
        </View>
        <View style={{ ...actions, justifyContent: "flex-end" }}>
          <Action theme={screen.theme} label={t().saveStack} glyph="check" primary pending={op.busy()} disabled={!ready()} run={save} />
        </View>
      </View>
    </Scroll>
  </>;
}

export function StackImport(props: FormProps) {
  const screen = props.screen;
  const t = () => strings(screen.locale);
  const op = operation(screen);
  const toml = signal("");

  function cancel() {
    if (!op.busy()) screen.navigate({ page: "stacks", source: "Local" });
  }

  function chooseFile() {
    return op.run(async () => {
      const path = await request("dialog.open", { title: t().openFile, extensions: ["toml"] });
      return path ? request("stack.read_toml", { path }) : null;
    }, (text) => { if (text !== null) toml.set(text); });
  }

  function preview() {
    const raw = toml();
    if (!raw.trim()) return;
    return op.run(async () => {
      const { stack, previous } = await request("stack.preview_import", { toml: raw });
      if (toml() !== raw || !screen.active) return;
      const description = [
        `${t().includedSkills} (${stack.skills.length})`,
        ...stack.skills.slice(0, 5).map((member) => member.name),
        ...(stack.skills.length > 5 ? ["…"] : []),
        ...(previous ? ["", t().replaceImport] : []),
      ].join("\n");
      if (!await confirm(screen, stack.name, t().import, description, previous !== null)) return;
      return request("stack.import", { toml: raw, previous });
    }, (stack) => {
      if (!stack) return;
      props.saved(stack);
      screen.notify(t().imported, "success");
    });
  }

  return <>
    <Header theme={screen.theme} title={t().import}>
      <Action theme={screen.theme} label={t().cancel} ghost disabled={op.busy()} run={cancel} />
    </Header>
    <Scroll style={viewport}>
      <View style={{ width: "100%", maxWidth: 720, minWidth: 0, ...padding }}>
        <View style={actions}>
          <View style={{ flex: 1, minWidth: 0, flexBasis: 180 }}>
            <Label theme={screen.theme} label={t().importDescription} muted size={screen.theme.fontSize} />
          </View>
          <Action theme={screen.theme} label={t().openFile} compact run={chooseFile} disabled={op.busy()} />
        </View>
        <Field theme={screen.theme} label={t().pasteToml} placeholder={t().pasteToml} hideLabel
          value={toml()} update={toml.set} multiline mono height={280} />
        <Feedback theme={screen.theme} label={t().working} error={op.error()} />
        <View style={{ ...actions, justifyContent: "flex-end" }}>
          <Action theme={screen.theme} label={t().previewImport} primary pending={op.busy()} disabled={!toml().trim()} run={preview} />
        </View>
      </View>
    </Scroll>
  </>;
}
