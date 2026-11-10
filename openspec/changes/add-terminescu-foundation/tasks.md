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

- [x] 6.1 Context panel: variables with storage modes, pins, links, text; `⌘E` extract with
      suggested names, `⌘⇧P` pin of a selection or a whole message with text-quote anchors
      (D12). Verify: `suggest.test.ts` names a GitHub PR URL `pr_<repo>`; `anchor.test.ts`
      re-anchors a pin after its offsets shift. From the group 4 review: an extracted value now
      sits in SQLite in plaintext and shows unmasked; the secret mode masks it and keeps the
      value out of SQLite (D35).
      Done: saved variables, links and text live in SQLite (`context_items`, migration 0004);
      session and secret variables live in the daemon per side tab (`SideVar*` messages, feature
      `vars`), and a list blanks secret values. A name is unique across both stores; a secret
      stays a secret. Moving a saved value to the daemon deletes the row with `secure_delete`
      and truncates the WAL; `context_vars.rs` checks no db file holds it after. Closing a side
      tab drops its daemon variables. Pins point at a transcript entry or terminal rows
      (`rowStart`/`rowEnd`). One `ExtractDialog` serves both pane kinds. Limits: daemon values
      do not follow a moved tab yet (group 8), die when the daemon idle-exits with no panes, and
      side tabs closed while the app was down keep theirs until the daemon exits.
- [x] 6.2 Insert a secret as `$NAME`, with the transcript warning (D21, D35). Verify: test reads
      only `$API_TOKEN` on the pty, and a Claude Bash command fixture sourcing the
      `CLAUDE_ENV_FILE` line sees the value.
      Done: each variable row has an insert button that pastes into the focused terminal
      through xterm (bracketed paste when the program asked for it, no Enter): `$NAME` for a
      secret, the value otherwise. Inserting a secret into a Claude pane shows a note that a raw
      value typed into a prompt stays in Claude's transcript. `context.spec.ts` reads the pane
      buffer and finds `$API_TOKEN` and no value; `shell.rs` runs the `SessionStart` hook in a
      real zsh pane, sets the secret afterwards, and a `bash -c` that sources the env file, with
      the variable and its generation stripped as a Claude started earlier would have it, sees
      the value while neither the file nor the scrollback holds it. `⌘I` comes with 6.4.
- [x] 6.3 Prompt library: search, tags, versions, restore, duplicate, archive; seed import.
      Verify: `prompts.test.rs` covers search by tag and body, and restore creating a version.
      Done: `terminescu_core::prompts` keeps immutable versions (migration 0005 adds last used
      and use count); search needs every word, `#tag` a whole tag, other words the name, a tag
      or the body; restore and save add versions, duplicate starts a `-copy` key at v1, archive
      hides. The 8 files in `seeds/prompts` are embedded and imported once per database; a key
      the person has wins, a deleted seed stays deleted. `crates/core/tests/prompts.rs` covers
      it. The library opens with `⌘⇧I` or from the panel's Prompts section. Limits: the body is
      a plain textarea until text tabs land (group 7, D29), and images in prompts wait too.
- [x] 6.4 `⌘I` insert: prompts and variables, resolution from context, missing variables asked
      inline, bracketed paste, insert and send. Verify: test that plain insert writes no `\r`.
      Done: `InsertDialog` lists prompts and variables; a prompt renders `{{name}}` from the
      side tab (a secret as `$NAME`, `{{pins}}` as quotes), asks for missing names inline with
      an opt-in save to context, and stays editable. `⌘↵` inserts, `⌘⇧↵` inserts and sends.
      Inserts skip xterm's paste, which turns line breaks into `\r` outside bracketed mode:
      `pasteBytes` never writes `\r`, brackets any multi-line text, and drops paste markers in
      it. `insert.test.ts` and `prompts.spec.ts` check the exact bytes on the pane.
- [x] 6.5 Save as prompt from the insert dialog with variable detection. Verify: saving text
      containing the `branch` value offers `{{branch}}`.
      Done: one `SavePromptForm` serves the insert dialog's prompt step and a "Save as prompt"
      button on the person's own transcript messages. `detectVariables` finds context values
      of 3 or more characters, longest first, and a secret by its `$NAME`; each becomes a
      ticked `{{name}}` the person can untick, with a live preview. `insert.test.ts` and
      `prompts.spec.ts` cover the `branch` case from both entry points.
- [x] 6.6 Attention: status line count, dock badge, macOS notification on by default without
      sound, chime off by default, `⌘⇧A`. Verify: manual with a waiting session in the background.
      Done: `core::attention` keeps waiting panes and since when, and the settings in `app_state`
      (notify on, chime off). The app feeds agent updates to it on a thread: dock badge, a
      notification "side tab › tab" when no window has focus, the chime event. `⌘⇧A` and the
      amber status line segment ask the core for the oldest wait, which raises its window and
      reveals the pane. `e2e/attention.spec.ts` covers the count, the jump and the chime on the
      mock; the badge and the notification stay manual.

- [x] 6.7 Commands: saved per side tab or global, sidebar section, run or insert, targets (D24).
      Verify: `commands.test.rs` resolves `{{pr_api}}` and run writes `\r`, insert does not.
      Done: `core::commands` stores, resolves and types; the core types into the focused pane
      or the new tab's pane (`Runtime::command_then_type`), so launchers (6.8) reuse it. A secret
      goes in as `$NAME`; a missing name is asked for once and never saved; Run refuses a value
      with a line break, Insert pastes it bracketed. `crates/core/tests/commands.rs`,
      `e2e/commands.spec.ts`.
- [x] 6.8 Launcher side tabs: run on open and after a reboot, restart, `n/m up` status (D24).
      Verify: manual, kill one process and see `1/2 up` with restart.
      Done: migration 0006 adds `commands.tab_id`; a launcher command types into its own tab
      (created on save, reused while it exists) and reruns at startup for panes the daemon
      spawned fresh. A command is up while its tab's auto status is Running, so it needs shell
      integration; a manual status wins over `n/m up`. Restart reruns only the down ones and
      types no Ctrl-C. A daemon reconnect without a reboot reruns nothing. Global launchers are
      refused. `status.rs` unit test, `test/status.test.ts`, `e2e/launchers.spec.ts`.
