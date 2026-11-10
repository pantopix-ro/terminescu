# Proposal: add-plugin-architecture

## Why

Claude Code support is woven into the core: the runtime parses hook events, tails transcripts,
types `claude --resume` and installs the hook, in about 30 places. The next two changes add git
and more Claude features. Built the same way, every feature would deepen the core's dependency
on one tool. Terminescu should be a terminal manager that works with no integration at all, and
gains git and Claude awareness from plugins it can switch off.

## What Changes

- A plugin API in Rust and TypeScript. Plugins are compiled into the app and registered in one
  place per side: the Tauri entry point and the app's `main.tsx`. The core, the UI package and
  the host package never import a plugin.
- Plugins observe core events (a pane's cwd moved, a command ended, a hook arrived, a tab or
  row closed), answer calls from their UI half, and push decorations: chips, subtitle parts and
  badges on rows and tabs.
- The UI exposes slots that plugins fill: row and tab decorators, context panel sections, pane
  views, menu and palette actions, a Settings page.
- The core keeps a generic agent model (state, session, waiting since). Agents come from
  agent adapters. Attention, the amber row, statuses and resume work for any adapter.
- The existing Claude Code support moves into a Claude plugin with no change in behaviour:
  hook parsing, the transcript tail, resume after reboot, reply extraction, the files Claude
  edits, hook install and uninstall, the first-run helper entry and the transcript pane.
- Settings › Plugins lists the built-in plugins with a switch each. A disabled plugin gets no
  events and shows nothing.
- A plugin that errors, panics or overruns its time budget is skipped and logged. Typing in a
  terminal never waits on a plugin.
- A check fails the build when the core, the UI package or the host package imports a plugin.

## Capabilities

### New Capabilities

- `plugins`: built-in plugins, their switches, their isolation from the terminal, the slots they
  fill, and the agent adapters that drive attention and statuses.

### Modified Capabilities

None. `claude-awareness` keeps every requirement; the Claude plugin now provides it. Its specs
still live in `add-terminescu-foundation`, which is not archived.

## Impact

- New crates `crates/plugin-api` (`terminescu-plugin-api`) and `crates/plugin-claude`
  (`terminescu-plugin-claude`). New packages `packages/plugin-api` and `packages/plugin-claude`.
- `crates/core`: `agent.rs`, `transcript.rs`, the hook part of `files.rs`, `hook_install.rs`
  and the Claude part of `uninstall.rs` move to the Claude plugin. `runtime.rs` drives agents
  through the adapter. A plugin host and a decorations store join the core.
- SQLite migration `0013_plugins.sql`: `plugin_settings` and `plugin_side_tab_data`.
- Host interface: `pluginCall`, `onPluginEvent`, `decorations`, `onDecorations`. The
  transcript and agent host calls stay as aliases until the Claude plugin's UI uses the plugin
  call, then go.
- Desktop cargo features `plugin-claude` (default on) and later `plugin-git`.
- No core/ptyd protocol change. The `terminescu hook` CLI and its 100 ms contract stay as they
  are.

Out of scope: third-party plugins loaded at run time, owned by a later change if ever. Git
features belong to `add-git-plugin`; new Claude features to `add-claude-insights`.
