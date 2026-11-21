import { cleanup, effect, signal, untracked } from "nsh";
import { errorMessage, type ScreenProps } from "../shared";

export function load<T>(fetch: () => Promise<T> | undefined, retain = false) {
  const state = signal<{ data?: T; error?: string; pending: boolean }>({ pending: true });
  effect(() => {
    let active = true;
    cleanup(() => { active = false; });
    const request = fetch();
    if (!request) {
      state.set({ pending: false });
      return;
    }
    const data = retain ? untracked(() => state().data) : undefined;
    state.set({ data, pending: true });
    void request.then(
      (data) => { if (active) state.set({ data, pending: false }); },
      (error: unknown) => { if (active) state.set({ data, error: errorMessage(error), pending: false }); },
    );
  });
  return state;
}

export function operation(props: ScreenProps) {
  const busy = signal(false);
  const error = signal<string | undefined>(undefined);
  let active = true;
  cleanup(() => { active = false; });
  return {
    busy, error,
    async run<T>(work: () => Promise<T>, done?: (result: T) => void) {
      if (busy()) return;
      busy.set(true);
      error.set(undefined);
      try {
        const result = await work();
        if (active) done?.(result);
      } catch (reason) {
        if (active) {
          const description = errorMessage(reason);
          error.set(description);
          props.notify(description, "error");
        }
      } finally {
        if (active) busy.set(false);
      }
    },
  };
}


export function toggle<T>(values: ReadonlySet<T>, value: T, checked: boolean): Set<T> {
  const next = new Set(values);
  if (checked) next.add(value);
  else next.delete(value);
  return next;
}