- [x] 6.9 Settings screen: Statuses, Commands, Variable defaults, Extraction rules (read-only),
      Shell with the opt-in `EDITOR` setting, and Keys (D25). Verify: manual against the canvas.
      Done: `SettingsSheet` (`⌘,`) with a section list: Statuses, Commands (edit name, line,
      target, launcher; delete), Variables (default mode and TTL, which Extract starts with),
      Extraction (read-only), Shell, Keys (read-only until 6.10) and Notifications (notify and
      chime). `core::settings` keeps them in `app_state` (`app.settings`). The `EDITOR` box stays
      disabled until 7.4 adds `terminescu edit --wait`. `tests/settings.rs`,
      `e2e/settings.spec.ts`. The manual canvas check is still owed.
- [x] 6.10 Editable shortcuts in Settings › Keys with conflict detection; app chords win in
      every pane kind, select-next on `⌃⌘G` (D17). Verify: `keys.test.ts` rebinds "Open file" to
      `⌘P`, flags a clash with `⌘T`, and `⌘D` in a text pane splits.
      Done: Settings › Keys records a chord (click, press; Esc cancels) and refuses one without
      ⌘, one macOS keeps (⌘C, ⌘Q…) or another action's, shown inline. `core::keys` stores the
      changed chords in `app_state` (`keys.bindings`), runs the same checks and rebuilds the
      menu; while recording the menu drops its action chords, since macOS hands a menu chord
      to the menu first. Saved chords that stop passing fall back to the defaults at start.
      "Open file" is a test fixture until 7.3 adds it to actions.json; select-next on `⌃⌘G`
      comes with CodeMirror in 7.1. `tests/keys.rs`, `test/keys.test.ts`, `e2e/settings.spec.ts`.

## 7. Text tabs (P1)

- [x] 7.1 CodeMirror 6 text pane: Markdown and lazy-loaded languages, soft wrap, find and
      replace, multiple cursors, pane status bar, byte-exact save (D29). Verify: `exact.test.ts`
      opens a CRLF file with trailing spaces, edits one word, saves, and diffs one line.
      Done: `TextPane` (CodeMirror per pane in the pane registry, lazy `language-data`,
      `lineSeparator` per file, LF/CRLF/Mixed in the status bar); `exact.test.ts`. Next
      occurrence is `⌃⌘G` since `⌘D` splits.
- [x] 7.2 Document store in the core: sources (file, Untitled, variable, prompt version, editor
      request), 300 ms change sync, reload on window move, hot exit in SQLite, session-only
      documents never on disk (D28). Verify: `documents.test.rs` restores an unsaved buffer after
      a core restart and finds no row for a session-only document.
      Done: `documents.rs` with migration 0007 (`base_sig`), 300 ms sync, hot exit rows
      (none for session-only), `on_docs_closed`; `tests/documents.rs`. Editor requests in 7.4.
- [x] 7.3 Openers: `⌥⌘T`, `+ ▾ › Text`, `⌘O` file search then the macOS dialog, `⌘`-click on
      paths in terminal output via the OSC 7 cwd, `terminescu edit`, Finder drop, `⌥` to split
      (D27). Verify: Playwright for `⌥⌘T` and `⌘O`; `cli.test.rs` for `terminescu edit` picking
      the caller's side tab; manual for path click and Finder drop.
      Done: `+ ▾ › Text` in the tab bar, `⌘O` FileFinder (own ranking, top 200, `⌥↵` beside,
      `⌘O` again for the Open dialog), `⌘`/`⌥⌘`-click on paths via an xterm link provider and
      `doc_open_path` (cwd from OSC 7), Finder drop through `onFileDrop`, `terminescu edit` as
      `CoreRequest::Edit`. Path click and Finder drop stay manual checks in the app.
- [x] 7.4 Editor requests: `terminescu edit --wait`, split beside the caller, Waiting status,
      `⌘↵` returns, close ends the wait; opt-in `EDITOR` in Settings › Shell (D27). Verify:
      `GIT_EDITOR="terminescu edit --wait" git commit` in a test repo finishes after `⌘↵`.
      Done: `edit_from_cli` splits beside the caller (new tab when too small), `Waits` block the
      core-call thread until `⌘↵` (`doc_return`) or the last pane closes; the tab shows the
      waiting marker; Settings › Shell sets `TERMINESCU_EDITOR` for new shells. The `git commit`
      check runs by hand in the built app.
- [x] 7.5 Save and close flows: `⌘S`, Save dialog for Untitled, Save As in menu and palette,
      Save / Don't save / Cancel on close, dirty dot (D27). Verify: Playwright with a mock Host
      covers each close choice.
      Done: `saveDoc` and `closeWith` in App.tsx flush the editor first; one prompt for every
      dirty document of a pane or tab; a cancelled Save dialog keeps it open. `e2e/text.spec.ts`.
- [x] 7.6 Disk watcher: reload clean buffers, changed-on-disk bar with Reload and Keep mine,
      20 MB read-only, binary refused (D28). Verify: `watch.test.rs` writes the file under a
      dirty buffer and gets the conflict event, not a reload.
      Done: 1 s poll by size and mtime in `docs.rs`, `host://document` events, the bar in
      `TextPane`; `tests/watch.rs` and `e2e/text.spec.ts`.
- [x] 7.7 `{{variable}}` chips, autocomplete from the side tab's context, unknown-name warning,
      secrets as `$NAME` only; pasted images saved next to the file (D29). Verify:
      `varchips.test.ts` flags `{{tikcet}}` and offers `$API_TOKEN` for a secret.
      Done: `varchips.ts` marks `{{name}}` over the plain text (chip, dashed for a secret, wavy
      underline titled "unknown variable") in Markdown, buffers, prompts and variables; `{{`
      completes names, a secret as `$NAME`. A pasted image goes next to the file as
      `pasted-<ms>.png`; an Untitled buffer holds it until its first save. Tests: varchips, e2e.
