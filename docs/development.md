# Development

How Terminescu is built, planned and laid out. To run it from source, start with
[Getting started](getting-started.md). To contribute, read [CONTRIBUTING.md](../CONTRIBUTING.md).

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
- **Claude support and git are plugins.** The core knows a generic agent; the Claude plugin
  turns hook events into agent changes and draws the transcript pane. The git plugin runs the
  person's `git` and `gh` and never reads a GitHub token. The app builds and runs without
  either, and a check keeps the core from reaching a plugin. See
  [docs/plugins.md](plugins.md).
- **Every table carries `owner_id`.** v1 has one local owner. A later change, `add-remote-view`,
  serves the same protocol over WebSocket so a browser can view the Mac, with no cloud
  involved.

The decisions behind each choice are numbered D1 to D36 in
[design.md](../openspec/changes/add-terminescu-foundation/design.md).

## Plan

Planning follows [OpenSpec](https://github.com/Fission-AI/OpenSpec). One change holds the
foundation:

| File | Contents |
|---|---|
| [proposal.md](../openspec/changes/add-terminescu-foundation/proposal.md) | Why, what changes, scope, phases, resolved and open questions |
| [design.md](../openspec/changes/add-terminescu-foundation/design.md) | Decisions D1 to D36, risks |
| [tasks.md](../openspec/changes/add-terminescu-foundation/tasks.md) | Tasks, each with how it's verified |
| [specs/](../openspec/changes/add-terminescu-foundation/specs) | Requirements and scenarios for 9 capabilities |
| [sequence.mmd](../openspec/changes/add-terminescu-foundation/sequence.mmd) | The needs-you flow, from Claude Code hook to amber side tab |
| [review](review-2026-11-04.md) | Plan review: issues found, fixes, risks, confidence |
| [getting started](getting-started.md) | Run from source, build a `.dmg`, install, update, remove |
| [spikes](spikes) | Spike notes 0.1 to 0.5: reattach, keys, daemon lifetime, throughput, hooks |

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
| `crates/plugin-api`, `packages/plugin-api` | The plugin API, Rust and UI halves |
| `crates/plugin-claude`, `packages/plugin-claude` | The Claude plugin: hook adapter, transcript pane, hook helper, context meter, todos, session changes and history |
| `crates/plugin-git`, `packages/plugin-git` | The git plugin: branch and pull request chips, Changes, diff tab, worktree rows |
| `packages/ui` | React components and design tokens |
| `packages/host` | The `Host` interface: Tauri IPC now, WebSocket later; a mock for tests |
| `scripts/sidecars.mjs` | Builds the daemon and the CLI for the app bundle |
| `scripts/check-plugins.mjs` | Fails when the core reaches a plugin (`pnpm check:plugins`) |
| `seeds/prompts` | Seed prompts for the library |
| `design/` | Design system and canvas sources |
| `openspec/` | Change proposals, designs, specs and tasks |
| `docs/` | Reviews and notes that don't belong to one change |

## Design

`design/system` builds the Workbench design system and `design/canvas` holds the screen
boards. The README images come from the reel, `docs/media/README.md` says how to refresh them.

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

## Requirements

macOS 14 or later, Rust stable, Node 24, pnpm, and Claude Code with a logged-in `claude`.
The git plugin needs git 2.30 or later; its GitHub items need a logged-in `gh`.

## Build and run

[Getting started](getting-started.md) has the full steps: tools, development mode, the
`.dmg` build, install, update and removal.

```
pnpm install
pnpm dev                      # the app with hot reload
pnpm build --bundles dmg      # Terminescu.app and a .dmg, in target/release/bundle
pnpm build --debug            # a faster debug bundle, in target/debug/bundle
pnpm check:rust               # rustfmt, clippy and Rust tests: run before every commit
pnpm test                     # TypeScript tests
pnpm --filter @terminescu/desktop e2e   # Playwright on the mock host
pnpm tokens                   # after editing design/system/build_tokens.py
```

`pnpm --filter @terminescu/desktop dev` serves the UI alone in a browser on a mock `Host`.
