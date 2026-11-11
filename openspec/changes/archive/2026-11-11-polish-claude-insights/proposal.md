## Why

The review of add-claude-insights left nine rough edges. Two Claude panes in one tab overwrite
each other's chips. The plan and change sections say "No Claude session" in every plain shell
tab. The 5 MB per-file limit is fixed. The snapshot runs clean filters, so git-lfs writes objects
into the repository and the baseline holds pointers. The tail timer reads every transcript once a
second, idle or not. A waiting notification can show a whole Bash command line, secrets
included, and a question from Claude shows no text. Every hook event sends the session again. A
dead CSS rule stays. The e2e fakes copy answer shapes by hand and can drift from Rust.

## What Changes

- The pane in a tab with the latest hook event leads its tab's decorations; when it ends, the
  other pane's come back.
- A panel section can hide itself through a new `setHidden` prop. The Claude sections hide in a
  tab with no Claude session.
- Settings › Claude gains "Largest file kept"; the hook reads it from `<support>/baselines/cap`.
  The other two labels read "Keep baselines for" and "Total baseline storage".
- The snapshot turns off clean and process filters, and hashes filtered files from the working
  tree.
- The tail timer ticks every second while a session works and every 10 s otherwise, and skips a
  transcript whose size and time did not change.
- A waiting reason names a Bash command by its program and subcommand only, and drops a URL's
  query. A question shows its own text.
- The agent session goes to the core only on `SessionStart`, `UserPromptSubmit` and
  `SessionEnd`.
- The `.tr-extract` rule goes.
- Rust tests write the plugin's answers as JSON fixtures; the e2e fake and tests read them.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `claude-insights`: lead pane, hidden sections, per-file limit, filter-free snapshots, idle tail,
  safe waiting reasons.

## Impact

- `crates/plugin-claude`, `crates/plugin-api` (`baseline` cap file, `label`), `crates/cli` hook,
  `crates/core` attention (re-export).
- `packages/plugin-api` (`setHidden`), `packages/ui` ContextPanel, `packages/plugin-claude`,
  `apps/desktop` App, fake plugins, CSS, e2e.
- No migration. An older hook ignores the cap file and keeps 5 MB.
