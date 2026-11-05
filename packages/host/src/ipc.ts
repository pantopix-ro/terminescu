import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Host, PaneView } from "./host";
import type { Command, PatchBatch, Snapshot } from "./protocol";

export const PATCHES_EVENT = "host://patches";
/** The app menu sends each action to the focused window only. */
export const ACTION_EVENT = "menu://action";

const ignore = () => {};

export function createIpcHost(): Host {
  return {
    snapshot: () => invoke<Snapshot>("host_snapshot"),
    send: (command: Command) => invoke<number>("host_command", { command }),
    onPatches: (listener) => listen<PatchBatch>(PATCHES_EVENT, (e) => listener(e.payload)),
    async openPane(paneId, cols, rows, onOutput) {
      // Raw bytes arrive as an ArrayBuffer, never as JSON (D33).
      const channel = new Channel<ArrayBuffer>((buf) => onOutput(new Uint8Array(buf)));
      const view = await invoke<number>("pane_open", { pane: paneId, cols, rows, onOutput: channel });
      const pane: PaneView = {
        input: (data, binary = false) => void invoke("pane_input", { view, data, binary }).catch(ignore),
        resize: (c, r) => void invoke("pane_resize", { view, cols: c, rows: r }).catch(ignore),
        ack: (bytes) => void invoke("pane_ack", { view, bytes }).catch(ignore),
        close: () => void invoke("pane_close", { view }).catch(ignore),
      };
      return pane;
    },
    openUrl: (url) => invoke("open_url", { url }),
    onAction: (listener) => getCurrentWebviewWindow().listen<string>(ACTION_EVENT, (e) => listener(e.payload)),
  };
}
