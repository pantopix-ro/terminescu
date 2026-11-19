## Why

The Links and text section shows raw URLs. Three artifacts on claude.ai read as three strings
of hex, and a PR reads as its path. A pasted link keeps whatever it came with: `user:token@`,
`utm_source`, a trailing slash, or `/pull/28/files` next to `/pull/28`. Two forms of one link
count as two links. The tooltip is the browser's plain `title`, unlike the panel's other
tooltips.

In the Prompts section, each row spends its width on `{{variable}}` names. The name gets cut,
and the tooltip already shows the body with its values.

## What Changes

- **Clean links.** Every link that enters a side tab's context passes one cleaner in the core,
  from any source: paste, terminal, Claude's reply or a plugin. The cleaner:
  - removes `user:password@`, tracking parameters (`utm_*`, `fbclid`, `gclid` and a short fixed
    list);
  - lowercases the host, and drops the default port and a trailing slash;
  - cuts GitHub PR sub-pages and comment anchors back to the PR, and claude.ai artifact URLs to
    their base form;
  - keeps every other query parameter and `#anchor`.
- **Unique on the cleaned form.** Two links are the same link when their cleaned forms match.
  Saved links get the same cleanup once, on the first start after the update, and copies show
  as one row.
- **Link names, best source wins.**
  - An artifact takes its title from the `Artifact` tool call that Claude's `PostToolUse` hook
    already reports, or else from the `<title>` of the HTML file it published.
  - A PR takes its title from `gh`, through the git plugin, while GitHub is on.
  - Any link takes the text of a `[Name](url)` in Claude's reply or in terminal output.
  - A link without a name gets a short label: `PR #28 · terminescu`, `Artifact`, or the host
    and the end of the path.
  - A name from a stronger source replaces a name from a weaker one.
- **Rows show the name.** A small icon sits before each link: PR with its number, artifact, or
  web. The panel tooltip shows the full name and the cleaned URL.
- **Right-click on a link:** Open, Copy link, Copy as Markdown (`[Name](url)`), Remove.
- **PR state badge.** Open, draft, merged or closed, with the checks result, from the git
  plugin's `gh` data. It shows only while GitHub is on.
- **Section rename.** "Links and text" becomes "Links". Captured branch names and typed notes
  move to a small "Notes" group under the links.
- **Prompts.** A row shows only the prompt's name, wrapping to a second line when it is long.
  The `{{variables}}` leave the row, and the tooltip keeps them.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `context-panel`: links get cleaned, named, iconised and badged. Notes split out under Links,
  and prompt rows show only the name. The capability still lives in the unarchived
  `refine-context-panel` change, so its requirements are ADDED here. Archive
  `refine-context-panel` first. This change's "Link rows" requirement then replaces the
  link-display sentence of "Unique links and text".
- `git-integration`: the git plugin names PR links and reports their state.

## Impact

- `crates/core`:
  - new `links.rs` (the cleaner, labels, name ranks);
  - `context.rs` (`add_item` cleans, keeps the best name, and `Item` gains `name`, `label` and
    `link_type`);
  - `extract.rs` (`[Name](url)` text);
  - a one-time cleanup run at start;
  - migration `0017_link_names.sql`;
  - a direct dependency on the `url` crate (2.5.8, already in `Cargo.lock` through Tauri).
- `crates/plugin-claude`: `agent.rs` maps an `Artifact` `PostToolUse` to a new
  `AgentEvent::Link`.
- `crates/plugin-api`:
  - `AgentEvent::Link`;
  - `CoreEvent::LinkAdded`;
  - `PluginCx::describe_link`, with a default no-op, so other plugins build unchanged.
- `crates/plugin-git`: `Forge::pr_at(url)`, a cached PR lookup per link, badges.
- `packages/host`: `Item` fields, the mock.
- `packages/ui`: `ContextPanel.tsx` (link rows, menu, tooltip, Notes, prompt rows),
  `styles.css`.
- `apps/desktop`: the wiring in `main.rs` and `plugins.rs`, e2e `context.spec.ts` and
  `prompts.spec.ts`.
- No change to the core/ptyd protocol. The hook needs no reinstall: `PostToolUse` already fires
  for every tool.

## Out of scope

- Renaming a link by hand. The owner chose names from sources only, on 2026-11-19. A later
  change can add a person-set name as the strongest source.
- Capturing plain web links from replies. Automatic capture stays with PR and artifact URLs;
  other links come in by paste.
- PR badges for GitLab or other forges. The forge Strategy stays GitHub-only.
- A variable count or a "needs input" mark on prompt rows. The owner chose name-only rows.

## Resolved questions (owner, 2026-11-19)

1. Cleaning depth: safe and canonical. Credentials and tracking parameters go, the host is
   normalised, and PR and artifact URLs take their base form. Other query parameters and
   anchors stay. Saved links get the cleanup once, and copies merge.
2. Name sources: every source, best wins, with short labels as the fallback.
3. Prompt rows: full name, wrapping, without variables.
4. Extras: type icons, the right-click menu with Copy as Markdown, the section rename with
   Notes, and the PR state badge.

5. Secret query parameters (`access_token`, `sig`, `X-Amz-Signature` and the like): keep
   them, so signed links keep working. The owner chose this over stripping them, knowing such a
   token then sits in the SQLite file and the tooltip, outside the reach of D35.
6. Plan page: declined. The change folder's files and diagrams are enough.

## Unresolved questions

- None.
