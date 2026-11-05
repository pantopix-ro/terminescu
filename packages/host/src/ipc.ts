import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Host, PaneView } from "./host";
import type {
  AgentUpdate,
  Command,
  Id,
  PatchBatch,
  Pin,
  Rule,
  SavedStatus,
  Shown,
  Snapshot,
  StatusConfig,
  TranscriptChunk,
  Variable,
} from "./protocol";

export const PATCHES_EVENT = "host://patches";
export const AGENT_EVENT = "host://agent";
export const CONTEXT_EVENT = "host://context";
export const STATUS_EVENT = "host://status";
export const STATUS_CONFIG_EVENT = "host://status-config";
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
    agents: () => invoke<AgentUpdate[]>("host_agents"),
    onAgent: (listener) => listen<AgentUpdate>(AGENT_EVENT, (e) => listener(e.payload)),
    readTranscript: (session, offset) => invoke<TranscriptChunk>("transcript_read", { session, offset }),
    pins: (sideTab) => invoke<Pin[]>("pins_list", { sideTab }),
    addPin: (pin) => invoke<Pin>("pin_add", { pin: { ...pin, id: "", createdAt: 0 } }),
    removePin: (id) => invoke("pin_remove", { id }),
    variables: (sideTab) => invoke<Variable[]>("vars_list", { sideTab }),
    extractVariable: (sideTab, name, value) => invoke<Variable>("var_extract", { sideTab, name, value }),
    onContext: (listener) => listen<string>(CONTEXT_EVENT, (e) => listener(e.payload)),
    statuses: () => invoke<Record<Id, Shown>>("host_statuses"),
    onStatuses: (listener) => listen<Record<Id, Shown>>(STATUS_EVENT, (e) => listener(e.payload)),
    statusConfig: () => invoke<StatusConfig>("status_config"),
    onStatusConfig: (listener) => listen(STATUS_CONFIG_EVENT, () => listener()),
    saveStatus: (sideTab, look) => invoke<SavedStatus>("status_save", { sideTab, look }),
    updateStatus: (id, look) => invoke<SavedStatus>("status_update", { id, look }),
    deleteStatus: (id) => invoke("status_delete", { id }),
    makeGlobal: (id) => invoke<SavedStatus>("status_make_global", { id }),
    reorderStatuses: (ids) => invoke("status_reorder", { ids }),
    addRule: (statusId, kind) => invoke<Rule>("rule_add", { statusId, kind }),
    toggleRule: (id, enabled) => invoke("rule_toggle", { id, enabled }),
    deleteRule: (id) => invoke("rule_delete", { id }),
    setBuiltin: (auto, look) => invoke("builtin_set", { auto, look }),
    openUrl: (url) => invoke("open_url", { url }),
    onAction: (listener) => getCurrentWebviewWindow().listen<string>(ACTION_EVENT, (e) => listener(e.payload)),
  };
}
