import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { createIpcHost, createWorkspaceStore, type Host, type WorkspaceStore } from "@terminescu/host";
import { createMockHost } from "@terminescu/host/mock";
import type { Terminal } from "@xterm/xterm";
import { NO_PLUGINS, fakeRustHalves } from "./plugins";

export interface Connection {
  windowId: string;
  host: Host;
  workspace: WorkspaceStore;
  /** Set on the mock host only, as `window.__terminescu`, for Playwright. */
  terminals?: Map<string, Terminal>;
}

/**
 * Inside Tauri the window's label is its window id in the core. In a plain browser (Vite dev
 * server, Playwright) the app runs on the mock Host.
 */
export async function connect(): Promise<Connection> {
  const inTauri = "__TAURI_INTERNALS__" in window;
  const host = inTauri ? createIpcHost() : createMockHost(undefined, { claude: !NO_PLUGINS, plugins: fakeRustHalves() });
  const workspace = createWorkspaceStore(host);
  await workspace.start();
  const windowId = inTauri
    ? getCurrentWebviewWindow().label
    : workspace.store.getState().workspace.windows[0]!.id;
  if (inTauri) return { windowId, host, workspace };
  const terminals = new Map<string, Terminal>();
  Object.assign(window, { __terminescu: { host, terminals } });
  return { windowId, host, workspace, terminals };
}
