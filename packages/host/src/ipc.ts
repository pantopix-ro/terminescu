import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import type { Host, PaneView } from "./host";
import { ACTIONS, plusActionFor, withBindings, type KeyBindings } from "./keys";
import type {
  Decoration,
  PluginInfo,
  TerminalProfile,
  AgentUpdate,
  DragEnd,
  DragPoint,
  Reopened,
  AttentionSettings,
  AppInfo,
  AppSettings,
  RunOutcome,
  SavedCommand,
  Command,
  DocView,
  FileList,
  Id,
  Item,
  TouchedFile,
  MergeClash,
  MergeDone,
  PatchBatch,
  Pin,
  Prompt,
  PromptVersion,
  Rule,
  SavedStatus,
  PaneLabel,
  Shown,
  Snapshot,
  StatusConfig,
  TranscriptChunk,
  Variable,
} from "./protocol";

export const PATCHES_EVENT = "host://patches";
/** Task 8.1: a drag from another window over this one, and a drop on it. */
export const DRAG_HOVER_EVENT = "host://drag-hover";
export const DRAG_DROP_EVENT = "host://drag-drop";
export const AGENT_EVENT = "host://agent";
export const CONTEXT_EVENT = "host://context";
export const PROMPTS_EVENT = "host://prompts";
export const COMMANDS_EVENT = "host://commands";
export const SETTINGS_EVENT = "host://settings";
export const UPDATE_EVENT = "host://update";
export const KEYS_EVENT = "host://keys";
export const STATUS_EVENT = "host://status";
export const LABEL_EVENT = "host://labels";
export const STATUS_CONFIG_EVENT = "host://status-config";
/** The core sends these to one window: the one holding the pane, for `⌘⇧A` and the chime. */
export const REVEAL_EVENT = "host://reveal-pane";
export const CHIME_EVENT = "host://chime";
/** A document changed outside the editor (D28). */
export const DOCUMENT_EVENT = "host://document";
/** The app menu sends each action to the focused window only. */
export const ACTION_EVENT = "menu://action";
export const DECORATIONS_EVENT = "host://decorations";
export const PLUGIN_EVENT = "host://plugin";
export const PLUGINS_EVENT = "host://plugins";

const ignore = () => {};

