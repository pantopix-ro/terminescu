import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import type { Host, PaneView } from "./host";
import type { KeyBindings } from "./keys";
import type {
  AgentUpdate,
  AttentionSettings,
  AppSettings,
  RunOutcome,
  SavedCommand,
  Command,
  DocView,
  FileList,
  Id,
  Item,
  PatchBatch,
  Pin,
  Prompt,
  PromptVersion,
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
export const PROMPTS_EVENT = "host://prompts";
export const COMMANDS_EVENT = "host://commands";
export const SETTINGS_EVENT = "host://settings";
export const KEYS_EVENT = "host://keys";
export const STATUS_EVENT = "host://status";
export const STATUS_CONFIG_EVENT = "host://status-config";
/** The core sends these to one window: the one holding the pane, for `⌘⇧A` and the chime. */
export const REVEAL_EVENT = "host://reveal-pane";
export const CHIME_EVENT = "host://chime";
/** A document changed outside the editor (D28). */
export const DOCUMENT_EVENT = "host://document";
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
    jumpWaiting: () => invoke<Id | null>("attention_jump"),
    onRevealPane: (listener) => getCurrentWebviewWindow().listen<Id>(REVEAL_EVENT, (e) => listener(e.payload)),
    onChime: (listener) => getCurrentWebviewWindow().listen(CHIME_EVENT, () => listener()),
    attentionSettings: () => invoke<AttentionSettings>("attention_settings"),
    setAttentionSettings: (settings) => invoke("attention_set", { settings }),
    commands: (sideTab) => invoke<SavedCommand[]>("commands_list", { sideTab }),
    allCommands: () => invoke<SavedCommand[]>("commands_all"),
    settings: () => invoke<AppSettings>("settings_get"),
    setSettings: (settings) => invoke<AppSettings>("settings_set", { settings }),
    onSettings: (listener) => listen(SETTINGS_EVENT, () => listener()),
    keyBindings: () => invoke<KeyBindings>("keys_get"),
    setKeyBindings: (bindings) => invoke<KeyBindings>("keys_set", { bindings }),
    onKeyBindings: (listener) => listen(KEYS_EVENT, () => listener()),
    recordKeys: (on) => invoke("keys_record", { on }),
    createCommand: (command) => invoke<SavedCommand>("command_create", { command }),
    updateCommand: (id, command) => invoke<SavedCommand>("command_update", { id, command }),
    deleteCommand: (id) => invoke("command_delete", { id }),
    runSavedCommand: (r) =>
      invoke<RunOutcome>("command_run", {
        windowId: r.windowId,
        sideTab: r.sideTabId,
        id: r.id,
        target: r.target ?? null,
        pane: r.paneId ?? null,
        run: r.run,
        extra: r.extra ?? {},
      }),
    restartLauncher: (sideTab, id) => invoke("launcher_restart", { sideTab, id: id ?? null }),
    onCommands: (listener) => listen(COMMANDS_EVENT, () => listener()),
    readTranscript: (session, offset) => invoke<TranscriptChunk>("transcript_read", { session, offset }),
    pins: (sideTab) => invoke<Pin[]>("pins_list", { sideTab }),
    addPin: (pin) => invoke<Pin>("pin_add", { pin: { ...pin, id: "", createdAt: 0 } }),
    removePin: (id) => invoke("pin_remove", { id }),
    variables: (sideTab) => invoke<Variable[]>("vars_list", { sideTab }),
    addVariable: (sideTab, variable, replace = false) => invoke<Variable>("var_add", { sideTab, variable, replace }),
    changeVariable: (sideTab, name, change) => invoke<Variable>("var_change", { sideTab, name, change }),
    removeVariable: (sideTab, name) => invoke("var_remove", { sideTab, name }),
    items: (sideTab) => invoke<Item[]>("items_list", { sideTab }),
    addItem: (sideTab, kind, value, source, originTabId = null) =>
      invoke<Item>("item_add", { sideTab, kind, value, source, originTabId }),
    removeItem: (id) => invoke("item_remove", { id }),
    onContext: (listener) => listen<string>(CONTEXT_EVENT, (e) => listener(e.payload)),
    searchPrompts: (query, archived = false) => invoke<Prompt[]>("prompts_search", { query, archived }),
    createPrompt: (prompt) => invoke<Prompt>("prompt_create", { prompt }),
    savePrompt: (id, body) => invoke<Prompt>("prompt_save", { id, body }),
    updatePrompt: (id, name, tags) => invoke<Prompt>("prompt_update", { id, name, tags }),
    promptVersions: (id) => invoke<PromptVersion[]>("prompt_versions", { id }),
    restorePrompt: (id, version) => invoke<Prompt>("prompt_restore", { id, version }),
    duplicatePrompt: (id) => invoke<Prompt>("prompt_duplicate", { id }),
    archivePrompt: (id, archived) => invoke<Prompt>("prompt_archive", { id, archived }),
    markPromptUsed: (id) => invoke("prompt_used", { id }),
    onPrompts: (listener) => listen(PROMPTS_EVENT, () => listener()),
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
    newText: (sideTab, beside) => invoke<DocView>("doc_new", { sideTab, beside: beside ?? null }),
    openFile: (sideTab, path, beside) => invoke<DocView>("doc_open_file", { sideTab, path, beside: beside ?? null }),
    document: (id) => invoke<DocView>("doc_get", { id }),
    editDocument: (id, text) => invoke<DocView>("doc_edit", { id, text }),
    saveDocument: (id, path) => invoke<DocView>("doc_save", { id, path: path ?? null }),
    reloadDocument: (id) => invoke<DocView>("doc_reload", { id }),
    keepMine: (id) => invoke<DocView>("doc_keep_mine", { id }),
    onDocument: (listener) => listen<DocView>(DOCUMENT_EVENT, (e) => listener(e.payload)),
    listFiles: (pane) => invoke<FileList>("files_list", { pane }),
    chooseFile: async (defaultPath) => {
      const picked = await openDialog({ multiple: false, directory: false, defaultPath });
      return typeof picked === "string" ? picked : null;
    },
    chooseSavePath: (defaultPath) => saveDialog({ defaultPath }),
    onAction: (listener) => getCurrentWebviewWindow().listen<string>(ACTION_EVENT, (e) => listener(e.payload)),
  };
}
