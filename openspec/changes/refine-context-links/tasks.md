## 1. Cleaner and storage (core)

- [x] 1.1 Add `url = "2.5.8"` to `crates/core`. Write `links.rs` with `clean`, `label`,
  `link_type` and `NameRank` (D1, D6). Verify with `cargo test -p terminescu-core links::`,
  covering the spec scenarios: credentials and tracking removed, PR sub-page, artifact forms,
  ftp refused, IDN host, root slash kept.
- [x] 1.2 Dropped (owner, 2026-11-19: secret query parameters stay, D2). A `links::` test
  keeps `?sig=` to pin the decision.
- [x] 1.3 Add migration `0017_link_names.sql` (`name_rank`). `add_item` cleans the value,
  takes `name` with a rank and upgrades per D5. `Item` gains `name`, `label` and `linkType`.
  Verify with `cargo test -p terminescu-core context::`: two forms of one PR, then a reply name
  that does not override a tool name, then a same-rank rename.
- [x] 1.4 Add the one-time cleanup `context::clean_saved_links`, guarded by `links.cleaned.1` and run
  at app start (D4). Verify with `crates/core/tests/links.rs`: saved copies show once with the
  best name, a moved tab's auto copy survives, and a second run changes nothing.
- [x] 1.5 `extract::find` returns `[Name](url)` names (D8). Verify with `cargo test -p
  terminescu-core extract::`: a markdown link, ignored texts ("here", a URL), a bare URL.

## 2. Names from tools

- [ ] 2.1 Capture one `PostToolUse` `Artifact` event as a fixture with invented content
  (`crates/plugin-claude/tests/fixtures/hooks/post-tool-use-artifact.json`, written with
  invented data; the real capture is part of 4.2). Verify by a manual
  check: publish a throwaway artifact and copy the event from `app.log` with the content
  replaced.
- [x] 2.2 `AgentEvent::Link` comes from that event, with the title from the response, the
  input or the HTML `<title>` (D7, file checks included). The core records it with rank
  `tool`. Verify with `cargo test -p terminescu-plugin-claude agent::`: the fixture, the HTML
  fallback, and a path outside cwd and temp that is ignored.
- [x] 2.3 Add `CoreEvent::LinkAdded` (sent through `Runtime::link_added` from all three
  entry points), plus `PluginCx::links` and `PluginCx::describe_link` with defaults. The
  runtime keeps badges per side tab and sends them with the items (D9). Verify with `cargo test
  -p terminescu-plugin-api` and `apps/desktop` `plugins.rs` tests. Done: covered through the
  git plugin's fake `PluginCx` and the e2e rows; `plugins.rs` has no unit test of its own.
- [x] 2.4 Add `Forge::pr_at(url)` with a two-minute cache. The git plugin names PR links and
  sets badges, re-checking on `selected`, and stays quiet while GitHub is off. Verify with
  `cargo test -p terminescu-plugin-git` using a fake `gh`: open with failing checks, merged
  later, logged out. Done: the fake covers failing checks, the two-minute cache on `selected`,
  a non-PR link and logged out; "merged later" needs a clock seam and waits for 4.2.

## 3. Panel

- [x] 3.1 Add the `Item` fields and the badge to `packages/host` (types, IPC, mock with
  cleaning parity for the e2e). Verify with `pnpm --filter @terminescu/host test`
  (`context.test.ts`).
- [x] 3.2 Rename the section to "Links" and give Notes its own group and count (spec "Notes
  under Links"). Update the "Links and text" uses in `context.spec.ts`, in `host.ts` and in
  the unarchived `refine-context-panel` spec. Verify with e2e `context.spec.ts` "a captured
  branch goes to Notes".
- [x] 3.3 Link rows: icon, label, badge, panel tooltip and no native title (spec "Link rows").
  Verify with e2e `context.spec.ts` "a link row shows its name and the URL in the tooltip".
- [x] 3.4 Right-click menu: Open, Copy link, Copy as Markdown, Remove. Verify with e2e
  `context.spec.ts` "copy a link as Markdown".
- [x] 3.5 Prompt rows: name only, two-line clamp. Verify with e2e `prompts.spec.ts` "a long
  prompt name wraps and shows no placeholders", and update the existing `.ao-dim` hover test.

## 4. Close

- [x] 4.1 Run `pnpm check:rust`, `pnpm -r test`, the full e2e suite and `openspec validate
  refine-context-links --strict`.
- [ ] 4.2 Manual check on real data: an artifact published from Claude shows its title, a PR
  shows its title and badge, and old duplicate links merge after the first start.