- [x] 7.8 Send to pane (`⌘⇧↵`) as bracketed paste without Enter. Verify: test reads the
      paste markers and no `\r` on the target pty.
      Done: `sendToPane` in App.tsx pastes the selection, or the whole text, into the tab's last
      focused terminal (else a Claude pane, else the first, then the side tab's other tabs) via
      `INSERT_EVENT` with `stay`, so the editor keeps focus. `e2e/text.spec.ts` checks the markers
      and that no `\r` reaches the pane.
- [x] 7.9 Prompt library editing in the text editor, `⌘S` creating a version (D14). Verify:
      `prompts.test.rs` saves twice and lists two versions.
      Done: `doc_open_prompt` and `doc_open_variable` (secrets refused, session variables stay
      session only) reuse an open document; "edit in a text tab" in the library and "edit" on
      context variables. `prompts.rs` saves a prompt document twice and lists both versions.
- [x] 7.10 Files section: `PostToolUse` payloads of Write, Edit, MultiEdit, NotebookEdit, paths
      relative to the side tab's cwd, added or modified, one entry per path, click opens a text
      tab (D30). Verify: `files.test.rs` feeds two hook payloads for one path and gets one entry.
      Done: `files::touched` reads the four tools' paths (relative ones join the event cwd); the
      core socket records them per side tab and emits a context change. The panel lists them
      with A or M, opens a text tab on click, forgets on ×. `crates/core/tests/files.rs`, e2e.

## 8. The browser moves (P2)

- [x] 8.1 Pointer-based drag tracked in the core across windows with a ghost window; drop
      outside every window opens a new window with a default side tab (D20). Verify: manual, drag out, drag back into the
      first window's tab bar.
      Done: `drag.rs` tracks the cursor, moves the ghost and sends hover and drop points;
      `MoveTabToNewWindow` plus `WindowCreated`/`WindowClosed` patches open and close native
      windows; palette "Move tab to… New window". state.rs, host.test.ts and windows.spec.ts
      cover the core and the window side; the real drag between windows stays manual.
- [x] 8.2 Side tab merge with clash dialog and undo toast; items follow their tab on move.
      Verify: `merge.test.rs` for clash, rename and concatenation.
      Done: `merge.rs` previews clashes, merges and undoes; `MergeSideTab`/`RestoreSideTab`
      move the tabs; `persist::save` moves a lone tab's items; daemon `SideVarsMove` moves
      session and secret variables. `tests/merge.rs` (5), state and ptyd tests, two Playwright tests.
- [x] 8.3 Extraction rules on entries and terminal output (PR URL, artifact URL, branch).
      Verify: fixture with an artifact URL twice yields one auto link.
      Done: `extract.rs` finds PR links (cut to `/pull/N`), artifact links and branch names in
      git wording; a per-pane `Scanner` reads output by whole lines through escape codes, and
      the `Stop` hook's last reply is read too. Each match is saved once as an `auto` item.
      Hook events now run on one ordered thread so `terminescu hook` never waits on the db.
- [x] 8.4 Full transcript pane: tool calls, diffs, thinking; save as prompt from a sent
      message. Verify: manual against a recorded session.
      Done: `parse_line` returns every entry on a line (thinking, tool call, tool result with
      `structuredPatch` hunks; a denied call gets its failed result). The pane shows them
      collapsed and draws a body only when open; save as prompt was already there (4.4).
      jsonl.rs and transcript.spec.ts cover it; the recorded-session check stays manual.
- [x] 8.5 Send selection to another tab. Verify: manual, text lands in the target without Enter.
      Done: ⌘⌥↵ "Send to Tab…" takes the focused selection (text, transcript or terminal),
      lists the window's other tabs and pastes via `pane_send_text` (bracketed, no Enter);
      Playwright covers it in text.spec.ts, the real-shell check stays manual.
- [x] 8.6 Commands global by default (D24 amendment): the Commands form starts global, "this
      side tab only" opts into the side tab, launcher still forces the side tab; existing
      commands keep their scope. Verify: Playwright saves a command in one side tab and runs it
      from another with that side tab's `{{variable}}`.
      Done: the form starts global; "this side tab only" opts in, and ticking launcher ticks and
      locks it. Saved commands keep their scope. `commands.spec.ts` runs a command saved in one
      side tab from another with that side tab's `pr_api`.

- [x] 8.7 Right-click menus on the sidebar (owner request, 2026-11-05): side tabs (rename in
      place, new tab, edit status, merge into, move to new window, close with a confirmation),
      their tabs (rename, edit status, move to, close) and saved commands (run, insert, restart,
      edit, duplicate, delete). In-app HTML menu. Verify: Playwright opens each menu by mouse
      and keyboard and checks every action against the mock host.
      Done: `ContextMenu` (arrows, Enter, right and left for submenus, Esc, kept inside the
      window); core `RenameSideTab`, `CloseSideTab`, `MoveSideTabToNewWindow` (same id, so the
      context follows); host-protocol.json checks both reducers. `sidebar-menu.spec.ts` (6).
- [x] 8.8 Zoom (owner request, 2026-11-05): `⌘=`/`⌘+` in, `⌘-` out, `⌘0` back to 100%, for
      the whole UI, saved with the settings. `⌘` over `Ctrl` keeps D17. Verify: Playwright steps
      the zoom on the mock host; manual, the terminals refit at each step in the app.
      Done: `zoom_percent` in the settings (50-200, steps as a browser has them); the app sets
      the native webview zoom of every window and of each new one. Three View menu actions,
      editable in Settings › Keys; `⌘⇧=` runs zoom in unless it has its own action.
      `zoom.spec.ts` (3), keys tests.
- [x] 8.9 Sidebar status chip (owner request, 2026-11-05): a click on a side tab's or a tab's
      status chip opens the status editor on it, with this side tab's and the global statuses.
      Verify: Playwright clicks both chips and picks a global status.
      Done: `ChipButton` in `SideTabList` stops the drag and the row select; the editor takes
      `initialTarget`, and an empty global list says so. `sidebar-status.spec.ts` (2).
- [x] 8.10 Add a side tab from the sidebar's empty space (owner request, 2026-11-05): hovering
      the space under the last row shows a `+` that adds one. Verify: Playwright hovers it and
      clicks. Done: `.side-fill` fills the list's free height; the `+` also shows on keyboard
      focus and hides while a tab drags. `sidebar-status.spec.ts` (1).
- [x] 8.11 Sidebar title strip (owner request, 2026-11-05): no header row; the TERMINESCU
      wordmark in the reel's font and colours sits beside the traffic lights; sort moves to the
      empty-space menu and the palette. Verify: Playwright checks the strip, the menu and the
      palette. Done: the 34 pt strip matches the tab bar, `--chrome` undoes the zoom on both, and
      the lights sit at (14, 20). `sidebar-chrome.spec.ts` (3), sort test moved.
- [x] 8.12 Move the window by its header (owner report, 2026-11-05). Cause: the drag regions
      were in the markup, but the capability had only `core:default`, which leaves out
      `core:window:allow-start-dragging`, so each drag was refused. Done: the permission, and the
      tab bar hint is a drag region too.
- [x] 8.13 Startup crash report (owner report, 2026-11-05): two crashes at launch, both a setup
      error that Tauri turns into an abort with no message. Not reproduced since. Done: stderr
      goes to `logs/app.log` when no terminal is attached (every `terminescu:` line and the
      panic text, stamped), and a setup error shows an alert and quits.
- [x] 8.14 Terminal colors (owner request, 2026-11-05, after the colors spike): new shells get
      `CLICOLOR=1` unless `CLICOLOR` or `NO_COLOR` is set, and a status background tints the
      terminal. Verify: daemon test reads the env; Playwright sets a side tab and a tab status.
      Done: the tab's own background wins over its side tab's; `themeFor` recolors the running
      xterm and the pane slot follows in CSS. `terminal-tint.spec.ts` (1). A running daemon
      keeps its env; quit it to apply `CLICOLOR` to new shells.
- [x] 8.15 Rename with a double click (owner request, 2026-11-05) on a side tab's or a tab's
      name, in the sidebar and the tab bar. Verify: Playwright renames all three, Esc keeps.
      Done: the rename state records where it started, so one field opens; a pane focus queued
      by the row click no longer steals the field. `rename-dblclick.spec.ts` (3).
- [x] 8.16 Icon picker (owner request, 2026-11-05): a double click on a side tab's or a tab's
      icon opens 20 predefined icons (`ICONS` in `state.rs` and `protocol.ts`), also under
      "Change icon…" in the menus. Verify: core and persist tests, Playwright picks by mouse,
      arrows and the menu. Done: `SetIcon` / `IconSet`, migration 0008 adds `icon` to
      `side_tabs` and `tabs`, an unknown saved icon drops on load. The tab bar shows a tab's
      icon when one is set or Claude runs. `icon-picker.spec.ts` (3).
- [x] 8.17 Prompt library `+` (owner request, 2026-11-05): hovering the space under the last
      prompt shows a `+` that opens the new prompt form. `prompts.spec.ts` (1 more).
- [x] 8.18 Quick status pick (owner request, 2026-11-05, replaces 8.9's editor): a click on a
      sidebar status chip opens a small menu with Auto, this side tab's and the global
      statuses, the current one checked, and "Edit status…" for the full editor. Done: menu
      items can show a status chip; `sidebar-status.spec.ts` rewritten (3).
- [x] 8.19 Double-click from another app (owner report, 2026-11-05: a double click on an icon
      did nothing). Not reproduced in a focused window; from another app macOS spent the first
      click on bringing the window forward. Done: `accept_first_mouse` on every window.
- [x] 8.20 Each new tab opens with a different random icon (owner request, 2026-11-05).
      Done: `pick_icon` takes the first unused icon from a hash of the tab id (agent left
      out), mirrored in the mock; a one-tab row shows its tab's icon; Claude keeps its state
      dot. Core test, `icon-picker.spec.ts` and `agents.spec.ts` updated.
- [x] 8.21 Default statuses and the quick pick (owner request, 2026-11-05): In progress,
      Review, Testing, Blocked and Done seeded once (`statuses.seeded`); a sidebar chip opens
      `QuickStatus`: the box for a new status takes the cursor, the current, Auto, side tab
      and global statuses follow. Verify: store test, `sidebar-status.spec.ts` (3).
- [ ] 8.22 Neon accent per sidebar row and a 12 px icon set: proposals on the Screens canvas
      v45 (boards RowAccents, IconsHiRes; recommended Edge + icon, 12×12). The neon part is
      built as 8.25; the 12 px icons wait on the owner's pick.
- [x] 8.23 Adjustable sidebar (owner request, 2026-11-05): the right edge drags the width
      (180-480 px, `sidebarWidth` in the settings, double click resets 272); hovering shows a
      pixel button that collapses it to the icons (`sidebarCollapsed`); collapsed icons show a
      pixel tooltip with name and status; a name cut short shows whole over itself. Done:
      settings test, `sidebar-layout.spec.ts` (3), checked in the running app.
- [x] 8.24 SF Mono Regular as the terminal font (owner request, 2026-11-05). Done:
      `terminalFont` starts with `ui-monospace`, WebKit's name for the system SF Mono.
- [x] 8.25 A random neon per new sidebar row (owner request, 2026-11-05), as the Edge + icon
      proposal: eight `--neon-*` tokens (no amber), `pick_neon` takes the first one no row uses
      from a hash of the side tab id, saved in the existing `side_tabs.accent` column; rows from
      before keep the neutral edge. The owner picked the Wash look on 2026-11-05: the neon also
      mixes 12% into the row's background. Verify: core and persist tests, `sidebar-neon.spec.ts`.
- [x] 8.26 A neon per tab (owner request, 2026-11-05): `pick_tab_neon` keeps a side tab's tabs
      apart (`tabs.accent`, migration 0009); the tab's icon in the sidebar and the tab bar and
      its square under the row name take it; every row now shows its squares. `fill_accents`
      gives saved rows and tabs a neon at start. Verify: core tests, `sidebar-neon.spec.ts` (2).
- [x] 8.27 Stacked context panel (owner request, 2026-11-05): Context, Pins and Prompts show one
      under the other with sticky pixel headers; the section tabs are gone and `.ctx-scroll`
      scrolls under the fixed header, pinned or overlaid. Verify: `context.spec.ts` stack test.
- [x] 8.28 Side tab row redesign (owner request, 2026-11-05): a 24 px icon over two lines, the
      status under the name beside the squares, no tab count; `ShellPicker` lists a row's shells
      beside it after a 250 ms rest (groups, or any row when collapsed) and one click selects the
      shell. Verify: `sidebar-layout.spec.ts` (4).
- [x] 8.29 Tinted tabs with icons (owner request, 2026-11-05): a tab bar tab washes 12% of its neon
      into its background, the active one too, and always shows its icon; `fill_accents` gives
      tabs saved before task 8.20 a random icon at start. Verify: core test, `sidebar-neon.spec.ts` (3).
- [x] 8.30 Tab names and subtitles (owner request, 2026-11-06): each new tab gets an animal or
      bird name unique across windows (`tabs.name`, migration 0010; "Otter 2" once all 94 are
      taken), shown when it has no title or document; `fill_accents` names saved tabs. The daemon
      keeps the OSC 0/2 title without its spinner and clears it at each command's start and end;
      the core sends `host://labels` (path from home, program name, Claude's title while Claude
      runs) and the UI shows the focused terminal's subtitle in the tab bar, the one-tab row, the
      group and the shell list. Verify: ptyd and core tests, `labels.test.ts`,
      `subtitles.test.ts`, `tab-names.spec.ts` (3).
- [x] 8.31 Wordmark (owner request, 2026-11-06): ESCU in amber instead of SCU, centred between the
      last traffic light (68 pt) and the sidebar border; the canvas boards' title bars match
      (canvas v47). Verify: `sidebar-chrome.spec.ts`.
- [x] 8.32 Dock a tab beside a pane (owner request, 2026-11-06): a tab dropped on a pane's
      left, right, top or bottom triangle splits that pane 50/50 and its panes take the half
      shown by an amber preview (`DockTab`); the middle 30%, the shown tab and a pane too small
      to halve (red preview) change nothing, and shells, context and pins follow. A pane header
      dragged to the tab bar makes it a tab (`PaneToTab`); the
      palette adds "Split right/down with tab…" (canvas board). Verify: core and persist tests,
      `dock.test.ts` (host, ui), `dock.spec.ts` (8).
- [x] 8.33 Dock follow-ups (owner request, 2026-11-06): while a preview shows, a dashed amber
      outline marks the pane that splits (red when too small, none in the middle); "Move Pane to
      New Tab" (`pane_to_tab`, ⌘⇧B, Shell menu, palette, Settings › Keys) makes the focused pane
      of a split tab a tab right after it. Verify: `dock.spec.ts` (9).
- [x] 8.34 Right-click edits (owner request, 2026-11-06): a right-click (or Shift F10) on a side
      tab, a sidebar tab or a tab bar tab opens the status editor, grown into an edit dialog: name,
      icon, colour from the neons (`SetAccent`), status, then the old menu's actions (merge and
      move as drop-downs). Esc keeps a typed name. The empty list and saved commands keep their
      menus. Verify: core test, `sidebar-menu.spec.ts` (6), `icon-picker.spec.ts`.
- [x] 8.35 Load test and frozen views (2026-11-06): `crates/ptyd/tests/load.rs` (ignored; ten
      panes flood and redraw through the real daemon and core) found a view that overflowed with
      nothing in flight froze for good, as no acknowledgement came to wake it; it now attaches
      again at once. Verify: `views` unit test, the load test by hand.
- [x] 8.36 Settings › Terminal (2026-11-06): font family, size, line height, letter spacing and
      weight for every terminal, and the tab text size, saved with the app settings and applied
      live (xterm.js re-measures, the pane refits, the shell gets its size). Default line height
      drops from 1.38 to 1.15. A split pane's header tooltip shows its whole name. Verify:
      `settings` Rust tests, `terminal-font.spec.ts`.
- [x] 8.37 Terminal.app defaults (2026-11-06): the terminal starts as Terminal.app's Basic
      profile, SF Mono Regular 11 px at line height 1.00 with a blinking bar cursor and no
      bright bold; Option as Meta stays, so Option+Enter sends Esc+Return. Verify:
      `terminal-font.spec.ts`.
- [x] 8.48 Tab tooltips (2026-11-07): hovering a tab in the tab bar shows the sidebar's pixel
      tooltip under it, with the whole name, the path and program, and a set status. It
      replaces the plain title. Verify: `tab-names.spec.ts`.
- [x] 8.38 Find in pane (2026-11-07): `⌘F` opens a find bar over the focused terminal with
      case, word and regex toggles and an "n of m" count; `⌘G`, `⌘⇧G`, `↵` and `⇧↵` step, the
      scrollbar marks the matches, Esc closes and refocuses. In a text pane the chords stay
      the editor's search. Find, Find Next and Find Previous sit in the Edit menu.
      Verify: `find.spec.ts`.
- [x] 8.39 Maximize pane (2026-11-07): `⌘⇧M` or the header's square shows the focused split
      pane alone and brings the split back; a split or a focus move restores it, and so does
      the pane leaving the tab. Session only; the hidden panes park (D33). Verify:
      `maximize.spec.ts`.
- [x] 8.41 Unicode 11 widths (2026-11-07): terminals load `@xterm/addon-unicode11` and measure
      with its table. `crates/ptyd/tests/fixtures/widths.json` holds characters up to Unicode
      11 with their widths; the daemon's grid and headless xterm.js both match it. Emoji newer
      than Unicode 11 stay a known gap, since xterm.js has no newer table. A DEC 2026 block
      split over two writes keeps synchronized output on until its end. Verify:
      `widths.rs`, `widths.test.ts`, `unicode.spec.ts`.
- [x] 8.42 Terminal behaviour settings (2026-11-07): Settings › Terminal sets the cursor (bar,
      block or underline, blink; an unfocused terminal shows an outline), history (1,000 to
      50,000 lines, live, and new shells' daemon copies through `SpawnSpec.scrollback`),
      which Option key is Meta, the bell (off, flash or sound; both bounce the dock icon in the
      background), copy on select, a check before a risky paste (over 5,000 characters, a
      `sudo` line, or a pipe into a shell) and raised contrast. Defaults as Terminal.app.
      Verify: `terminal-behaviour.spec.ts`, `paste.test.ts`, `settings.rs`.
- [x] 8.43 Drop files on a terminal (2026-11-07): files dropped from Finder on a terminal go in
      as shell-quoted, space-joined paths with a trailing space and no Enter; dropped anywhere
      else, the tab bar first, they open in text tabs as before. Verify: `file-drop.spec.ts`,
      `insert.test.ts`.
- [x] 8.40 Long command notification (2026-11-07): the runtime times each command from its
      OSC 133 start to its end; one that ran at least the threshold posts "Side › Tab" with
      "`cargo build` finished in 4m 12s" or "failed with exit 1 after …" while no window has
      focus. A pane running Claude is left to the waiting notification. Settings ›
      Notifications: on by default, 30 seconds (5 to 3,600). Verify: `command_tests`,
      `attention.rs` tests, `settings.spec.ts`.
- [x] 8.44 Clear a terminal (2026-11-07): Edit › Clear to Start `⌘K` (`⌘⇧K` until 8.52) makes the prompt line
      the first line and drops the history; Clear Scrollback `⌥⌘K` drops the history and keeps
      the screen. The daemon clears its copy (`Control::Clear`), so a reattach shows the same; an
      older daemon keeps its lines. Verify: `clear.spec.ts`, ptyd clear tests.
- [x] 8.45 Prompt marks (2026-11-07): the daemon records each prompt (OSC 133 A), output (C)
      and end (D) as rows and replays them in a reattach snapshot. The terminal marks each
      prompt on the scrollbar, red after a failed command; Edit › Previous Prompt `⌘↑` and Next
      Prompt `⌘↓` scroll to them (a text pane keeps both for its editor), and Select Command
      Output `⌘⇧O` selects the output of the prompt jumped to or of the last finished command.
      The scrollbar's ruler gets a width, which the find bar's match marks needed too. Verify:
      `prompt-marks.test.ts`, `prompt-marks.spec.ts`, ptyd mark tests.
- [x] 8.46 Find in all panes (2026-11-07): Edit › Find in All Panes `⌘⇧F` searches every
      terminal of the window, history included, case-insensitive, one hit per line (wrapped rows
      joined), up to 200, grouped by "Side › Tab". A hit selects its tab, focuses the pane and
      selects the line. Verify: `find-all.test.ts`, `find-all.spec.ts`.
- [x] 8.47 Colours and opacity (2026-11-07): Settings › Terminal › Colours picks Terminescu,
      Terminal.app Basic, Solarized Dark or Light, or Imported. "From Terminal.app" reads its
      default profile, "File…" a `.terminal` or `.itermcolors` file (core `term_import`):
      colours, font, size, cursor, Option as Meta and background opacity. A status tint still
      replaces a dark scheme's background and mixes 35% into a light one. Opacity 30 to 100%:
      windows are transparent and the page paints its own background, so below 100% only the
      terminals let the desktop through. Verify: `terminal-theme.spec.ts`, `theme.test.ts`,
      `settings.rs` tests, `term_import` tests, a real-app screenshot.
- [x] 8.49 Terminal links (owner request 2026-11-07): a plain click on a link, path or email
      address copies it and shows "Copied link" in a pixel tip at the pointer; `⌘`-click still
      opens it (D27). A right click opens a pixel menu for the kind: a link Open in Browser and
      Copy; a path Open in Text Tab, Reveal in Finder (`open -R`), Open with Default App and
      Copy; an address Compose in Mail and Copy. The core resolves a path against the pane's
      folder and refuses to open programs and scripts; `open_url` takes one plain `mailto:`
      address. Verify: `links.spec.ts`, `links.test.ts`, `docs.rs` and `main.rs` tests.
- [x] 8.51 Selected tab and row (owner request 2026-11-07): the active tab covers the tab bar's
      bottom line and opens into its pane. A one-tab sidebar row wears its tab's neon, and each
      tab under a group wears its own. The selected row, or in an open group the selected tab
      under it, reaches the panes with no border between them, a stronger wash and an outline
      on its other sides, a 1px line in the group's neon at half strength; the group's header
      and other rows keep their gap. Verify:
      `sidebar-highlight.spec.ts`, `sidebar-neon.spec.ts`.
From the 8.38-8.51 review: a path link opens only a plain folder or a regular file with no
execute bit, after following symlinks, and `open` gets the real path; a longer list of program,
bundle, disk image and web location extensions is refused. A command's notification names the
program and subcommand only. Clear runs through its own parser. Imports read regular files of at
most 4 MB. A plain link click waits out a double click and skips when text got selected; package
versions and git remotes are no email addresses. WebGL restarts when transparency flips, a stale
maximized pane is forgotten, `⌘↑`/`⌘↓` move a field's caret, and find in all panes reads the
buffers once per sheet.
- [x] 8.52 macOS terminal and editor shortcuts (owner request 2026-11-07): `⌘K` is Clear to
      Start, as in Terminal.app and iTerm2, and the palette moves to `⌘P`. `⌘A` is the app's
      Select All: the terminal's whole buffer, the editor's document, or a field's text. A
      terminal sends `⌥←`/`⌥→` as Esc b/f (xterm.js sent `Esc [1;3D`, which zsh does not bind)
      and `⌘←`/`⌘→`/`⌘⌫` as ^A/^E/^U. `⌘⇧K` is free again, so it deletes a line in the editor.
      Verify: `shortcuts.spec.ts`, `terminal-keys.test.ts`, `keys.test.ts`.
- [x] 8.53 Editable terminal chords (owner request 2026-11-07): Settings › Keys lists the
      terminal's chords under "In the terminal": word left and right, line start and end, delete
      to the line's start, and a new line chord that sends Esc CR (`⌥↵` in Claude), off by
      default. Each can be rebound, turned off or reset. A chord needs ⌘, ⌥ or ⌃ (⇧ only with a
      named key such as ↵), and one that an app shortcut or macOS owns is refused or flagged.
      Saved as `terminalKeys` in settings. Verify: `settings.spec.ts`, `terminal-keys.test.ts`,
      `terminal_keys_are_editable`.
- [x] 8.54 Busy icons (owner request 2026-11-07): a tab's icon, in the tab bar, the sidebar row
      and the row's shell list, blinks while a command runs (`mvn`, `npm`) or Claude works, and
      blinks and bounces while Claude waits for an answer. Claude at its prompt stays still, and
      so does `claude` with no hook state; an interrupt (Esc, no `Stop` hook) read from the
      transcript ends Running. A one-tab row blinks with its tab; a group's own icon stays still
      and only its shells blink, or its tab squares while the group is closed. Reduced motion stops both. Verify: `activity.test.ts`,
      `activity.spec.ts`, Rust test `an_interrupt_in_the_transcript_ends_running`.
- [x] 8.55 Pixel context menu (owner request 2026-11-07): a right click that no row, tab or link
      takes opens the app's pixel menu instead of the system's. A terminal gets Copy, Paste,
      Select All, Find, Clear to Start and its pane actions; a text pane Undo, Redo, Cut, Copy,
      Paste, Select All and Find; a field Cut, Copy, Paste and Select All; other selected text
      Copy. Undo to Paste go down the responder chain as the Edit menu's items do (`edit_command`).
      Verify: `context-menu.spec.ts`, `links.spec.ts`, `edit_actions_map_to_the_standard_selectors`.
- [x] 8.56 Claude asks (owner request 2026-11-07): while Claude waits for the person, its tab
      name gets Claude Code's ✳ in front, in the tab bar and the sidebar shell list, and the
      dock icon bounces once (macOS skips it while the app is in front). Verify:
      `activity.spec.ts`; the bounce by hand.
- [x] 8.57 Groups stay as left (owner request 2026-11-07): a group in the sidebar starts open
      and only its chevron opens or closes it; selecting another row no longer folds it, and
      the active group has a chevron too. Verify: `sidebar-layout.spec.ts`, `windows.spec.ts`.
- [x] 8.58 Tabs in the sidebar only (owner request 2026-11-07): the setting `topTabs`, off by
      default, puts the tabs in the top bar too (Settings › Terminal › Tabs); the bar keeps its
      `+`, its menu and the split hint. The mock's `?top-tabs` turns it on for the specs that
      use the top bar. Verify: `sidebar-layout.spec.ts`; core default `top_tabs: false`.
- [x] 8.59 Sidebar controls (owner request 2026-11-07): with `topTabs` off the main pane has no
      top bar. The sidebar's `+` gets a `▾` with Terminal (⌘N) and Text, a new side tab that
      holds an untitled text; a small `+` on a group's bottom line adds a tab to that group.
      The sidebar toggle sits on the sidebar's right line by the wordmark, and a row or group
      tab under the mouse gets the amber line. Verify: `sidebar-layout.spec.ts`.
- [x] 8.60 Group names (owner request 2026-11-08): a tab is named "Terminal" (a text tab keeps
      its document's title) and no longer gets an animal. A side tab gets the animal when it is
      made, unique across windows, and shows it while it holds two tabs or more (migration 11
      moves each row's first tab name over). Verify: `tab-names.spec.ts`, core and host tests.
- [x] 8.61 Sidebar and panel polish (owner request 2026-11-08): a click on the selected group
      row opens or closes it, a click on another group selects and opens it; the selected row's
      path reads in `--dim`; rows in the context panel and the commands list take the amber
      hover line. Verify: `sidebar-layout.spec.ts`.
- [x] 8.62 Live tab names (owner request 2026-11-08): a tab is named as Terminal.app names one,
      after what runs in its focused terminal: `claude` while Claude runs, else the command the
      shell runs, else the pty's foreground process (`zsh` at the prompt), polled every second.
      The subtitle leaves the program out when the name shows it; a renamed group shows its
      animal in its hover list. Verify: `tab-names.spec.ts`, `subtitles.test.ts`.
- [x] 8.63 Opacity fix (owner report 2026-11-08): below 100% three layers each painted the scheme
      at the opacity (70% three times reads solid) and xterm's viewport painted black. Each
      terminal's slot now carries the colour alone. Verify: `terminal-theme.spec.ts` measures
      the coverage under a terminal.
- [x] 8.64 Sidebar opacity (owner request 2026-11-08): the sidebar and the context panel share
      the terminal opacity. Their frames paint at that alpha and the card colours inside turn
      into the light they add over `--bg`, so rows look the same; menus and tooltips stay
      solid. Verify: `terminal-theme.spec.ts` measures both sidebars.
- [x] 8.65 Context panel hover (owner request 2026-11-08): every button, input and link in the
      context panel takes the amber line under the mouse; a button that fills its row leaves it
      to the row. Verify: `context.spec.ts`.
- [x] 8.66 Codebase review (owner request 2026-11-08). Closing a tab or pane that runs a program
      or Claude asks first. An OSC 8 link shows its real address on hover. Enter on a group row
      does what a click does, and → and ← open and close it. The daemon clamps pane sizes to
      1000x500 and never signals a reaped pid. The hook swallows panics and keeps its write in
      budget. Variables cannot take names the shell acts on (`PATH`, `PROMPT_COMMAND`,
      `DYLD_*`). Text saves and the settings.json edit go through a private temp file and a
      rename. A window with no side tab is not restored. A notification names only a plain
      lowercase subcommand. CI runs every gate and the release job waits for it, with secrets
      on the Build step only, actions pinned by commit, and repo, tag and signature checks.
      Verify: `windows.spec.ts`, `mirror.test.ts`, `atomic.rs`, `screen.rs`, `attention.rs`.
- [x] 8.67 Review follow-up (owner request 2026-11-08). Tab and Shift+Tab stay inside an open
      dialog, focus taken behind it comes back, and closing it returns focus to where it was.
      A removed variable leaves the panel at once and is deleted when its Undo toast goes, so
      a secret comes back without the panel holding its value. The bundle id is
      `ro.pantopix.terminescu`. Verify: `a11y.spec.ts`, `context.spec.ts`, `modal.test.ts`.
- [x] 8.68 Review follow-up 2. A removed pin or saved command waits on the same Undo toast as
      a variable. A panic in the daemon's screen model starts that pane's model over at the
      same size and keeps the output flowing; a panic on one client message drops only that
      message. One thread lists the panes once a second for the foreground names and reads
      the cwds from the same list every 5 s. Verify: `context.spec.ts`,
      `sidebar-menu.spec.ts`, `server.rs` tests.
- [x] 8.69 Review follow-up 3. A removal still on its toast is done before a merge or a side
      tab close, so the merge cannot carry it along and the late delete cannot fail. Settings'
      command list deletes with the same Undo. The e2e waits for a slide, a split and a drop
      poll a condition instead of sleeping. `docs/releasing.md` gains a first-run check by
      hand. Verify: `windows.spec.ts`, `settings.spec.ts`, `dock.spec.ts`.
- [x] 8.70 Sidebar drags, part 1. A tab listed under its group drags as a tab of the tab bar
      does: beside a pane, into a row, to a gap, to another window or out to a new one, and
      within its group, where a line shows where it lands. The only tab of a side tab takes
      its side tab along (`MoveSideTab`, new): a gap or a new window moves the side tab with
      its context, and a row, the tab bar or a pane merges it in first. A dock after a merge
      has no Undo. Verify: `sidebar-drag.spec.ts`, core `state` tests.
- [x] 8.71 Rows side by side. A window shows two or more sidebar rows at once, each in its
      own area with its current tab; the window keeps this view, per window, across restarts.
      Shown rows are marked open in the sidebar and the selected one wears the orange focus
      outline. A pane click selects its row; a row picked in the sidebar takes the selected
      area's place. A row, or a row's only tab, dropped on a pane's edge shows beside that
      row with its context, from this window or another, whose emptied window closes. A tab
      of a group leaves it as a new row beside, taking the group's plain variables. A
      window's only row dropped in another window's panes joins it as a row, unmerged. The
      palette shows one row again or hides one. Verify: `split-view.spec.ts`,
      `dock.spec.ts`, `sidebar-drag.spec.ts`, ui `view` tests, core `state` tests.
- [x] 8.73 Area heads. With two rows or more on screen, each area is headed by its row's
      name in the row's neon, the selected one underlined in the focus orange. A group's head
      shows the group mark, the group and the tab it shows; its pixel tip lists the group's
      tabs, the shown one marked. A click on a head selects its row. Verify:
      `split-view.spec.ts`.
- [x] 8.74 Group names in tips and the tab bar. A group's pixel tip lists its tabs one per
      line with each tab's folder, the shown one marked. With top tabs, the bar starts with
      the selected group's mark and name, whose tip lists the same lines. Verify:
      `split-view.spec.ts`, ui `view` tests.
- [x] 8.75 Areas show tabs. Each area of the split shows one tab, so two tabs of one folder
      show side by side and docking never takes a tab out of its folder. A folder header
      dropped on a pane's edge shows all its tabs there. A tab from another window joins as
      its own row beside. A folder with a shown tab is marked open, and so is each shown tab;
      the selected one wears the orange outline. An open row's background reaches the
      sidebar's right edge. An area head shows the tab's coloured icon left of its name.
      Verify: `split-view.spec.ts`, `dock.spec.ts`, `sidebar-drag.spec.ts`, ui `view` tests.
- [x] 8.76 Sidebar drop zones. A tab dragged from this window or another lands where it
      drops: a gap between rows makes it a row, a folder or a row takes it in, a spot between
      a folder's tabs puts it there. The zone under the cursor lights orange. While a tab drags, the gaps between rows grow over the rows' edges, so nothing moves. Verify:
      `sidebar-drag.spec.ts`.
- [x] 8.77 One-line rows. A row with one tab shows as a folder's tab does, one line at its
      height: icon, name, subtitle, and a status chip only when one is set or the row needs
      the person; under the mouse the chip shows, so the quick pick stays one click away.
      Folder rows keep two lines and the squares strip. Verify: `sidebar-layout.spec.ts`,
      `sidebar-status.spec.ts`, `sidebar-neon.spec.ts`.
- [x] 8.78 Folders. A side tab with two tabs or more is a folder in every label, tip and
      class, with a pixel folder icon in the design system. Saved `group` icons become
      `folder` (migration 0012); new tabs never pick the folder icon. Verify: core `db`
      tests, the whole e2e suite.
- [x] 8.79 New terminals at the selected row's level. `⌘T` and the sidebar's `+` add a
      one-tab row right below a selected one-tab row, or a tab to the selected folder; only a
      drag of a row onto a row makes a folder. `⌘N` still adds a row at the end, and the tab
      bar's `+` adds to the tabs it shows. Verify: `windows.spec.ts`, the whole e2e suite.
- [x] 8.80 Text tabs and dropped rows. `⌥⌘T` and the sidebar's `+ ▾ › Text` follow `⌘T`:
      into the selected folder, or a text row right below a one-tab row. A row dropped on a
      row puts its tabs last in the folder, opens the folder and selects the dropped tab. The
      sidebar's `+` reads "New terminal". Verify: `windows.spec.ts`, `text.spec.ts`.
- [x] 8.81 New window. `⌘N` opens a window with one new terminal tab (core `CreateWindow`);
      New Side Tab moves to `⌘⇧N`. A row's edit dialog says "Add tab to this row" or "Add
      tab to this folder", and that tab is selected. Verify: core `create_window` test,
      `windows.spec.ts`, `settings.spec.ts`.
- [x] 8.82 Cascaded windows. A window that a drag did not place opens 28 points right of and
      below the focused window, as Terminal.app cascades, and starts again at the screen's
      top left when it would leave the work area. Verify: desktop `cascade` unit test; manual:
      ⌘N three times in the app.
- [x] 8.83 One-tab rows across windows. A one-tab sidebar row drags like its tab: released
      over another window it joins that window, and its own window closes when emptied; out of
      every window it opens a new one. ⌘T and the sidebar's + still add a row to a window left
      with none. Verify: `windows.spec.ts`; manual: drag a second window's only row back.
- [ ] 8.72 Sidebar drags, part 3. A row dragged between rows reorders (hand-sorted only), and
      a folder row carries all its tabs to another window or out to a new one (8.83 did one-tab
      rows; 8.86 does folders).
- [x] 8.84 Merge all windows. Window › Merge All Windows (⌃⌘M) and the palette move every
      other window's rows to the end of this one, in window order, as rows; the emptied windows
      close. With one window it does nothing and the palette hides it. Verify: core test `merge_windows_moves_every_row`;
      `windows.spec.ts` "merge all windows".
- [ ] 8.85 Window placement on relaunch. Each window saves its frame in points with its screen
      (windows.frame_json), 500 ms after a move or resize, and reopens there; a frame off every
      screen moves into the main screen's work area. A drag's drop point, then the saved frame,
      then the cascade. Verify: desktop unit test `a_saved_frame_fits_the_screens`; core test
      `window_frames_persist`; manual: move two windows, ⌘Q, reopen.
- [ ] 8.86 Folder drag across windows. The core drag carries a row or a tab; a folder row
      dropped on another window's row merges (clash dialog), on a gap, the end or the panes
      moves as a row, and outside every window opens a new window at the cursor. Verify: desktop
      unit test for the drag item; `windows.spec.ts` "a folder drags to another window".
- [ ] 8.87 Undo close. ⇧⌘T reopens the last closed tab or row, from a session-only list of 10:
      names, icons, accents, layout, status, variables, pins and context come back, each pane
      with a fresh login shell in its last cwd; secrets stay in the daemon until the entry
      leaves the list. A tab goes back to its row at its index, else as a new row; a row to its
      window at its index, else the focused window. Verify: core test
      `reopen_restores_a_row_with_its_context`; `windows.spec.ts` "⇧⌘T reopens a tab and a row".
From the group 8 review: the drag tracker holds no lock across a window call (a deadlock with
the main thread), each drag has a token, and a closed source window ends its drag. A new
window opens before an emptied one closes, at the drop point on any screen. Merge undo moves
items before renaming them back (a kept `branch_477` stayed renamed), needs the daemon before
it starts, rolls back when it fails, and runs one at a time; quitting wipes parked secrets.
The merge dialog stays open on an error. A sidechain denial no longer adds a failed result,
prose branch names need a `/`, and a one-line text with a tab goes in bracketed.

## 9. Reach (P3)

- [x] 9.1 Code signing, notarization, DMG build in CI. Dropped (moved to add-public-release).
- [x] 9.2 Auto-update through the Tauri updater. Dropped (moved to add-public-release).

The remote view of the Mac (D22) is a later change, `add-remote-view`.
