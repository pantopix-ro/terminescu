# Plugins

Terminescu is a terminal manager first. Claude Code support and git come from plugins built
into the app. The core never names a plugin, and the app builds and runs with none. The
decisions behind this are D1 to D11 in
[design.md](../openspec/changes/add-plugin-architecture/design.md).

A plugin has two halves:

| Half | Where | Builds on |
|---|---|---|
| Rust | `crates/plugin-<id>` | `terminescu-plugin-api` |
| UI | `packages/plugin-<id>` | `@terminescu/plugin-api`, `@terminescu/ui` |

Either half can be absent. A Rust half without a UI half still shows its decorations. The
Claude plugin (`crates/plugin-claude`, `packages/plugin-claude`) is the small worked example;
the git plugin, described at the end, uses every slot.

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
off and on in Settings › Plugins. A plugin holds at most 4 calls at once; a fifth fails at once
with `PluginError::Busy` and does not count as a failure. A call that ran over its budget keeps
its place until its thread ends.

A handler that should run later, or once after a burst of events, sets a timer:
`cx.schedule(Duration::from_millis(300), "pane:p1")` sends the plugin `CoreEvent::Timer`
with that token on its own worker. Setting the same token again moves its time, which gives a
debounce. A due timer runs before the queued events. The git plugin reads a pane's repository
this way.

`CoreEvent::SideTabClosed { side_tab, data }` carries the plugin's own row data as it stood
before the close: the save deletes the row's data with the row. It arrives for every close,
the ones a shell exit makes too.

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
  actions: [{ id: "status", label: "Git Status", accel: "Cmd+Alt+S", run: (cx) => cx.bridge.call("status") }],
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
- An action may ask for a `⌘` chord. The person's chords from Settings › Keys come first, then
  `actions.json`, then each plugin's in plugin order. An action that loses its chord keeps none,
  and Settings › Keys says who holds it. An action hidden by its `when` flag still holds its
  chord.
- An action asks through `cx.ask({ title, text, fields, pick, choices })`, which the app draws
  as its own dialog. The first choice answers Enter; Escape answers `null`. `cx.openUrl` opens
  an http or https link in the browser.
- An action with `newMenu: true` also shows in the sidebar's + menu. One with `when: "github"`
  shows only while the plugin's `flags` (a function that gets the bridge and raises flags)
  has that flag up.
- `beforeRowClose(cx)` runs when the person closes a row, after the app's own questions and in
  plugin order (Chain of Responsibility). Return `false` to keep the row open. A row whose last
  shell exits closes without asking.
- `paneBar` draws a bar above each terminal pane. It gets the pane's program and resume offer
  and a facade to resume, dismiss and switch the agent hook. The Claude plugin draws its
  hook-off and resume bars this way.
- `app.openText(path, view)` opens a file drawn by a pane view, such as a diff tab. The document
  keeps the view across restarts; opening the file plainly draws it as text again. A view's
  `app.renderText({ mount, bar, readOnly })` draws the app's text pane; `readOnly` locks it, as
  for a deleted file's diff.
- `watch(cx)` runs in each window while the plugin is on, with the window id, the bridge and
  `ask`, and returns its stop. The git plugin answers its `orphan` event this way.
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

## The git plugin

`crates/plugin-git` and `packages/plugin-git` follow
[add-git-plugin](../openspec/changes/add-git-plugin/design.md), decisions D1 to D7.

- **Providers (Strategy).** `Vcs` reads and changes a repository; `GitCli` runs the person's
  `git`, 2.30 or later, with optional locks off. `Forge` answers pull request questions;
  `GhCli` runs `gh`, which keeps its own login.
- **Layers (Decorator).** The plugin builds `CoalescingVcs(CachedVcs(TimedVcs(GitCli)))`.
  `TimedVcs` kills a read after 1 s, `CachedVcs` reuses facts for 2 s or until `HEAD` or the index
  changes, and `CoalescingVcs` folds a burst of requests for one repository into one follow-up.
  A slow repository delays its chips, never typing.
- **Triggers.** Facts refresh when a pane's cwd moves, a command ends, a tab is selected and at
  launch, debounced 300 ms per pane through `cx.schedule`. Nothing polls the disk. Pull request
  lookups run once a minute per branch at most, on a thread of their own.
- **What it shows.** Tab chips `branch` and `pr`, the Changes section, the `git-diff` pane view
  and the facts `git.pushed`, `git.conflict` and `git.checksFailed` for status rules.
- **Quiet refresh.** A tab's chips and facts change only where they differ. The minute pull
  request tick reads no git and stops on a pane with no branch. The `facts` event carries
  `{ pane, root }`; a diff tab reads its base again only for its own root, in place, so its
  scroll stays.
- **Worktree rows.** A new worktree goes to `<repo>-<branch>` beside the repository, or to
  `<folder>/<repo>/<branch>` when Settings › Plugins › Git names a folder. A bare repository
  (`shop.git`, or a `.bare` folder) counts as the repository folder. The New Row dialog shows
  the target folder and base. The row keeps a mark in its plugin data, and only a marked row
  asks about its worktree when it closes. Keep is the default, and a worktree with changes asks
  a second time. A marked row that closes without asking, as on a shell exit, raises the notice
  "Worktree kept at X. Remove it?" in its window. No branch is ever deleted.
- **Renames and deletes.** A renamed file diffs against its old path at HEAD. A deleted file
  opens read-only with a "Deleted" note.

Test the UI half on the mock host with `?plugins=git`. `window.__fakeGit` answers by pane, root
or row, `window.__fakeGitHub` stands in for `gh` (absent means no `gh`), and
`window.__fakeGitDone` records what the fake added, checked out and removed (`removedAt` for
the orphan notice).
