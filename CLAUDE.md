# Claude Code Preferences

## Working style
- Ask for clarification when a request is vague, before starting work.
- Suggest improvements after completing a task.
- Follow the conventions already in the code and reuse the libraries already present.

## Stack
Terminescu, a macOS desktop app: Tauri 2 (`apps/desktop`, React 19, Vite, xterm.js), Rust crates
in `crates/` (packages prefixed `terminescu-`): `core` (state, SQLite), `ptyd` (`terminescu-ptyd`,
pty daemon), `cli` (`terminescu`: hook, edit, status, var) and `protocol`,
`packages/ui` and `packages/host`. The current Hono, DBOS and Next skeleton is replaced by
task 1.1 of `add-terminescu-foundation`. Project facts for planning live in `openspec/config.yaml`.

## Hard rules
- Claude runs as the unmodified `claude` CLI, started by the person. Never add the Agent SDK
  npm package, never read or store a Claude OAuth token.
- Plain keys always reach the focused terminal; app actions use `⌘` chords.
- New tabs start the login shell, never `claude`.

## OpenSpec
Plan with the `/opsx:*` skills: `/opsx:explore` to think, `/opsx:propose` to write a change,
`/opsx:apply` to implement, `/opsx:verify` before a PR, `/opsx:archive` after merge. Wait for
review of the proposal before applying. Run `openspec validate <id> --strict` before pushing.

## Plans
- Make the plan extremely concise. Sacrifice grammar for concision.
- End each plan with the unresolved questions, if any, and propose improvements.
- When entering plan mode, ask: multi-phase plan? OpenSpec change? git worktree?

## Branches and commits
- Implement on a feature branch started from `develop`; rebase on `develop` before pushing.
- Commit messages: at most 9 lines. Never mention Claude, AI assistants, model names or
  Co-Authored-By.
- Never put client ticket contents, tokens or real names in the repo.
- `terminescu hook` always exits 0, prints nothing to stdout and returns within 100 ms. It runs
  in every Claude session on the Mac (D34).
- Secret values never get typed into a pane or written to disk (D35).
