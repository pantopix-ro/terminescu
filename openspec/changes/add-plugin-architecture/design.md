# Design: add-plugin-architecture

## Context

See proposal.md for why. Today the runtime holds the Claude session per pane
(`runtime.rs` `hook`, `hook_agent`, `tail_agents`, `transcript_path`, `set_resume`), the pure
state machine in `agent.rs`, the transcript reader in `transcript.rs`, the hook-payload part of
`files.rs`, and the settings.json editing in `hook_install.rs`. `attention.rs` and `status.rs`
read `AgentState`, which is already tool-neutral. The UI reads agents through `Host.agents` and
`onAgent`, transcripts through `readTranscript`, and renders `TranscriptPane` as the
`transcript` leaf kind. The `panes.kind` column accepts `terminal`, `transcript` and `text`.

## Goals / Non-Goals

**Goals:** a seam that lets git and Claude features grow without touching the core; the
existing Claude behaviour unchanged across the move; a build check that keeps the seam.

**Non-Goals:** run-time loading of outside code; a stable public plugin API with versioning;
moving context, prompts, statuses or text tabs into plugins.

## Decisions

### D1. Compile-time plugins, registered in the composition root
Each plugin is a Rust crate and a TypeScript package. `apps/desktop/src-tauri/src/plugins.rs`
and `apps/desktop/src/plugins.tsx` are the only files that name one; cargo features
`plugin-claude` and `plugin-git` (default on) and the `plugins.tsx` list decide what ships.
Rejected: dylibs or WebAssembly loaded at run time. Signing, ABI drift and a sandbox are a
project of their own, and the owner ships the only plugins.

### D2. The core owns a generic agent; adapters are strategies
`AgentState`, `AgentView`, `AgentUpdate` and the attention and status logic stay core
concepts, moved to `terminescu-plugin-api` so both sides share them. An `AgentAdapter`
strategy turns raw hook events and transcript entries into agent changes, names the resume
command, finds a session's transcript, and reports replies and touched files as typed agent
events. The runtime keeps one adapter list and asks each `accepts(event)`.
Rejected: the Claude plugin owning attention and statuses. Every future agent would copy them,
and the amber row would differ per tool.

### D3. A plugin is an observer with calls and a lifecycle
```
trait Plugin: Send + Sync {
    fn manifest(&self) -> Manifest;                        // id, name, description, budget
    fn on_event(&self, ev: &CoreEvent, cx: &PluginCx);     // Observer
    fn call(&self, method: &str, args: Value, cx: &PluginCx) -> Result<Value, PluginError>;
    fn helpers(&self) -> Vec<Helper> { vec![] }            // first-run and uninstall entries
    fn agent_adapter(&self) -> Option<Arc<dyn AgentAdapter>> { None }
}
```
`CoreEvent` covers pane spawned, cwd moved, command started and ended, hook arrived, pane, tab
and row closed, row selected, and app launched. Rejected: plugins calling core internals.
Every such call would be a new coupling.

### D4. `PluginCx` is a facade
The context a plugin gets can read a snapshot of the workspace and a pane's shell state, add
context items and touched files, set and clear its decorations, open a text tab, run a
command line in a new login-shell tab without pressing Enter, read and write its own settings
and per-row data, set named facts on a row or tab that dynamic status rules can test, provide
read-only variables that prompts fill at insert time (a saved variable of the same name wins),
and emit an event to its UI half. Nothing else.

### D5. Isolation: one worker per plugin
Events reach each plugin through a bounded queue (256) on its own thread. A full queue drops
the oldest event. Each handler and call runs under `catch_unwind` with the manifest's budget
(default 500 ms for events, 2 s for calls). Overruns and panics are logged; 5 in a minute
pause the plugin. Rejected: running handlers on the core thread. One slow `git status` would
freeze patches.

### D6. Decorations are ephemeral and keyed
A decoration is `{ plugin, target: side tab | tab | pane, slot: chip | subtitle | badge |
meter, key, text, tone, tip }`. The core keeps them in memory, merges by `(plugin, target,
slot, key)`, and emits `host://decorations` on change. They are never persisted; a plugin
rebuilds them on `AppLaunched`. Rejected: storing them in the workspace patches. They change
every few seconds and would churn the database.

### D7. The UI side: a registry of contributions, composed as decorators
`packages/plugin-api` exports `definePlugin({ id, rowDecorators, tabDecorators, panelSections,
paneViews, actions, settingsPage })`. A row decorator is a function from a row and the
plugin's decorations to render items; the registry composes every enabled plugin's decorators
in plugin order (Decorator). `SideTabList`, `TabBar` and `ContextPanel` take the composed
lists as props and know no plugin. Actions merge into the actions list; a chord already taken
by `actions.json` or the person loses (spec: Plugins fill the app's slots).

### D8. One generic host call
`Host.pluginCall(plugin, method, args)`, `Host.onPluginEvent(plugin, listener)`,
`Host.decorations()` and `Host.onDecorations(listener)` replace per-feature host methods. The
mock host takes fake plugins so e2e tests drive plugin behaviour without Rust.

### D9. Pane views keep their leaf kind
A `transcript` leaf stays a `transcript` leaf in the layout and the `panes` table. The UI looks
the kind up in the pane-view registry; the Claude plugin registers `transcript`. With no
registered view the pane shows the notice in the spec. Rejected: a generic `view` leaf with a
migration. It changes the saved layout for no behaviour gain today.

### D10. Storage: migration `0013_plugins.sql`
`plugin_settings(owner_id, plugin_id, key, value_json, PRIMARY KEY(owner_id, plugin_id, key))`
holds the switches and plugin settings. `plugin_side_tab_data(owner_id, side_tab_id REFERENCES
side_tabs ON DELETE CASCADE, plugin_id, key, value_json, PRIMARY KEY(side_tab_id, plugin_id,
key))` holds per-row facts, such as "this row is a worktree", that die with the row. It
carries `owner_id` like the other context tables, so undo close copies and restores it.

### D11. The seam is checked
`scripts/check-plugins.mjs` (run by `pnpm check:plugins`, part of `pnpm -r test`) fails when
`crates/core`, `packages/ui` or `packages/host` reach a plugin crate or package, through
`cargo metadata` and the TypeScript import graph. CI also runs `cargo check -p
terminescu-desktop --no-default-features` and a Vite build with `VITE_PLUGINS=none`.

## Risks / Trade-offs

- [The move breaks a subtle Claude behaviour: lock order between panes and agents, or the
  waiting tail] → Move code, not logic: the adapter wraps today's `agent::on_hook` and
  `on_entry`; the runtime keeps its locks; every moved unit test moves with it and all 190 e2e
  tests stay unchanged.
- [Events dropped under load hide a state change] → Agent hook events do not go through the
  queue: the adapter runs inline in `Runtime::hook` as today. Only plugin observers are queued.
- [Decorations crowd a narrow row] → The row keeps 6 name characters; extra chips fold into a
  `+n` tip.
- [Two sources of truth for touched files during the move] → The Claude adapter emits
  `TouchedFile` agent events; `files.rs` keeps storage and drops its payload parsing in the
  same commit.

## Migration Plan

Each task keeps the app shippable. The plugin API lands first, then the host, then the Claude
code moves one piece per commit behind the adapter. Rollback is a revert per commit; migration
0013 only adds tables.