export function createIpcHost(): Host {
  // Settings › Keys records: key events belong to the recorder.
  let recording = false;
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
        clear: (keepScreen) => void invoke("pane_clear", { view, keepScreen }).catch(ignore),
        close: () => void invoke("pane_close", { view }).catch(ignore),
      };
      return pane;
    },
    agents: () => invoke<AgentUpdate[]>("host_agents"),
    pluginCall: <T>(plugin: string, method: string, args?: unknown) => invoke<T>("plugin_call", { plugin, method, args: args ?? null }),
    onPluginEvent: (plugin, listener) =>
      listen<{ plugin: string; event: string; payload: unknown }>(PLUGIN_EVENT, (e) => {
        if (e.payload.plugin === plugin) listener(e.payload.event, e.payload.payload);
      }),
    plugins: () => invoke<PluginInfo[]>("plugin_list"),
    setPluginEnabled: (plugin, enabled) => invoke<PluginInfo[]>("plugin_set_enabled", { plugin, enabled }),
    onPlugins: (listener) => listen<PluginInfo[]>(PLUGINS_EVENT, (e) => listener(e.payload)),
    decorations: () => invoke<Decoration[]>("decorations"),
    onDecorations: (listener) => listen<Decoration[]>(DECORATIONS_EVENT, (e) => listener(e.payload)),
    setPluginActions: (items) => invoke<void>("plugin_menu", { items: items.map(({ id, label, menu, accel }) => ({ id, label, menu, accel })) }),
    rowSelected: (window, sideTab) => invoke<void>("plugin_row_selected", { window, sideTab }),
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
    recordKeys: (on) => {
      recording = on;
      return invoke("keys_record", { on });
    },
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
    touchedFiles: (sideTab) => invoke<TouchedFile[]>("files_touched", { sideTab }),
    forgetFile: (id) => invoke("files_forget", { id }),
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
    labels: () => invoke<Record<Id, PaneLabel>>("host_labels"),
    onLabels: (listener) => listen<Record<Id, PaneLabel>>(LABEL_EVENT, (e) => listener(e.payload)),
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
    appInfo: () => invoke<AppInfo>("app_info"),
    feedbackOpen: (route, includeLog) => invoke("feedback_open", { route, includeLog }),
    uninstallHelpers: (deleteData) => invoke("uninstall_helpers", { deleteData }),
    appQuit: () => invoke("app_quit"),
    editCommand: (action) => invoke("edit_command", { action }),
    checkUpdates: () => invoke<string | null>("update_check"),
    updateRestart: () => invoke("update_restart"),
    onUpdate: (listener) => listen<string>(UPDATE_EVENT, (e) => listener(e.payload)),
    openPathWith: (pane, path, reveal) => invoke("path_open", { pane, path, reveal }),
    bounceDock: () => invoke("bounce_dock"),
    importTerminalApp: () => invoke<TerminalProfile>("import_terminal_app"),
    importTerminalFile: (path) => invoke<TerminalProfile>("import_terminal_file", { path }),
    newText: (sideTab, beside) => invoke<DocView>("doc_new", { sideTab, beside: beside ?? null }),
    openFile: (sideTab, path, beside) => invoke<DocView>("doc_open_file", { sideTab, path, beside: beside ?? null }),
    openPath: (sideTab, pane, path, beside) => invoke<DocView>("doc_open_path", { sideTab, pane, path, beside: beside ?? null }),
    returnEdit: (id) => invoke("doc_return", { id }),
    sendText: (pane, text) => invoke("pane_send_text", { pane, text }),
    mergePreview: (from, into, label) => invoke<MergeClash[]>("side_tab_merge_preview", { from, into, label }),
    mergeSideTabs: (from, into, label, resolution) => invoke<MergeDone>("side_tab_merge", { from, into, label, resolution }),
    undoMerge: (undoId) => invoke("side_tab_merge_undo", { undoId }),
    reopenClosed: (windowId) => invoke<Reopened | null>("reopen_closed", { windowId }),
    dragBegin: (item, title) => invoke<number>("drag_begin", { item, title }),
    dragEnd: (token, drop) => invoke<DragEnd>("drag_end", { token, drop }),
    onDragHover: (listener) => getCurrentWebviewWindow().listen<DragPoint | null>(DRAG_HOVER_EVENT, (e) => listener(e.payload)),
    onDragDrop: (listener) => getCurrentWebviewWindow().listen<DragPoint>(DRAG_DROP_EVENT, (e) => listener(e.payload)),
    pasteImage: (id, bytes, ext) => invoke<string>("doc_paste_image", bytes, { headers: { "x-doc": id, "x-ext": ext } }),
    openPrompt: (sideTab, promptId, beside) => invoke<DocView>("doc_open_prompt", { sideTab, promptId, beside: beside ?? null }),
    openVariable: (sideTab, name, beside) => invoke<DocView>("doc_open_variable", { sideTab, name, beside: beside ?? null }),
    onFileDrop: (listener) => {
      const win = getCurrentWebviewWindow();
      return win.onDragDropEvent((e) => {
        if (e.payload.type !== "drop" || !e.payload.paths.length) return;
        const { paths, position } = e.payload;
        // The point comes in physical pixels; the window's width in both units gives the factor,
        // the screen's scale and the page zoom together.
        void win.innerSize().then(
          (size) => {
            const f = size.width / window.innerWidth || 1;
            listener(paths, { x: position.x / f, y: position.y / f });
          },
          () => listener(paths),
        );
      });
    },
    document: (id) => invoke<DocView>("doc_get", { id }),
    editDocument: (id, text, rev) => invoke<DocView>("doc_edit", { id, text, rev: rev ?? null }),
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
    onAction: async (listener) => {
      const offMenu = await getCurrentWebviewWindow().listen<string>(ACTION_EVENT, (e) => listener(e.payload));
      // The menu has `⌘=` for zoom in; `⌘+` (`⌘⇧=`) reaches the page as a key event (task 8.8).
      let actions = ACTIONS;
      const load = () => invoke<KeyBindings>("keys_get").then((b) => (actions = withBindings(ACTIONS, b)), () => undefined);
      void load();
      const offKeys = await listen(KEYS_EVENT, () => void load());
      const onKey = (e: KeyboardEvent) => {
        const action = recording ? undefined : plusActionFor(e, actions);
        if (!action) return;
        e.preventDefault();
        e.stopPropagation();
        listener(action);
      };
      window.addEventListener("keydown", onKey, true);
      return () => {
        offMenu();
        offKeys();
        window.removeEventListener("keydown", onKey, true);
      };
    },
  };
}
