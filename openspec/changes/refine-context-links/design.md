## Context

`context::add_item` (`crates/core/src/context.rs`) handles every link from three sources:

- `extract::record`, for terminal output and Claude's `Stop` reply;
- the `add_item` command, for a paste;
- `PluginCx::add_context_item`, for a plugin.

It trims the value and checks the scheme, and it deduplicates on the exact string.
`extract::link_at` already cuts PR and artifact URLs to a base form, but a paste skips that
step. `context_items.name` exists and is NULL for links. The unique index on `name` covers
variables only.

Claude's hook already sends `PostToolUse` for every tool (`hook.rs` `EVENTS`), so an
`Artifact` call reaches `plugin-claude/agent.rs::events` today. The function drops it there.
The git plugin's `Forge` (`forge.rs`) runs `gh pr list` per branch, and it knows the title,
the state and the checks.

## Goals / Non-Goals

**Goals:**
- One cleaner for every link source.
- Names from tools and replies, with a fixed rank.
- PR state on the row.
- Notes split from Links.
- Prompt rows that show the name only.

**Non-Goals:**
- Renaming a link by hand.
- Capturing plain web links automatically.
- Forges other than GitHub.

## Decisions

### D1. One cleaner in the core, `links::clean`
`crates/core/src/links.rs` parses the URL with the `url` crate (WHATWG). That gives the
lowercase host, IDNA, the default port and percent-encoding. Then it:

- drops the username and password;
- drops the tracking parameters (D2);
- drops a trailing `/` from a non-root path;
- applies the host rules that `extract::link_at` has today (GitHub PR, claude.ai artifact),
  and lowercases a PR's owner and repo, since GitHub treats them without case;
- drops GitHub comment anchors (`#issuecomment-`, `#discussion_`, `#pullrequestreview-`) and
  diff anchors on a PR.

`add_item` calls it for `ItemKind::Link`. `link_at` keeps finding URLs in text, and hands them
to `clean`.

*Rejected:* cleaning in the UI's add box. Rejected because the terminal, Claude's reply and
plugins would skip it.

### D2. Tracking parameters are a fixed list; secret parameters stay
The cleaner drops `utm_*`, `fbclid`, `gclid`, `dclid`, `msclkid`, `mc_cid`, `mc_eid`,
`igshid`, `_hsenc` and `_hsmi`, matched without case.

Query parameters that carry a secret (`access_token`, `sig`, `X-Amz-Signature`) stay, so a
signed link keeps working (owner, 2026-11-19). Credentials in the `user:password@` part still
go.

*Rejected:* stripping secret parameters. The owner rejected it because signed links would
break.

*Rejected:* entropy guessing. Rejected because it gives false positives on IDs like
`?id=8f3c…`.

### D3. Deduplication on the cleaned value; the first row stays
`add_item` cleans the value, then looks for the same kind and value in the side tab. A match
returns the existing row. Its name, and the name of every copy with that value in the side
tab, is upgraded per D5, so whichever copy `items()` shows carries the best name. `items()`
keeps its read-time deduplication for merged tabs.

*Rejected:* a unique index on `(side_tab_id, kind, value)`. Rejected because merges and moved
tabs bring copies on purpose, and `remove_item` removes them together (refine-context-panel).

### D4. Migration `0017_link_names.sql` and a one-time cleanup
The migration adds `context_items.name_rank INTEGER NOT NULL DEFAULT 0`. The rank values are:

| Rank | Source |
|---|---|
| 0 | none |
| 1 | text |
| 2 | tool |

Cleaning needs the URL parser, so it runs in Rust at start, guarded by `app_state` key
`links.cleaned.1`, as `prompts.seeded.2` does. The cleanup:

1. Cleans every link row and updates `value`. A row that no longer parses keeps its value.
2. Within each side tab, gives every copy of one cleaned value the best name among them.
3. Deletes nothing. Copies exist on purpose: an auto link carries `origin_tab_id`, and
   `persist::move_tab_context` moves it with its tab. `items()` already shows one row per
   value, and `remove_item` removes all copies.

All of it runs in one transaction.

*Rejected:* an SQL-only migration. Rejected because SQLite has no URL parser, and string
`REPLACE` misses ports, case and parameter order.

### D5. Name ranks: a name from the same or a stronger source wins
`add_item` gains `name: Option<(&str, NameRank)>`. A new name is written when its rank is at
least the stored rank. Within one rank, the newest name wins, so a PR rename shows up. A name
is trimmed, has control and bidi characters removed, and is cut at 120 characters. Link text
that is a URL, or is `here`, `link`, `this` or `url` (any case), is ignored.

*Rejected:* first name wins. Rejected because a reply's `[the PR](url)` would block the real
PR title.

### D6. Labels are computed, not stored
`Item` gains:

- `name: Option<String>`;
- `label: String`, the name or `links::label(url)`;
- `linkType: "pr" | "artifact" | "web"`.

The labels are:

| Link | Label |
|---|---|
| GitHub PR | `PR #918 · shop-api` |
| claude.ai artifact | `Artifact` |
| any other link | `docs.example.test · guide` (host and last path segment) |

