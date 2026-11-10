# Tasks: add-plugin-architecture

## 1. Plugin API and host in Rust

- [x] 1.1 `crates/plugin-api`: `Plugin`, `Manifest`, `CoreEvent`, `PluginCx`, `Decoration`,
      `Helper`, `AgentAdapter` and the moved `AgentState`, `AgentView`, `AgentUpdate`. Verify:
      `cargo test -p terminescu-plugin-api` (a fake plugin and adapter compile and run).
- [x] 1.2 Core plugin host: registry, a worker and bounded queue per plugin, budgets,
      `catch_unwind`, pause after 5 failures a minute, switches. Verify: core tests
      `a_hanging_plugin_does_not_block_patches`, `a_panicking_plugin_is_paused`,
      `a_disabled_plugin_gets_no_events`.
- [x] 1.3 Migration `0013_plugins.sql` with `plugin_settings` and `plugin_side_tab_data`, and
      their store. Verify: core tests `plugin_settings_persist`,
      `row_data_goes_with_its_row`.
- [x] 1.4 Decorations store, `host://decorations` event, Tauri commands `plugin_call`,
      `plugin_list`, `plugin_set_enabled`, `decorations`. Verify: core test
      `decorations_merge_by_key_and_clear_per_plugin`; `pnpm check:rust`.
- [x] 1.5 Status facts and variable providers: plugins set named facts per row or tab that
      dynamic status rules test, and provide read-only prompt variables that lose to saved
      ones. Verify: core tests `a_plugin_fact_drives_a_dynamic_status`,
      `a_saved_variable_wins_over_a_plugin_variable`.

## 2. Plugin API and slots in the UI

- [x] 2.1 `packages/plugin-api`: `definePlugin`, contribution types, a registry that composes
      decorators in plugin order and merges actions with chord clashes resolved for the app.
      Verify: vitest `registry.test.ts`.
- [x] 2.2 Host: `pluginCall`, `onPluginEvent`, `decorations`, `onDecorations` in `ipc.ts` and
      the mock, which takes fake plugins. Verify: host vitest.
- [x] 2.3 Slots: row and tab decorations in `SideTabList` and `TabBar`, plugin sections in
      `ContextPanel`, the pane-view registry with the missing-plugin notice, plugin actions in
      menus and the palette; the UI reports row selection as `SideTabSelected`; plugin
      variables (`plugin:` ids) show read-only; Settings › Statuses offers fact rules. Verify: `e2e/plugins.spec.ts` with a fake plugin (chip, section,
      action, chord clash, missing view).
- [x] 2.4 Settings › Plugins: a switch per plugin and the stopped-after-errors state. Verify:
      `e2e/plugins.spec.ts` "switching a plugin off clears what it shows".

## 3. The Claude plugin

- [x] 3.1 `crates/plugin-claude` with `ClaudeAdapter`: `agent.rs` and `transcript.rs` move in;
      the runtime drives agents through the adapter list. Verify: the moved unit tests pass in
      the new crate; core tests pass with a fake adapter; `pnpm check:rust`.
- [x] 3.2 Replies and touched files become adapter events; `files.rs` keeps storage only; the
      `claude --resume` line comes from the adapter. Verify: core tests
      `a_reply_runs_the_extraction_rules`, `resume_uses_the_adapter_command`; e2e Files tests.
- [x] 3.3 Hook install, uninstall and the first-run entry become the plugin's helpers. Verify:
      moved `hook_install` tests; `e2e/app-sheets.spec.ts` first-run and uninstall.
- [ ] 3.4 `packages/plugin-claude`: `TranscriptPane` and transcript reading through
      `pluginCall`; the agent dot stays generic. Verify: transcript e2e tests unchanged.
- [ ] 3.5 Registration only in `main.rs` and `plugins.ts`, behind cargo feature
      `plugin-claude`. Verify: `cargo check -p terminescu-desktop --no-default-features`;
      `VITE_PLUGINS=none pnpm --filter @terminescu/desktop build`.

## 4. Seam and gate

- [ ] 4.1 `scripts/check-plugins.mjs` and `pnpm check:plugins`, run by `pnpm -r test`. Verify:
      the script fails on a deliberate import from `packages/ui` and passes on the tree.
- [ ] 4.2 Full gate: `pnpm check:rust`, `pnpm -r test`, all e2e tests, no Claude behaviour
      change. Verify: the commands; manual: a real `claude` session turns its row amber and
      `⌘⇧A` focuses it; with Claude off it does not.
- [ ] 4.3 `docs/plugins.md`: how a plugin is built and registered; README architecture lines.
      Verify: `openspec validate add-plugin-architecture --strict`.
