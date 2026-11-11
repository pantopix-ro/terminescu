## Context

add-claude-insights D1 to D7 stand. polish-git-plugin D2 (decorations as a diff) is the model
for D1 here.

## Decisions

### D1. One lead pane per tab

Each followed pane keeps the sequence number of its latest hook event. A publish groups the
panes by tab; the highest number leads and its decorations show. The plugin keeps what it last
set per tab and changes a tab only when its decorations differ. A lead that ends or closes
leaves the tab to the next pane at once. Rejected: merging both panes' chips; two models side by
side read as one session.

### D2. Sections hide themselves

`PanelSectionProps.setHidden?(hidden)` hides the section's head and body; the body stays mounted
so it can show itself again. The setter is stable per section. The Claude sections hide while
the tab has no shell or no Claude session. Rejected: a `visible(tab)` predicate on the section;
only the body knows the answer, after a call.

### D3. The per-file limit lives in a file

`baselineFileMb` (1 to 100, default 5) is a setting. The plugin writes the limit in bytes to
`<support>/baselines/cap` on launch and on each change. The hook reads that file before a copy; a
missing or bad file means 5 MB. `baseText` uses the same limit. Rejected: a hook flag; an older
hook entry would then refuse the whole option list and stop copying.

### D4. Snapshots run no filters

The store lists the repository's filter drivers with a `clean` or `process` command. Each `git
add` of a snapshot gets `-c filter.<name>.clean= -c filter.<name>.process= -c
filter.<name>.required=false`, and so does `write-tree`, which checks racily clean entries
through the filter. The snapshot drops the copied index's entries whose `filter` attribute names
such a driver and whose file is at most the per-file limit, so their content is hashed from the
working tree. A larger one keeps the index's blob, an LFS pointer: hashing gigabytes of assets
per session costs more than their diff. The drop is best effort. Rejected: `--attr-source` with an
empty tree; it still reads `info/attributes` and the global file.

### D5. The tail rests when the session does

A pane is busy from `UserPromptSubmit` or a tool event until `Stop` or `SessionEnd`. The timer
runs every second while any pane is busy, else every 10 s. The state keeps the time the next
tick is due; an earlier tick is stale and returns, and a busy event brings a far tick forward. A
transcript whose length and modified time match the last read is skipped.

### D6. Waiting reasons carry no secrets

`command_label` moves to `terminescu-plugin-api` (`label`); the core re-exports it. A Bash reason
reads `Bash: cargo build`. A URL loses its query and fragment. A Notification of type
`elicitation_dialog` waits with its `message` as the reason, cut to 60 characters, else "Claude
asks a question".

### D7. The session goes with three events

`events()` sends `Session(Some)` only for `SessionStart` and `UserPromptSubmit`, and
`Session(None)` for `SessionEnd`. An app started mid-session learns the session at the next
prompt. The core still ignores a session it already has.

### D8. Contract fixtures

`crates/plugin-claude/tests/contract.rs` calls `settings`, `setSetting` with bad values,
`todos`, `changes` and `baseText` on a real plugin in a temporary repository, replaces the
temporary folder with `/Users/owner/src/shop-api`, and compares with
`packages/plugin-claude/contract/*.json`. `TERMINESCU_BLESS=1` writes them instead. The fake
plugin takes the settings defaults, ranges and messages from the fixture; claude.spec builds its
answers from the fixtures; a unit test reads each fixture through the UI's types. Rejected:
generating TypeScript types from Rust; it adds a tool for four shapes.

## Risks / Trade-offs

- [D4 drops index entries] → a repository with many small filtered files hashes them once per
  snapshot, within the snapshot's 30 s limit. A filtered file over the limit diffs against its
  pointer.
- [D7 later session] → mid-session, resume after a reboot misses a session until the next
  prompt. A session that never got a prompt has nothing to resume.
- [D5 10 s idle] → a transcript change with no hook (a manual edit) shows within 10 s, not 1 s.

## Review fixes

- `url_label` drops userinfo (`user:pass@`) before the path; an `@` in the path stays.
- Waiting reasons for tools other than Bash show a URL (through `url_label`), a file name, or the tool name alone; patterns and MCP values stay out.
- `command_label` keeps a second word only for known subcommand programs (npm, git, cargo, and so on) and for `*.sh` scripts, which keeps the `deploy.sh prod` scenario.
- `Live.busy` clears on `Notification` `idle_prompt` and after 30 unchanged transcript reads with no new busy event.
