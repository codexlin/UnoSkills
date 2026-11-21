import { host, nativeModule, signal } from "nsh";

export type Locale = "en" | "zh-CN" | "ja";
export type Page = "home" | "library" | "discover" | "projects" | "stacks" | "settings";
export type Tone = "info" | "success" | "error";

export interface Route {
  page: Page;
  skillId?: string;
  stackId?: string;
  projectId?: string;
  scopePath?: string;
  agentKey?: string;
  source?: string;
  tab?: string;
  focusSearch?: boolean;
}

export interface Theme {
  background: string;
  surface: string;
  foreground: string;
  muted: string;
  border: string;
  primary: string;
  onPrimary: string;
  accent: string;
  danger: string;
  warning: string;
  success: string;
  fontFamily?: string;
  monoFamily?: string;
  fontSize: number;
  radius: number;
}

export interface ScreenProps {
  locale: Locale;
  theme: Theme;
  route: Route;
  navigate(route: Route): void;
  active: boolean;
  revision: number;
  refresh(): void;
  notify(message: string, tone?: Tone): void;
}

type Reply<T> = { ok: true; value: T } | { ok: false; code: string; message: string };
type DialogRequest = { title?: string; name?: string; extensions?: string[]; path?: string };

interface FileDialogs {
  folder(title: string | null): Promise<string | null>;
  open(title: string | null, extensions: string[]): Promise<string | null>;
  save(title: string | null, name: string | null, extensions: string[]): Promise<string | null>;
  reveal(path: string): Promise<void>;
}

export class HostError extends Error {
  constructor(readonly code: string, message: string) {
    super(message);
  }
}

async function dialog(command: string, input: DialogRequest) {
  const { Files } = nativeModule<{ Files: FileDialogs }>("unoskills/desktop");
  switch (command) {
    case "dialog.folder":
      return Files.folder(input.title ?? null);
    case "dialog.open":
      return Files.open(input.title ?? null, input.extensions ?? []);
    case "dialog.save":
      return Files.save(input.title ?? null, input.name ?? null, input.extensions ?? []);
    case "dialog.reveal":
      if (input.path) return Files.reveal(input.path);
      throw new HostError("INVALID_PATH", "A path is required.");
    default:
      throw new HostError("UNKNOWN_COMMAND", command);
  }
}

export const pendingCalls = signal(0);

export async function call<Request, Response>(command: string, input: Request): Promise<Response> {
  if (command.startsWith("dialog.")) {
    return await dialog(command, input as DialogRequest) as Response;
  }
  const reportBusy = command !== "update.status" && command !== "update.check";
  if (reportBusy) pendingCalls.update((count) => count + 1);
  try {
    const response = await host.call("unoskills.command", { command, input }, { timeout: 0 }) as Reply<Response>;
    if (response.ok) return response.value;
    throw new HostError(response.code, response.message);
  } finally {
    if (reportBusy) pendingCalls.update((count) => count - 1);
  }
}

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
