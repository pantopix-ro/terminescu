# Terminescu

Terminescu is a macOS terminal manager for working in Claude Code all day. Side tabs group your
terminals by ticket. Each side tab has its own browser-style tabs and split panes. It also
remembers what came out of those sessions: variables, pins, links, prompts and the files
Claude edited. When a Claude session waits on a permission prompt, its side tab turns amber,
whichever of your 20 terminals it lives in.

It doesn't orchestrate Claude. You start every session yourself by typing `claude` in a
shell, and Terminescu never starts a turn on its own.

> Status: P1, milestone M1. The spikes are done and the skeleton (tasks 1.1 to 1.4) builds:
> an app window with side tabs and tabs over the core's command and patch protocol, the
> SQLite schema, and the design tokens. The pty daemon (task 2.1) runs shells and restores
> their screens on reattach. The app starts it and gives each tab a shell that outlives the
> app (task 2.6), shown in an xterm.js pane that streams raw bytes from the core (task 2.5).
> The `terminescu` CLI forwards hooks, sets statuses and keeps session and secret variables in
> the daemon, so `var env` answers while the app is closed (task 2.3).

## See it

All links are private claude.ai artifacts. Only the owner can open them until they are shared.

| What | Link |
|---|---|
| Plan page: proposal, 53 tasks, decisions D1 to D36, risks, confidence | https://claude.ai/artifact/NxQVoq2agT798FUijmKxmQ |
| Reel: an 80-second animated walkthrough in 11 scenes | https://claude.ai/artifact/9xWiUJnmsdS2z4aYauTGik |
| Screens: 12 canvas boards (main window, drags, merge, statuses, extract, insert, prompts, hidden context panel, settings, text editor, editor request) | https://claude.ai/artifact/RGGKUzakwC7mZqvNZ3tPpH |
| Workbench: the design system (tokens, 8x8 icons, components) | https://claude.ai/artifact/AhrMAGQJ1CGCzb6JYYBojH |
| UI directions from 2026-11-03, from back when the project was called agent-orch | https://claude.ai/artifact/VjG2fC1eTnc2QTogGHNpmq |

