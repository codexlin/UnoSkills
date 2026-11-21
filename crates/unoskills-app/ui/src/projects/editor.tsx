import { computed, effect, keymap, List, Scroll, signal, View } from "nsh";
import type { ScreenProps } from "../shared";
import { Action, Check, Feedback, Field, Heading, Label, actions, confirm, load, operation, padding, toggle, viewport } from "./controls";
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
    } : { page: "stacks" });
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

  effect(() => {
    if (!screen.active) return;
    const unbindSave = keymap.bind("mod+s", () => { void save(); }, { when: "always" });
    const unbindCancel = keymap.bind("escape", cancel, { when: "notEditing" });
    return () => {
      unbindSave();
      unbindCancel();
    };
  });

  return <Scroll style={viewport} contentStyle={padding}>
    <Heading theme={screen.theme} label={original ? t().edit : t().create} level={1} />
    <Field theme={screen.theme} label={t().name} value={name()} update={name.set} />
    <Field theme={screen.theme} label={t().author} value={author()} update={author.set} />
    <Field theme={screen.theme} label={t().description} value={description()} update={description.set} multiline />
    <Field theme={screen.theme} label={t().tags} value={tags()} update={tags.set} />
    <If when={original?.skills.length}>
      <Heading theme={screen.theme} label={t().keepMembers} />
      <List each={originalRows} keyExtractor={(row) => row.key} viewport="parent" estimatedItemSize={46}>
        {(row) => <View style={{ gap: 2, paddingVertical: 4 }}>
          <Check theme={screen.theme} label={row().item.name} checked={!removed().has(row().originalIndex)}
            disabled={op.busy()} update={(checked) => removed.set(toggle(removed(), row().originalIndex, !checked))} />
          <Label theme={screen.theme} label={row().item.source} muted />
        </View>}
      </List>
    </If>
    <Heading theme={screen.theme} label={`${t().selectSkills} (${added().size})`} />
    <Field theme={screen.theme} label={t().searchSkills} value={query()} update={query.set} search />
    <Feedback theme={screen.theme} label={op.busy() ? t().working : t().loading}
      pending={op.busy() || library().pending} error={op.error() ?? library().error} />
    <If when={!library().pending && !library().error && skills().length === 0}>
      <Label theme={screen.theme} label={t().noSkills} muted />
    </If>
    <List each={skills()} viewport="parent" estimatedItemSize={32} keyExtractor={(skill) => skill.id}>
      {(skill) => <Check theme={screen.theme} label={skill().name} checked={added().has(skill().id)}
        disabled={op.busy()} update={(checked) => added.set(toggle(added(), skill().id, checked))} />}
    </List>
    <View style={actions}>
      <Action theme={screen.theme} label={t().saveStack} primary disabled={!ready()} run={save} />
      <Action theme={screen.theme} label={t().cancel} disabled={op.busy()} run={cancel} />
    </View>
  </Scroll>;
}

export function StackImport(props: FormProps) {
  const screen = props.screen;
  const t = () => strings(screen.locale);
  const op = operation(screen);
  const toml = signal("");
  const staged = signal<{ stack: Stack; previous: string | null; raw: string } | undefined>(undefined);
  const stagedRows = computed(() => stackMemberRows(staged()?.stack.skills ?? [], (member) => member));

  function update(text: string) {
    toml.set(text);
    staged.set(undefined);
  }

  function chooseFile() {
    return op.run(async () => {
      const path = await request("dialog.open", { title: t().openFile, extensions: ["toml"] });
      return path ? request("stack.read_toml", { path }) : null;
    }, (text) => { if (text !== null) update(text); });
  }

  function preview() {
    const raw = toml();
    return op.run(() => request("stack.preview_import", { toml: raw }), (result) => {
      if (toml() === raw) staged.set({ ...result, raw });
    });
  }

  function finish() {
    const preview = staged();
    if (!preview || preview.raw !== toml()) return;
    return op.run(async () => {
      if (preview.previous && !await confirm(screen, preview.stack.name, t().import, t().replaceImport, true)) return;
      return request("stack.import", { toml: preview.raw, previous: preview.previous });
    }, (stack) => {
      if (!stack) return;
      props.saved(stack);
      screen.notify(t().imported, "success");
    });
  }

  return <Scroll style={viewport} contentStyle={padding}>
    <Heading theme={screen.theme} label={t().import} level={1} />
    <View style={actions}>
      <Action theme={screen.theme} label={t().openFile} run={chooseFile} disabled={op.busy()} />
    </View>
    <Field theme={screen.theme} label={t().pasteToml} value={toml()} update={update} multiline mono />
    <Feedback theme={screen.theme} label={t().working} pending={op.busy()} error={op.error()} />
    <View style={actions}>
      <Action theme={screen.theme} label={t().previewImport} primary
        disabled={op.busy() || !toml().trim()} run={preview} />
      <Action theme={screen.theme} label={t().cancel} disabled={op.busy()} run={() => screen.navigate({ page: "stacks" })} />
    </View>
    <If when={staged()}>
      {(preview) => <View style={{ gap: 8, borderTopWidth: 1, borderColor: screen.theme.border, paddingTop: 12 }}>
        <Heading theme={screen.theme} label={preview().stack.name} />
        <Label theme={screen.theme} label={`${t().includedSkills} (${preview().stack.skills.length})`} />
        <List each={stagedRows()} keyExtractor={(row) => row.key} viewport="parent" estimatedItemSize={24}>
          {(row) => <Label theme={screen.theme} label={row().item.name} />}
        </List>
        <If when={preview().previous}><Label theme={screen.theme} label={t().replaceImport} danger /></If>
        <View style={actions}>
          <Action theme={screen.theme} label={t().import} primary disabled={op.busy()} run={finish} />
        </View>
      </View>}
    </If>
  </Scroll>;
}
