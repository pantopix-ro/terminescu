import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { createIpcHost, createWorkspaceStore, type Host, type WorkspaceStore } from "@terminescu/host";
import { createMockHost } from "@terminescu/host/mock";

export interface Connection {
  windowId: string;
  host: Host;
  workspace: WorkspaceStore;
}

/**
 * Inside Tauri the window's label is its window id in the core. In a plain browser (Vite dev
 * server, Playwright) the app runs on the mock Host.
 */
export async function connect(): Promise<Connection> {
  const inTauri = "__TAURI_INTERNALS__" in window;
  const host = inTauri ? createIpcHost() : createMockHost();
  const workspace = createWorkspaceStore(host);
  await workspace.start();
  const windowId = inTauri
    ? getCurrentWebviewWindow().label
    : workspace.store.getState().workspace.windows[0]!.id;
  return { windowId, host, workspace };
}
