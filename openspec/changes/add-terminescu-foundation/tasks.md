# Tasks: add-terminescu-foundation (Terminescu)

P1 lands in four milestones, each usable on its own, after the spikes in group 0:
M1 terminal manager (groups 1 to 5 and 6.6), M2 context and prompts (6.1 to 6.5),
M3 commands, settings and keys (6.7 to 6.10), M4 text tabs (group 7).

## 0. Spikes (P1, before 1.1)

Each spike is throwaway code that answers one question, and its notes land in `docs/spikes/`.
A no-go changes the named decision before group 1 starts.

- [x] 0.1 Reattach: a prototype daemon replays a running `claude` session into a fresh
      xterm.js at the same width and at a different width (D20, D31). Verify:
      `docs/spikes/reattach.md` records both results and picks raw replay or the headless
      screen model.
- [x] 0.2 Keys: a Tauri 2 window with xterm.js and CodeMirror 6 focused in turn. `⌘T`, `⌘W`,
      `⌘K`, `⌘D`, `⌘1`, `⌘I`, `⌘E`, `⌥⌘T` and `⌘\` fire as menu accelerators. Plain keys,
      `⌃` keys and `⌥` as Meta reach the pty (D17). Verify: `docs/spikes/keys.md` holds a table
      of each chord per pane kind.
- [x] 0.3 Daemon lifetime: the app spawns the daemon detached, then quits, then its bundle is
      replaced by a new build. The new app reconnects to the old daemon and finds every shell
      alive (D7, D32). Verify: `docs/spikes/daemon.md` with the steps and the outcome.
- [x] 0.4 Throughput: `cat` of a 50 MB log through daemon, core, IPC channel and xterm.js
      WebGL. Also 8 visible panes over 2 windows (D33). Verify: `docs/spikes/throughput.md`
      records the MB/s, the UI frame time and the WebGL context count.
      Done 2026-11-04; 0.1 picked the headless screen model (D31).
- [x] 0.5 Hooks: record the hook event order for a permission prompt that is approved, one
      that is denied, an idle prompt and a `⌃G` editor request, and save them as test fixtures
      with invented content (D10, D34). Verify: fixtures under `crates/core/tests/fixtures/hooks/`. Done 2026-11-04,
      `docs/spikes/hooks.md`.

## 1. Skeleton (P1)

- [x] 1.1 Replace the Hono, DBOS and Next skeleton with the Tauri layout: `apps/desktop`
      (Tauri 2, React 19, Vite; its crate joins the Cargo workspace), `crates/core`,
      `crates/ptyd`, `crates/cli`, `crates/protocol` (packages prefixed `terminescu-`),
      `packages/ui` (React components, tokens), `packages/host` (the `Host` interface). Remove
      `apps/api`, `apps/web`, `packages/core`, `packages/plugins`, `docker/`,
      `docker-compose.yml`, `.env.example`, `seeds/flows/`. Keep `seeds/templates/*.md` as seed
      prompts under `seeds/prompts/`. Bundle `terminescu-ptyd` and `terminescu` as Tauri
      `externalBin` sidecars built by `scripts/sidecars.mjs`; window with
      `titleBarStyle: Overlay` (D19, D20). Verify: `pnpm install && pnpm tauri build --debug`
      and `cargo test --workspace` pass on a fresh clone.
      Done 2026-11-04; the sidecar list lives in `tauri.bundle.conf.json` so `cargo test` needs no prebuilt binaries.
- [x] 1.2 SQLite schema in migration `0001_init.sql` with `owner_id` on every table, one local
      owner created on first run (D18). Verify: `cargo test -p terminescu-core migrations` applies on an
      empty file and twice without change, and a test asserts every table except `owners` has
      `owner_id`.
      Done 2026-11-04.
- [x] 1.3 Core command and patch protocol behind `Host` (IPC), Zustand store per window, mock
      `Host` for Playwright (D6, D19). Verify: `host.test.ts` applies a `MoveTab` patch in two
      store instances.
      Done 2026-11-04; `fixtures/host-protocol.json` checks the Rust and TypeScript reducers against each other.
- [x] 1.4 Tokens and xterm theme from the design system (D16), Geist Mono and Silkscreen
      bundled. Verify: `tokens.test.ts` asserts the 16 ANSI colors equal the CSS tokens.
      Done 2026-11-04; tokens gained `ansi-0` to `ansi-15`, yellow apart from amber.

## 2. Terminal sessions (P1)

Order: 2.1, 2.6, 2.5, 2.3, 2.2, 2.4.

- [x] 2.1 `terminescu-ptyd`: spawn the login shell per pane with cwd, `TERMINESCU_PANE_ID` and
      the socket variables (D36), Unix socket protocol v1 with a version handshake and a peer uid
      check, headless screen per pane with a snapshot on attach (D31: `alacritty_terminal`
      0.25.1 vendored under `crates/vendor/` with accessors and left out of the workspace
      members, never `swap_alt`), foreground process, `CLAUDE*` stripped from the pane
      environment, exit after 10 idle minutes. The app copies the daemon out of the bundle under
      a hash name and starts it detached with `setsid` (D32). Verify: integration test spawns a
      shell, disconnects, reconnects and reads the snapshot; a test replays the spike 0.1 stream
      and matches a committed snapshot file, which `packages/ui` feeds to `@xterm/headless` and
      compares with the raw stream; unit tests cover a wrapped row ending in blanks, a scroll
      region, the kitty keyboard stack and two attaches in a row with the alternate screen active.
      Done 2026-11-04: `crates/ptyd` (threads, no async runtime), `terminescu_core::launcher`,
      12 snapshot tests with a committed `claude-session.snapshot.bin`, 10 daemon tests, a launcher
      test and `packages/ui/test/snapshot.test.ts`. The app starts the daemon in task 2.6.
- [x] 2.6 Core daemon client and pane lifecycle: on start the app installs the bundled daemon
      and CLI copies (D32, D36), starts the daemon detached and connects with the version
      handshake; the client reconnects when the connection drops and gives fresh login shells to
      tabs whose panes died with the daemon. Each tab owns one daemon pane until splits arrive
      (3.x): a new tab spawns a login shell (cwd from the focused pane once 2.4 reports it, home
      until then), closing a tab closes its pane, a shell that exits closes its tab, and the
      daemon learns the pane's side tab at spawn and on every move (D36). Until layout
      persistence (3.3) a start adopts the daemon's live panes as tabs instead of opening a new
      shell, so restarts never leak shells. Verify: `daemon_client.rs` runs the core against the
      real daemon: a new tab lists a pane with its side tab, a move updates it, closing the tab
      removes the pane, `exit` in a pane closes its tab, a second core adopts the live panes, and
      a dropped connection reconnects.
      Done 2026-11-04: `terminescu_core::{daemon, runtime}`, 5 tests in
      `crates/ptyd/tests/daemon_client.rs` (plus a killed daemon), and a manual run of the app:
      quit leaves the shell running, the next start adopts it. Pane output reaches views in 2.5.
- [x] 2.2 Session store in the daemon for session-only and secret variables, `zeroize`, TTL
      (D21). Verify: integration test sets a secret, restarts the client, reads it, waits past
      the TTL and finds it gone; a test greps the SQLite file for the value and finds none.
      Done 2026-11-04; the store came with 2.3 (`crates/ptyd/src/vars.rs`), this task adds
      `Zeroizing` values and a sweep every 200 ms. Tests: `daemon.rs`
      `a_secret_outlives_its_client_and_expires_after_its_ttl`, `daemon_client.rs`
      `the_app_database_never_holds_a_secret` (scans every file in the app folder, log
      included). Copies made to answer a request are plain strings until the answer is written.
- [x] 2.3 `terminescu` CLI: `hook`, `status`, `var get/set/env`, pane from `TERMINESCU_PANE_ID`;
      `var get` of a secret needs `--reveal` (D35); routing per D36. Verify also: `var env`
      answers with the app closed.
      Verify: `cli.test.rs` runs `terminescu status "Deploying" --for 1s` against a test core.
      Done 2026-11-04; the test lives in `crates/cli/tests/cli.rs`. The app serves `core.sock`
      and answers `status` and saved `var set` with `not_built` until 5.x and 6.1, which the
      CLI turns into a warning. `var unset` and `var list` came along. For 2.4: `var env` only
      exports, so a variable unset or expired since the last prompt needs an `unset` line.
- [x] 2.4 Shell integration for zsh, bash and fish (OSC 133, OSC 7, command line), loaded after
      the person's rc files; `precmd` runs `terminescu var env` when the variable generation
      changed; `VISUAL` and `EDITOR` when the setting is on (D9, D35). Verify: test runs `false`
      in each shell and reads exit code 1, the command line and the cwd, and finds a secret set
      mid-session in the next command's environment but not in the pane's scrollback.
      Done 2026-11-04; scripts in `crates/ptyd/shell/`, written to `<run>/shell` at daemon start;
      zsh through ZDOTDIR, bash 3.2+ through `--rcfile` (reads the login files itself), fish
      through XDG_DATA_DIRS. Hooks run at the first prompt, after every rc file. `var env` also
      runs in `preexec`, so a secret inserted at a waiting prompt reaches that command; it
      prints nothing when `TERMINESCU_VARS_GEN` matches and `unset` lines for names that went.
      Our OSC 133 carries `src=terminescu`; other integrations (iTerm2's) are ignored once it
      reports. Test: `crates/cli/tests/shell.rs`. fish untested here (not installed). The
      `TERMINESCU_EDITOR` switch waits for the setting (6.x) and `edit` (M4).
- [x] 2.5 xterm.js pane, output over a raw-byte `ipc::Channel` batched per frame with
      acknowledgements, WebGL for visible panes only with DOM fallback on context loss, resize,
      copy, links (D33). Verify: manual, `vim` and `htop` render and resize; Playwright opens 20
      panes over 2 windows and every visible pane renders.
      Done: `terminescu_core::views` queues output per view, sends at most 256 KB per view per
      16 ms frame and goes stale past 1 MB unacknowledged; once the window acknowledges what is
      in flight, the view attaches again and the daemon's snapshot redraws it. A tab has one view;
      opening another replaces it. `TerminalPane` keeps every tab of a window mounted, hidden
      ones without WebGL, and at most 12 WebGL panes per window. Links open on `⌘`-click,
      http(s) only. `cargo test` covers snapshot, input, resize, a new tab's late shell and the
      stale resync; Playwright WebKit runs 20 panes over 2 pages on the mock host, keys and
      resize, and links. Manual on 2026-11-04: `vim` and `top` render and follow a window
      resize, and a restarted app shows the same screen (`htop` is not installed here). Copy
      and paste go through the default Edit menu and still need a manual check.

## 3. Windows and tabs (P1)

- [x] 3.1 Side tabs, tabs and splits in the core and UI; single row and group rendering;
      `⌘T`, `⌘N`, `⌘D`, `⌘⇧D`, `⌘W`, `⌘1`-`⌘9`, `⌃⌘1`-`⌃⌘9`. Verify: `layout.test.rs` for the
      split tree; manual for the keys. Done 2026-11-05; tests in `crates/core/tests/layout.rs`
      (11, with the 2x minimum split floor and rounding parity with the host), keys also in
      `e2e/windows.spec.ts`; native menu built from `actions.json`.
- [x] 3.2 Tab reorder and tab to sidebar (new side tab, move into side tab) inside one window.
      Verify: manual, each drop target and Esc cancel; palette "Move tab to…" does the same.
      Done 2026-11-05; dnd-kit, covered by `e2e/windows.spec.ts` (reorder, Esc, end zone,
      into a side tab, palette).
- [x] 3.3 Layout persistence and reattach on start. Verify: quit with 3 side tabs and splits,
      start, every pane shows its live shell. Done 2026-11-05; `persist.rs` tests and the
      `daemon_client` tests; manual check in the test steps.
- [x] 3.4 Reboot recovery: fresh login shells in the last cwd, resume button for Claude tabs
      (D23). Verify: kill the daemon to simulate a reboot; layout returns and nothing runs
      `claude` until the button is pressed. Done 2026-11-05; resume ids pass `is_session_id`
      on load and on Resume; daemon restarts drop the dead session; manual check in the test steps.
- [x] 3.5 Context panel pinned and auto-hide with the hover edge as an overlay; sidebar, panes
      and panel scroll on their own; tab bar scrolls sideways (D26). Verify: Playwright checks
      no pane resize event fires while the panel slides in. Done 2026-11-05; mode kept in
      localStorage, `⌘\` toggles.

## 4. Claude awareness (P1)

- [x] 4.1 `terminescu hook` and its idempotent installer, with the quoted D36 path: events and
      rules of D34, 5 s timeout,
      silent stdout, exit 0 without `TERMINESCU_PANE_ID` or socket. Verify: running the
      installer twice leaves one entry per event and other hooks unchanged; `hook.test.rs` runs
      the hook with no pane id and with no socket and gets exit 0 and empty stdout under 100 ms.
      Done 2026-11-05; `hook_install.rs` (4 tests) runs at every start, single-quotes the path,
      honours `CLAUDE_CONFIG_DIR`; `crates/cli/tests/hook.rs` also covers a stdin that never closes.
- [x] 4.2 Session to pane mapping, agent icon, Waiting set and cleared per D34, `CLAUDE_ENV_FILE`
      line on `SessionStart` (D35). Verify: `waiting.test.rs` replays the spike 0.5 fixtures
      (`sequences.json` with `transcript-denied.jsonl`) and checks each state; manual, a
      permission prompt turns the tab amber, approving clears it and Esc clears it. The denied
      case reads the transcript tail, so 4.2 lands after 4.3. Done 2026-11-05; `agent.rs` is the
      pure state machine, `crates/core/tests/waiting.rs` replays every case; the runtime tails the
      transcript only while Waiting; the hook appends the `var env` line once (`hook.rs` test).
      Hooks can arrive out of order: a late `permission_prompt` or post-`Stop` tool event changes
      nothing (`agent.rs` tests). Known gap: a `claude` killed without `SessionEnd` keeps its
      icon until the next session in that pane; 5.x can clear it when the foreground is a shell.
      Manual amber check in the test steps.
- [x] 4.3 JSONL tail to `Entry` with byte-offset cursor. Verify: `jsonl.test.rs` with a
      recorded transcript fixture including a compaction marker. Done 2026-11-05;
      `crates/core/tests/jsonl.rs` (6 tests, `transcript-compact.jsonl`).
- [x] 4.4 Minimal transcript pane: read-only user and assistant messages, Pin and Extract per
      message and per selection (D25). Verify: manual, pin part of a response and reopen it.
      Done 2026-11-05; `⌘⇧L` opens it (`OpenTranscript`, a pane without a shell), `⌘⇧P` and `⌘E`
      act on the selection, pins re-anchor by quote (`anchor.ts`); extract refuses a name in use.
      Covered by `e2e/transcript.spec.ts`; manual check in the test steps.

## 5. Statuses (P1)

- [x] 5.1 Auto status per tab and side tab (D15). Verify: unit test of the urgency order.
      Done 2026-11-05; `status.rs` computes every status, `ptyd` pushes shell changes with run
      counters to a core that asks (`FEATURE_SHELL`), an older daemon gets a 1 s poll.
- [x] 5.2 Status editor (`⌘⇧S`): label, back to auto, accent, background, save local, make
      global; needs-you marker over any status; sidebar sort. Verify: manual against the canvas.
      Done 2026-11-05; `StatusEditor.tsx`, sort select in the sidebar (kept per window in
      localStorage), `e2e/statuses.spec.ts`; manual check against the canvas in the test steps.
- [x] 5.3 Global statuses in Settings and the rules engine (exit code, Claude idle time),
      expiry, precedence manual > rule > auto (D15). Verify: `rules.test.rs` covers precedence,
      expiry and deleting a status in use. Done 2026-11-05; `crates/core/tests/rules.rs` (7
      tests), `status_store.rs` in SQLite (migration 0003), Settings › Statuses on `⌘,`; the
      full Settings screen comes with 6.9. CLI statuses are not persisted (D15 notes).

## 6. Context and prompts (P1)

- [ ] 6.1 Context panel: variables with storage modes, pins, links, text; `⌘E` extract with
      suggested names, `⌘⇧P` pin of a selection or a whole message with text-quote anchors
      (D12). Verify: `suggest.test.ts` names a GitHub PR URL `pr_<repo>`; `anchor.test.ts`
      re-anchors a pin after its offsets shift. From the group 4 review: an extracted value now
      sits in SQLite in plaintext and shows unmasked; the secret mode masks it and keeps the
      value out of SQLite (D35).
- [ ] 6.2 Insert a secret as `$NAME`, with the transcript warning (D21, D35). Verify: test reads
      only `$API_TOKEN` on the pty, and a Claude Bash command fixture sourcing the
      `CLAUDE_ENV_FILE` line sees the value.
- [ ] 6.3 Prompt library: search, tags, versions, restore, duplicate, archive; seed import.
      Verify: `prompts.test.rs` covers search by tag and body, and restore creating a version.
- [ ] 6.4 `⌘I` insert: prompts and variables, resolution from context, missing variables asked
      inline, bracketed paste, insert and send. Verify: test that plain insert writes no `\r`.
- [ ] 6.5 Save as prompt from the insert dialog with variable detection. Verify: saving text
      containing the `branch` value offers `{{branch}}`.
- [ ] 6.6 Attention: status line count, dock badge, macOS notification on by default without
      sound, chime off by default, `⌘⇧A`. Verify: manual with a waiting session in the background.

- [ ] 6.7 Commands: saved per side tab or global, sidebar section, run or insert, targets (D24).
      Verify: `commands.test.rs` resolves `{{pr_api}}` and run writes `\r`, insert does not.
- [ ] 6.8 Launcher side tabs: run on open and after a reboot, restart, `n/m up` status (D24).
      Verify: manual, kill one process and see `1/2 up` with restart.
- [ ] 6.9 Settings screen: Statuses, Commands, Variable defaults, Extraction rules (read-only),
      Shell with the opt-in `EDITOR` setting, and Keys (D25). Verify: manual against the canvas.
- [ ] 6.10 Editable shortcuts in Settings › Keys with conflict detection; app chords win in
      every pane kind, select-next on `⌃⌘G` (D17). Verify: `keys.test.ts` rebinds "Open file" to
      `⌘P`, flags a clash with `⌘T`, and `⌘D` in a text pane splits.

## 7. Text tabs (P1)

- [ ] 7.1 CodeMirror 6 text pane: Markdown and lazy-loaded languages, soft wrap, find and
      replace, multiple cursors, pane status bar, byte-exact save (D29). Verify: `exact.test.ts`
      opens a CRLF file with trailing spaces, edits one word, saves, and diffs one line.
- [ ] 7.2 Document store in the core: sources (file, Untitled, variable, prompt version, editor
      request), 300 ms change sync, reload on window move, hot exit in SQLite, session-only
      documents never on disk (D28). Verify: `documents.test.rs` restores an unsaved buffer after
      a core restart and finds no row for a session-only document.
- [ ] 7.3 Openers: `⌥⌘T`, `+ ▾ › Text`, `⌘O` file search then the macOS dialog, `⌘`-click on
      paths in terminal output via the OSC 7 cwd, `terminescu edit`, Finder drop, `⌥` to split
      (D27). Verify: Playwright for `⌥⌘T` and `⌘O`; `cli.test.rs` for `terminescu edit` picking
      the caller's side tab; manual for path click and Finder drop.
- [ ] 7.4 Editor requests: `terminescu edit --wait`, split beside the caller, Waiting status,
      `⌘↵` returns, close ends the wait; opt-in `EDITOR` in Settings › Shell (D27). Verify:
      `GIT_EDITOR="terminescu edit --wait" git commit` in a test repo finishes after `⌘↵`.
- [ ] 7.5 Save and close flows: `⌘S`, Save dialog for Untitled, Save As in menu and palette,
      Save / Don't save / Cancel on close, dirty dot (D27). Verify: Playwright with a mock Host
      covers each close choice.
- [ ] 7.6 Disk watcher: reload clean buffers, changed-on-disk bar with Reload and Keep mine,
      20 MB read-only, binary refused (D28). Verify: `watch.test.rs` writes the file under a
      dirty buffer and gets the conflict event, not a reload.
- [ ] 7.7 `{{variable}}` chips, autocomplete from the side tab's context, unknown-name warning,
      secrets as `$NAME` only; pasted images saved next to the file (D29). Verify:
      `varchips.test.ts` flags `{{tikcet}}` and offers `$API_TOKEN` for a secret.
- [ ] 7.8 Send to pane (`⌘⇧↵`) as bracketed paste without Enter. Verify: test reads the
      paste markers and no `\r` on the target pty.
- [ ] 7.9 Prompt library editing in the text editor, `⌘S` creating a version (D14). Verify:
      `prompts.test.rs` saves twice and lists two versions.
- [ ] 7.10 Files section: `PostToolUse` payloads of Write, Edit, MultiEdit, NotebookEdit, paths
      relative to the side tab's cwd, added or modified, one entry per path, click opens a text
      tab (D30). Verify: `files.test.rs` feeds two hook payloads for one path and gets one entry.

## 8. The browser moves (P2)

- [ ] 8.1 Pointer-based drag tracked in the core across windows with a ghost window; drop
      outside every window opens a new window with a default side tab (D20). Verify: manual, drag out, drag back into the
      first window's tab bar.
- [ ] 8.2 Side tab merge with clash dialog and undo toast; items follow their tab on move.
      Verify: `merge.test.rs` for clash, rename and concatenation.
- [ ] 8.3 Extraction rules on entries and terminal output (PR URL, artifact URL, branch).
      Verify: fixture with an artifact URL twice yields one auto link.
- [ ] 8.4 Full transcript pane: tool calls, diffs, thinking; save as prompt from a sent
      message. Verify: manual against a recorded session.
- [ ] 8.5 Send selection to another tab. Verify: manual, text lands in the target without Enter.

## 9. Reach (P3)

- [ ] 9.1 Code signing, notarization, DMG build in CI. Verify: a downloaded DMG opens without a
      Gatekeeper warning.
- [ ] 9.2 Auto-update through the Tauri updater. Verify: an older build updates itself.

The remote view of the Mac (D22) is a later change, `add-remote-view`.