The source of each page lives in this repo under `design/` (see [Design](#design)).

## The model

```
Window            an OS window
└─ Side tab       a row in the sidebar; owns one context
   └─ Tab         browser-style, across the top
      └─ Pane     a split: terminal, transcript or text
```

- **Context, per side tab.** Holds variables, pins, links, text and the files Claude edited.
  Every tab in the side tab shares it. `⌘E` turns a selection into a variable. `⌘⇧P` pins a
  selection or a whole message.
- **Prompts.** A versioned library with `{{variables}}`. `⌘I` inserts a prompt or a variable
  at the cursor and fills it from the context. It never presses Enter for you.
- **Statuses.** Built-in statuses come from Claude Code hooks and shell integration. You can
  rename them, recolor them and add your own. Rules and the `terminescu status` CLI can set
  them too. Amber is reserved for "needs you".
- **Text tabs.** `⌥⌘T` opens a plain-text tab next to your terminals. It's built on
  CodeMirror 6, saves files byte for byte, keeps unsaved work across restarts (hot exit) and
  can act as `$EDITOR`. That means Claude Code's `⌃G` and `git commit` open in a pane
  beside the terminal.
- **Drag like a browser.** Drag a tab to reorder it. Drop it on the sidebar to make a new side
  tab, on a side tab to move it there, or outside the window to open a new window. Drop one
  side tab onto another to merge them.
- **Keys.** Plain keys always reach the terminal. App actions are `⌘` chords, and every one of
  them can be rebound.

## Architecture

```
 Terminescu.app (Tauri 2)                        terminescu-ptyd (detached daemon)
 ┌───────────────────────────────────┐  unix    ┌──────────────────────────────┐
 │ one webview per window            │  socket  │ a pty per pane (login shell) │
 │  React 19, xterm.js, CodeMirror 6 │◀────────▶│ screen model, OSC 133/7      │
 │        ▲ commands  │ patches      │  v1      │ foreground process           │
 │        │  (Host)   ▼              │          │ session + secret variables   │
 │ Rust core: state, SQLite,         │          └──────────────────────────────┘
 │  statuses, rules, documents       │◀── terminescu hook     (Claude Code hooks)
 │  hook socket                      │◀── terminescu status / var / edit (any shell)
 └───────────────────────────────────┘
```

- **The Rust core is the single source of truth.** Windows send commands, and the core
  applies them, persists them to SQLite and broadcasts patches to every window. Pty output
  streams to each window over a binary IPC channel.
- **`terminescu-ptyd` owns every shell.** Sessions survive closing a window, quitting the app
  and upgrading it. A reboot ends them, and the app then restores the layout with fresh
  shells and offers a "resume" button for each Claude session.
- **Claude is the unmodified `claude` CLI.** Terminescu learns about it through Claude Code
  hooks. It never reads or stores a Claude token and never uses the Agent SDK.
- **Every table carries `owner_id`.** v1 has one local owner. A later change, `add-remote-view`,
  serves the same protocol over WebSocket so a browser can view the Mac, with no cloud
  involved.

The decisions behind each choice are numbered D1 to D36 in
[design.md](openspec/changes/add-terminescu-foundation/design.md).

## Plan

Planning follows [OpenSpec](https://github.com/Fission-AI/OpenSpec). One change holds the
foundation:

| File | Contents |
|---|---|
| [proposal.md](openspec/changes/add-terminescu-foundation/proposal.md) | Why, what changes, scope, phases, resolved and open questions |
| [design.md](openspec/changes/add-terminescu-foundation/design.md) | Decisions D1 to D36, risks |
| [tasks.md](openspec/changes/add-terminescu-foundation/tasks.md) | Tasks, each with how it's verified |
| [specs/](openspec/changes/add-terminescu-foundation/specs) | Requirements and scenarios for 9 capabilities |
| [sequence.mmd](openspec/changes/add-terminescu-foundation/sequence.mmd) | The needs-you flow, from Claude Code hook to amber side tab |
| [review](docs/review-2026-11-04.md) | Plan review: issues found, fixes, risks, confidence |
| [spikes](docs/spikes) | Spike notes 0.1 to 0.5: reattach, keys, daemon lifetime, throughput, hooks |

Delivery runs in three phases:

1. **P1, daily use.** Five spikes tested the riskiest assumptions on 2026-11-04 (all go, see
   `docs/spikes/`); the work then lands in four milestones. Each milestone is usable on its own:
   - M1: terminal manager (ptyd, tabs, splits, persistence, statuses from hooks)
   - M2: context and prompts
   - M3: commands, settings and keys
   - M4: text tabs
2. **P2, the browser moves.** Cross-window drag, side tab merge, extraction rules, the full
   transcript pane.
3. **P3, reach.** Signing, notarization, auto-update.

Later changes: `add-remote-view`, `add-flows`, `add-workspaces`, `add-mcp-pin-tool`, and a
rich editor layer.

## Repository layout

| Path | Role |
|---|---|
| `apps/desktop` | Tauri 2 app: windows, sidebar, tabs, panes, context panel |
| `crates/core` | `terminescu-core`: state, SQLite, statuses, documents, hook socket |
| `crates/ptyd` | `terminescu-ptyd`: shells, scrollback, session and secret variables |
| `crates/cli` | `terminescu`: `hook`, `status`, `var`, `edit` |
| `crates/protocol` | The versioned core and daemon protocol |
| `packages/ui` | React components and design tokens |
| `packages/host` | The `Host` interface: Tauri IPC now, WebSocket later; a mock for tests |
| `scripts/sidecars.mjs` | Builds the daemon and the CLI for the app bundle |
| `seeds/prompts` | Seed prompts for the library |
| `design/` | Design system and canvas sources |
| `openspec/` | Change proposals, designs, specs and tasks |
| `docs/` | Reviews and notes that don't belong to one change |

## Design

`design/system` builds the Workbench design system and `design/canvas` holds the screen
boards. Both get published as the artifacts linked above.

    python3 design/system/build_tokens.py
    python3 design/system/build_comps.py project
    PLAYWRIGHT_FROM=/path/to/package.json node design/render/render.mjs design/system/project out/

`PLAYWRIGHT_FROM` points at any `package.json` whose project depends on `playwright-core`.
The `ds/agentorch` folder name is left over from the old project name.

Visual rules: dark only, every color a token, Geist Mono for text and Silkscreen for short
uppercase labels. Frames have hard shadows, icons are 8x8 pixel art and motion moves in steps.
Amber means needs you, focus and drop targets, and it's never offered as a color for your own
statuses.

## Working rules

- Branches start from `develop`.
- Commit messages run 9 lines at most, in the present tense, and never mention AI assistants.
- Seeds, tests, specs and designs use invented data only (`PROJ-482`, `acme/api`). Never put
  client tickets, tokens or real names in the repo.
- Run `openspec validate add-terminescu-foundation --strict` after every change to the plan.

## Requirements (target)

macOS 14 or later, Rust stable, Node 24, pnpm, and Claude Code with a logged-in `claude`.

## Build and run

```
pnpm install
pnpm dev                      # the app with hot reload
cargo test --workspace        # Rust tests
pnpm check:rust               # rustfmt, clippy and Rust tests: run before every commit
pnpm test                     # TypeScript tests
pnpm --filter @terminescu/desktop e2e   # Playwright WebKit on the mock host
pnpm build --debug            # Terminescu.app with both sidecars, in target/debug/bundle
pnpm tokens                   # after editing design/system/build_tokens.py
```

`pnpm --filter @terminescu/desktop dev` serves the UI alone in a browser on a mock `Host`.
