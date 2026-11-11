# Tasks: add-claude-insights

## 1. Baselines

- [x] 1.1 Working-tree snapshot in Terminescu's object store at `SessionStart`, `ready`
      marker, 30-second stop. Verify: `cargo test -p terminescu-plugin-claude`
      `snapshot_leaves_the_repo_untouched` (refs, index, objects and `git status` unchanged),
      `snapshot_includes_untracked_not_ignored`.
- [x] 1.2 `PreToolUse` entry for edit tools in `hook_install`; first-touch `clonefile` copy in
      `terminescu hook` with the size cap and marker. Verify: CLI tests
      `first_touch_copies_once`, `a_large_file_gets_a_marker`, `hook_stays_under_100_ms`;
      moved `hook_install` tests updated. Done as `baseline` tests in `terminescu-plugin-api`
      and `first_touch_copies_once_and_hook_stays_under_100_ms` in the CLI (D3 note).
- [x] 1.3 Session changes and base text: per-file copy wins over the tree snapshot. Verify:
      plugin tests for a git session (Edit plus a formatter) and a non-git session (Edit plus
      `sed`).
- [x] 1.4 Retention: 7 days after session end, 500 MB cap oldest first, Uninstall Helpers
      removes `baselines/`. Verify: tests `old_baselines_go`, `the_cap_spares_running_sessions`.

## 2. Transcript and hook signals

- [x] 2.1 Usage and model per session from the transcript; meter decoration; window per model
      setting. Verify: plugin test over a fixture transcript; `e2e/claude.spec.ts` "the meter
      turns amber at 80%".
- [x] 2.2 Todo mirror panel section. Verify: fixture test; `e2e/claude.spec.ts` "the todo list
      shows Claude's plan".
- [x] 2.3 Waiting reason in notifications and the amber tip. Verify: plugin test over
      `PermissionRequest` fixtures; `e2e/attention.spec.ts` tip text.
- [x] 2.4 Model and mode chips, compacting subtitle, subagent count; hook events `PreCompact`,
      `SubagentStart`, `SubagentStop`. Verify: plugin tests; `e2e/claude.spec.ts` chips.
      Done: `insights` unit tests, `a_fixture_transcript_fills_the_meter_chips_and_todos`,
      `a_permission_request_names_what_it_waits_on`; the e2e play the Rust half (D4 note).

## 3. History and files

- [x] 3.1 Session history in row data, "Resume…" that types without Enter. Verify: plugin test
      `history_keeps_50_newest`; `e2e/claude.spec.ts` "resume types the command and no Enter".
- [x] 3.2 "What Claude changed" panel section and diff tabs through the git plugin's diff view
      (plain text view when git is off). Verify: `e2e/claude.spec.ts` "a session's changes open
      as diffs".
      Done: the diff view moved to `@terminescu/ui` as `DiffPane` (D5a), so the diff works with
      the git plugin off and no plain text fallback is needed.
- [x] 3.3 Open `CLAUDE.md`, `~/.claude/CLAUDE.md` and the transcript folder. Verify:
      `e2e/claude.spec.ts` "a missing CLAUDE.md opens unsaved".
- [x] 3.4 Settings › Claude: threshold, windows, cap, retention. Verify: `e2e/settings.spec.ts`.

## 4. Gate

- [x] 4.1 Full gate. Verify: `pnpm check:rust`, `pnpm -r test`, all e2e, `pnpm check:plugins`;
      manual: a real session in a repository shows its meter, todos and changes, and
      `git status` shows nothing Terminescu wrote.
      Automated part passes (222 e2e); the manual check waits for the owner.
      2026-11-11: CI passes the automated part on develop 66b0786; owner confirmed the manual checks.
- [x] 4.2 README feature lines, `PRIVACY.md` baseline paragraph. Verify:
      `openspec validate add-claude-insights --strict`.
