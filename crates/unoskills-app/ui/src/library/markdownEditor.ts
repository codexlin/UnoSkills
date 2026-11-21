import { batch, signal } from "nsh";
import { AlertDialog } from "nsh/base";
import type { ScreenProps } from "../shared";
import { errorMessage, getSkillMarkdown, saveSkillMarkdown } from "./host";
import { getStrings, toastSavedSkill } from "./strings";
import type { Skill, SkillLintReport } from "./types";

export function createMarkdownEditor(
  screen: Pick<ScreenProps, "locale" | "notify" | "refresh">,
  selectedSkillId: () => string | null,
  onSaved: (skill: Skill) => void,
) {
  let markdownReqSeq = 0;
  const s = () => getStrings(screen.locale);
  const loadedMarkdownSkillId = signal<string | null>(null);
  const editingSkillId = signal<string | null>(null);
  const loadedMarkdown = signal("");
  const draftMarkdown = signal("");
  const previewMarkdown = signal("");
  const markdownLoading = signal(false);
  const markdownError = signal<string | null>(null);
  const savingSkillId = signal<string | null>(null);
  const selectedLint = signal<SkillLintReport | null>(null);

  const isEditingCurrent = () =>
    editingSkillId() !== null && editingSkillId() === selectedSkillId();

  const canEditCurrent = () =>
    !markdownLoading() &&
    savingSkillId() === null &&
    selectedSkillId() !== null &&
    loadedMarkdownSkillId() === selectedSkillId() &&
    markdownError() === null;

  const isDirty = () =>
    editingSkillId() !== null &&
    loadedMarkdownSkillId() === editingSkillId() &&
    draftMarkdown() !== loadedMarkdown();

  const clearMarkdownState = () => {
    batch(() => {
      loadedMarkdownSkillId.set(null);
      editingSkillId.set(null);
      loadedMarkdown.set("");
      draftMarkdown.set("");
      previewMarkdown.set("");
      selectedLint.set(null);
      markdownError.set(null);
    });
  };

  const confirmDiscardIfDirty = async (): Promise<boolean> => {
    if (!isDirty()) {
      return true;
    }
    const { action } = await AlertDialog.show({
      title: s().library_discard_title,
      actions: [
        { id: "cancel", label: s().btn_cancel, role: "cancel" },
        { id: "discard", label: s().library_discard_action, role: "destructive" },
      ],
    });
    return action === "discard";
  };

  const loadSelectedMarkdown = async (skillId: string | null, preserveEditor: boolean) => {
    const req = ++markdownReqSeq;
    if (!skillId) {
      markdownLoading.set(false);
      clearMarkdownState();
      return;
    }

    const keepingSameDraft =
      preserveEditor &&
      loadedMarkdownSkillId() === skillId &&
      editingSkillId() === skillId;

    if (!keepingSameDraft) {
      clearMarkdownState();
    } else {
      markdownError.set(null);
    }
    markdownLoading.set(true);

    try {
      const data = await getSkillMarkdown(skillId);
      if (req !== markdownReqSeq || selectedSkillId() !== skillId) {
        return;
      }
      batch(() => {
        loadedMarkdownSkillId.set(skillId);
        previewMarkdown.set(data.preview_markdown);
        selectedLint.set(data.lint);
        markdownError.set(null);
        if (!keepingSameDraft) {
          loadedMarkdown.set(data.raw_markdown);
          draftMarkdown.set(data.raw_markdown);
        }
      });
    } catch (error) {
      if (req !== markdownReqSeq || selectedSkillId() !== skillId) {
        return;
      }
      const message = errorMessage(error);
      batch(() => {
        previewMarkdown.set("");
        selectedLint.set(null);
        markdownError.set(message);
        if (!keepingSameDraft) {
          loadedMarkdownSkillId.set(null);
          editingSkillId.set(null);
          loadedMarkdown.set("");
          draftMarkdown.set("");
        }
      });
      screen.notify(message, "error");
    } finally {
      if (req === markdownReqSeq) {
        markdownLoading.set(false);
      }
    }
  };

  const startEditMarkdown = () => {
    const skillId = selectedSkillId();
    if (!skillId || !canEditCurrent()) {
      return;
    }
    batch(() => {
      draftMarkdown.set(loadedMarkdown());
      editingSkillId.set(skillId);
    });
  };

  const saveMarkdown = async () => {
    const targetSkillId = editingSkillId();
    if (
      !targetSkillId ||
      savingSkillId() !== null ||
      loadedMarkdownSkillId() !== targetSkillId
    ) {
      return;
    }
    const contentToSave = draftMarkdown();
    savingSkillId.set(targetSkillId);
    try {
      const result = await saveSkillMarkdown(targetSkillId, contentToSave);
      batch(() => {
        onSaved(result.skill);
        if (selectedSkillId() === targetSkillId) {
          markdownReqSeq += 1;
          loadedMarkdownSkillId.set(targetSkillId);
          loadedMarkdown.set(result.raw_markdown);
          draftMarkdown.set(result.raw_markdown);
          previewMarkdown.set(result.preview_markdown);
          selectedLint.set(result.lint);
          markdownError.set(null);
          if (editingSkillId() === targetSkillId) {
            editingSkillId.set(null);
          }
        }
      });
      screen.notify(
        toastSavedSkill(screen.locale, result.skill.name, result.skill.estimated_tokens),
        "success",
      );
      screen.refresh();
    } catch (error) {
      screen.notify(errorMessage(error), "error");
    } finally {
      if (savingSkillId() === targetSkillId) {
        savingSkillId.set(null);
      }
    }
  };

  const requestCancelEdit = async () => {
    if (!isEditingCurrent()) {
      return;
    }
    if (!(await confirmDiscardIfDirty())) {
      return;
    }
    batch(() => {
      draftMarkdown.set(loadedMarkdown());
      editingSkillId.set(null);
    });
  };

  return {
    editing: isEditingCurrent,
    editingSkill: (id: string) => editingSkillId() === id,
    hasDraftFor: (id: string) => editingSkillId() === id && loadedMarkdownSkillId() === id,
    canEdit: canEditCurrent,
    dirty: isDirty,
    loading: () => markdownLoading(),
    error: () => markdownError(),
    saving: () => savingSkillId() !== null,
    draft: () => draftMarkdown(),
    preview: () => previewMarkdown(),
    lint: () => selectedLint(),
    changeDraft: (content: string) => draftMarkdown.set(content),
    endEditing: () => editingSkillId.set(null),
    confirmDiscardIfDirty,
    load: loadSelectedMarkdown,
    start: startEditMarkdown,
    save: saveMarkdown,
    cancel: requestCancelEdit,
  };
}
