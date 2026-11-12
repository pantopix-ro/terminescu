# Getting started

This guide shows how to run Terminescu from the source, how to build a `.dmg` file and how
to install the app for daily use. All steps are for macOS 14 or later on Apple silicon.

## 1. Install the tools

You need these tools once:

| Tool | Version | Install |
|---|---|---|
| Xcode Command Line Tools | current | `xcode-select --install` |
| Rust | stable, 1.97 or later | `curl https://sh.rustup.rs -sSf \| sh` |
| Node.js | 24 or later | `brew install node@24` |
| pnpm | 11.21 | `corepack enable` (the version comes from `package.json`) |
| Claude Code | current | Log in once with `claude` |

Claude Code is optional for the terminal features. You need it for the agent status and the
transcript pane.

## 2. Get the code

```
git clone https://github.com/pantopix-ro/terminescu.git
cd terminescu
pnpm install
```

## 3. Start the app in development mode

```
pnpm dev
```

This command builds the pty daemon and the `terminescu` CLI, starts Vite on port 1420 and
opens the app window. A change to the UI reloads the window. A change to the Rust code
rebuilds and restarts the app.

The first start compiles all the Rust crates. It takes a few minutes. Later starts take
seconds.

To see the UI alone in a browser, without Rust, start the mock host:

```
pnpm --filter @terminescu/desktop dev
```

Then open http://localhost:1420. The mock host has fake shells. Use it for UI work and for
the Playwright tests.

## 4. Build a `.dmg` file

```
pnpm build --bundles dmg
```

The build runs these steps:

1. It builds `terminescu-ptyd` and `terminescu` in release mode and copies them to
   `apps/desktop/src-tauri/binaries`.
2. It builds the UI into `apps/desktop/dist`.
3. It builds the app in release mode and makes the disk image.

The result is `target/release/bundle/dmg/Terminescu_0.1.0_aarch64.dmg`. The app bundle is
also in `target/release/bundle/macos/Terminescu.app`.

A full release build takes about 5 to 10 minutes the first time.

To build only the app bundle, without the disk image, use `pnpm build`. To build a debug
bundle, which builds faster, use `pnpm build --debug`. The debug bundle goes to
`target/debug/bundle`.

## 5. Install the app

1. Open the `.dmg` file in Finder.
2. Drag `Terminescu` to `Applications`.
3. Eject the disk image.
4. Open Terminescu from `Applications` or from Spotlight.

The app is not signed and not notarized (add-public-release). A `.dmg` that you build on your own
Mac does not get the quarantine flag, so macOS opens it with no warning.

If you copy the `.dmg` to a different Mac, macOS blocks the first start. Do one of these on
that Mac:

- Right-click `Terminescu` in `Applications`, select **Open**, then select **Open** again.
- Or remove the flag in a terminal:
  `xattr -dr com.apple.quarantine /Applications/Terminescu.app`

## 6. What the first start does

On each start the app does these steps:

1. It copies the daemon and the CLI to `~/Library/Application Support/Terminescu/bin`. The
   copies stay at the same path when you update the app.
2. It adds `terminescu hook` to the hooks in `~/.claude/settings.json`, once you said yes on
   the first-run screen or in Settings › Shell. An entry already there counts as yes. It keeps
   all your other settings. If `CLAUDE_CONFIG_DIR` is set, it uses that folder.
3. It starts `terminescu-ptyd`, which keeps your shells. The daemon continues to run when you
   quit the app, so your shells stay alive.
4. It opens the database `~/Library/Application Support/ro.pantopix.terminescu/terminescu.sqlite`.

New tabs start your login shell. Type `claude` in a tab to start a Claude session.

## 7. Update the app

1. Build a new `.dmg` (step 4).
2. Quit Terminescu with `⌘Q`. Your shells continue to run in the daemon.
3. Drag the new `Terminescu` to `Applications` and replace the old one.
4. Open Terminescu. The tabs attach to the same shells.

## 8. The development build and the installed app

The development build and the installed app use the same database, the same daemon and the
same Claude hook. Do not run the two at the same time.

Each start points the Claude hook at the CLI of the build that started. After you use
`pnpm dev`, open the installed app again. This points the hook back to the installed CLI.

## 9. Remove the app

1. Quit Terminescu.
2. Stop the daemon: `pkill -f 'Terminescu/bin/ptyd-'`. This closes all your Terminescu
   shells.
3. Delete `/Applications/Terminescu.app`.
4. Delete `~/Library/Application Support/Terminescu`.
5. Delete `~/Library/Application Support/ro.pantopix.terminescu`.
6. In `~/.claude/settings.json`, remove each hook whose command ends in `terminescu' hook`.

## Troubleshooting

| Problem | What to do |
|---|---|
| The Dock shows the old icon after an update | Run `killall Dock`. |
| A tab shows no agent status for Claude | Look for `terminescu hook` in `~/.claude/settings.json`, then start a new `claude` session. |
| `pnpm dev` fails on port 1420 | Stop the other Vite server that uses the port. |
| The build fails in `sidecars` | Run `cargo build -p terminescu-ptyd -p terminescu-cli` to see the Rust error. |
