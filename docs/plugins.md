# Plugins

Terminescu is a terminal manager first. Claude Code support, and later git, comes from plugins
built into the app. The core never names a plugin, and the app builds and runs with none. The
decisions behind this are D1 to D11 in
[design.md](../openspec/changes/add-plugin-architecture/design.md).

A plugin has two halves:

| Half | Where | Builds on |
|---|---|---|
| Rust | `crates/plugin-<id>` | `terminescu-plugin-api` |
| UI | `packages/plugin-<id>` | `@terminescu/plugin-api`, `@terminescu/ui` |

Either half can be absent. A Rust half without a UI half still shows its decorations. The
Claude plugin (`crates/plugin-claude`, `packages/plugin-claude`) is the worked example.

## The Rust half

Implement `Plugin` from `terminescu-plugin-api`:

```rust
pub struct GitPlugin;

impl Plugin for GitPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::new("git", "Git", "Branch and changes on each row.")
    }

    // Observer: every core event, on this plugin's own worker thread.
    fn on_event(&self, event: &CoreEvent, cx: &dyn PluginCx) {
        if let CoreEvent::CwdMoved { pane, cwd } = event {
            // read the branch, then cx.set_decoration(...)
        }
    }

    // A call from the UI half: bridge.call("status", args).
    fn call(&self, method: &str, args: Value, cx: &dyn PluginCx) -> Result<Value, PluginError> {
        match method {
            "status" => Ok(json!({ "dirty": false })),
            _ => Err(PluginError::UnknownMethod(method.into())),
        }
    }
}
```

The plugin acts on the app through `PluginCx` only. It reads panes and the workspace, adds
context items and touched files, sets decorations, opens text tabs, types a line into a new
login-shell tab without pressing Enter, keeps settings and per-row data, sets facts for status
rules, provides prompt variables and emits events to its UI half. Each of these is scoped to
the calling plugin.

Two optional parts:

- `helpers()`: entries the first-run screen offers and Uninstall Helpers removes, such as the
  Claude Code hook. A helper implements `Helper` (`installed`, `install`, `uninstall`).
- `agent_adapter()`: a strategy for one coding agent. It turns raw hook events into agent
  changes and typed events (a reply, a touched file), names the resume command and finds the
  session's log. The core keeps the agent state and the amber row for every adapter.

### Budgets and failures

Each plugin runs on its own thread with a queue of 256 events; a full queue drops the oldest.
An event handler gets 500 ms and a call gets 2 s, unless the manifest sets other budgets. A
panic or an overrun is logged. Five in one minute pause the plugin until the person switches it
off and on in Settings › Plugins.

A handler that should run later, or once after a burst of events, sets a timer:
`cx.schedule(Duration::from_millis(300), "pane:p1")` sends the plugin `CoreEvent::Timer`
with that token on its own worker. Setting the same token again moves its time, which gives a
debounce. The git plugin reads a pane's repository this way.

Decorations live in memory only. Rebuild them on `CoreEvent::AppLaunched`, which also arrives
when the plugin is switched on.

## The UI half

Export a `definePlugin` value from the package's `src/index.tsx`:

```tsx
export const git = definePlugin({
  id: "git", // the same id as the manifest
  name: "Git",
  rowDecorators: [(items, row, mine) => [...items, ...mine.map(toItem)]],
  panelSections: [{ id: "changes", title: "Changes", render: Changes }],
  actions: [{ id: "status", label: "Git Status", accel: "Cmd+Alt+G", run: (cx) => cx.bridge.call("status") }],
});
```

The registry composes every enabled plugin in plugin order. Row and tab decorators run in a
chain (Decorator): each one gets the items so far and returns the new list. Panel sections,
pane views, actions and the settings page are slots the app fills.

- Talk to the Rust half through the `bridge`: `bridge.call(method, args)` and
  `bridge.on(event, listener)`.
- A pane view draws one leaf kind. The view gets the leaf's `source` and a `PaneApp` facade for
  pins, extract, prompts and focus. With no view for the kind, the pane keeps its place and
  names the plugin that draws it.
- Every action needs a `⌘` chord. A chord that `actions.json` or the person already uses wins,
  and the plugin action keeps no chord.
- Mark the package `"sideEffects": false`, so a build without the plugin drops its code and its
  CSS.

## Register a plugin

Two files name plugins, and only these two:

1. `apps/desktop/src-tauri/src/plugins.rs`, function `registered()`: add the Rust half behind
   a cargo feature. Add the feature to `apps/desktop/src-tauri/Cargo.toml` with the crate as an
   optional dependency, and add it to `default`.
2. `apps/desktop/src/plugins.tsx`, list `BUILT_IN`: add the UI half. Add the package to
   `apps/desktop/package.json`.

Keep the two halves in step. A UI half whose Rust half is not built is left out, since the
app lists plugins from the Rust side. A Rust half with no UI half shows its decorations, and its
panes show the "needs a plugin" notice.

The order of the lists is the plugin order. The Rust order sets which adapter answers a hook
event first; the UI order sets the decorator chain.

To test the UI half without Rust, give the mock host a `FakePlugin` that answers its calls
(`createMockHost(undefined, { plugins: [...] })`). The mock plays the Claude plugin by default;
`claude: false` leaves it out.

## Check the seam

```sh
pnpm check:plugins
```

The script fails when `terminescu-core`, `terminescu-protocol`, `terminescu-ptyd` or
`terminescu-cli` depends on a plugin crate, when `packages/ui`, `packages/host` or
`packages/plugin-api` imports a plugin package, or when a plugin is named outside the two
registry files. It also runs in the `packages/plugin-api` tests.

CI builds the app with no plugins, and with each plugin alone, too. Run the same builds before
a pull request:

```sh
cargo clippy -p terminescu-desktop --no-default-features -- -D warnings
cargo check -p terminescu-desktop --no-default-features --features plugin-claude
cargo check -p terminescu-desktop --no-default-features --features plugin-git
VITE_PLUGINS=none pnpm --filter @terminescu/desktop build
```