A later change to the label rules then needs no migration.

### D7. Artifact names from `PostToolUse`
`plugin-claude/agent.rs::events` maps `PostToolUse` with `tool_name == "Artifact"` to a new
`AgentEvent::Link { url, name }`:

- **URL:** the first claude.ai artifact URL found in `tool_response`, searched as text.
- **Name:**
  1. `tool_response.title`, else `tool_input.title`;
  2. else the `<title>` in the first 8 KB of `tool_input.file_path`, but only when that file is
     absolute, ends in `.html`, and, after resolving symlinks, sits under the session's `cwd`,
     `std::env::temp_dir()` or `/private/tmp`. Claude's scratchpad lives under
     `/private/tmp`, not under macOS's per-user temp dir. The five basic HTML entities are
     decoded.

`terminescu hook` forwards events up to 4 MB (`MAX_EVENT`), so the tool response arrives
whole.

The core records the event like a reply match, with rank `tool`. Nothing changes in the hook
binary, so its 100 ms budget is untouched: the app does the reading.

The exact shape of `tool_response` is not in the repository. Task 2.1 captures one real event
with invented content as a fixture before the parser is written.

*Rejected:* reading Claude's transcript JSONL for the tool result. Rejected because it is
slower, already covered by the hook, and couples the core to the transcript format.

### D8. `[Name](url)` in text
`extract::find` also matches `[text](https://…)` and returns `Found { kind: Link, value,
name: Some(text) }` with rank `text`. A bare URL keeps `name: None`.

### D9. PR names and badges through the plugin API
`plugin-api` gains:

- `CoreEvent::LinkAdded { side_tab, url }`, sent after a new link row lands. One runtime
  helper, `Runtime::link_added`, sends it from all three entry points: the paste command, the
  extractor sink, and `PluginCx::add_context_item`.
- `PluginCx::links(side_tab) -> Vec<String>`, the side tab's cleaned link URLs, with a default
  that returns none. The git plugin's existing `selected` handler uses it to re-check PR links
  older than the cache, so the core sends no extra events on select.
- `PluginCx::describe_link(side_tab, url, name: Option<String>, badge: Option<LinkBadge>)`,
  with a default no-op. `LinkBadge { text, tone, tip }` reuses the decoration `Tone`.
  `Decoration` itself does not fit, because its `Target` has no link variant.

The git plugin answers a GitHub PR URL with `Forge::pr_at(url)`, which runs
`gh pr view <url> --json number,title,state,isDraft,statusCheckRollup`. The result is cached
per URL for two minutes. The lookup runs on its own thread, as `look_up` does today, so it
never blocks event dispatch. The name is stored with rank `tool`. The badge stays in the runtime and
is not persisted. The app's `items` command adds it to each `Item` as `badge`, and a new badge
fires `context_changed`, so the panel refetches.

*Rejected:* the UI calling the git plugin's bridge per row. Rejected because the core panel
would then know about a plugin.

### D10. The panel
`ContextPanel.tsx`:

- **Section and Notes:** the section is titled "Links". Text items render under a "Notes"
  sub-head with its own count, and the group is hidden while empty.
- **Link rows:** a 12 px inline SVG icon (`currentColor`, in the panel's dim colour), the
  label, then the badge. `LinkText` and `LINK_TAIL` go.
- **Tooltip:** rows use `tips.beside(el, label, [url])`, the same tip as the other panel
  tooltips. The native `title` goes.
- **Right-click menu:** uses `ContextMenu.tsx`. Copy calls `navigator.clipboard.writeText`
  inside the click handler, as `TerminalPane` does for copy on select.
- **Prompt rows:** the variables span goes. The name button gets `white-space: normal` and a
  two-line clamp (`-webkit-line-clamp: 2`); the tooltip already shows the whole name.

## Risks / Trade-offs

- **`tool_response` shape unknown.** Mitigation: the fixture in task 2.1, and the `<title>`
  fallback from the file.
- **Dropping a trailing slash can change the target on some servers.** This is rare; the
  owner accepted it.
- **A signed link's token sits in SQLite and in the tooltip.** The owner accepted this (D2).
- **`gh pr view` per link costs a network call.** It is cached for two minutes, runs only
  while GitHub is on, and only for the selected side tab.
- **The one-time cleanup rewrites saved values.** It runs in one transaction, deletes
  nothing, and a value that fails to parse stays as it was.
- **The mock host must clean links the way the core does.** e2e runs against the mock. The
  mock uses the WHATWG `URL` parser, the same standard as the `url` crate, and
  `context.test.ts` runs the core's spec examples against it.

## Migration Plan

`0017_link_names.sql` adds `name_rank`. On start, `links::clean_saved` runs once, guarded by
`links.cleaned.1`. There is no rollback path: an older build ignores the new column.

## Diagrams

- `er.mmd`: `context_items` with `name` and `name_rank`.
- `sequence.mmd`: an artifact link from Claude, and a PR link through the git plugin.

## Plan page

Declined (owner, 2026-11-19): the change folder's files and diagrams are enough.
