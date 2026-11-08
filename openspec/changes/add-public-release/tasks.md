# Tasks

## 1. Repository and license

- [x] 1.1 Add `LICENSE.md` (FSL-1.1-ALv2 from fsl.software, licensor PANTOPIX RO), a license
      section in the README, `CONTRIBUTING.md` and `CLA.md` (D1); Cargo `license` is the SPDX
      id. Verify: `head -1 LICENSE.md` names FSL 1.1 ALv2, `grep PANTOPIX LICENSE.md`.
- [x] 1.2 Add `PUBLIC_REPO` in `crates/protocol` (assumed `pantopix/terminescu`) and use it for
      every GitHub link (D4, D7). Verify: `grep -rn 'github.com/' crates apps packages` shows
      only `PUBLIC_REPO`.
- [ ] 1.3 Scan the whole git history for real names, paths, tokens and emails before the repo
      goes public. Rewrite or squash anything found. Verify: the scan pattern from CLAUDE.md
      over `git log -p --all` prints nothing.
- [ ] 1.4 Owner: move the repo from the owner's personal account to `pantopix/terminescu` (the name
      `PUBLIC_REPO` and the update feed use), install the CLA Assistant app, make the repo
      public, and update the remote and `docs/getting-started.md`. The release job refuses to
      run anywhere else. Verify: manual, `git remote -v` shows `pantopix/terminescu`.

## 2. Credits and license checks

- [ ] 2.1 `about.toml` and `deny.toml` with the allowed licenses. `cargo deny check licenses`
      runs in CI (`.github/workflows/licenses.yml`, D8). Verify: CI job green; adding a
      GPL-only test crate fails it. Local `cargo deny check licenses` says "licenses ok"; the
      job runs once the repo is on GitHub.
- [x] 2.2 Generate `apps/desktop/public/licenses.json` from `cargo about` and pnpm on every
      build (`scripts/credits.mjs`, gitignored: notices carry authors' addresses), shown under
      Help › Licenses (D8). Verify: `pnpm credits` writes the file, and it lists xterm,
      codemirror, tauri and alacritty_terminal.

## 3. Hook consent and first run

- [x] 3.1 Core: `Settings.hook` (`unasked`/`on`/`off`) and `firstRunDone`, plus
      `hook_install::present` and `hook_install::uninstall` (D5). Verify: core tests
      `hook_uninstall_keeps_other_hooks`, `unasked_with_entry_becomes_on`, `uninstall_drops_a_hooks_object_it_emptied…`.
- [x] 3.2 `main.rs`: install the hook only when `on`, and resolve `unasked` at launch (D5).
      Verify: a launch with `hook: "off"` leaves a fixture settings file byte-identical (core test
      `hook_off_leaves_settings` on `hook_install::at_launch`); the toggle goes through `settings_set`.
- [x] 3.3 UI: the first-run sheet (helpers list, hook switch, "Not now", Privacy link) and the
      hook toggle in Settings › Shell; a Claude pane says "hook is off" with a button (D6).
      Verify: `first-run.spec.ts` (shows once, Not now adds no hook, the toggle adds and
      removes, the Claude pane bar turns it on).

## 4. Help menu, About, privacy and feedback

- [x] 4.1 Help menu and a custom About item, built in `main.rs` from `public.rs` ids. They have
      no chord, so they stay out of `actions.json` and Settings › Keys (D7). Verify: Rust test
      `menu_only_ids_never_clash_with_an_action`; manual check of the menu.
- [x] 4.2 About sheet: version and build, "Made by PANTOPIX RO" link, license, Licenses
      button, the not-affiliated line. Verify: `about.spec.ts` reads the mock version and
      checks `host.openedUrls` gets `https://pantopix.ro`.
- [x] 4.3 Licenses and Privacy sheets, plus `PRIVACY.md` with the same facts. Verify:
      `about.spec.ts` finds "xterm" in Licenses and "update check" in Privacy; ui test
      `privacy.test.ts` keeps `PRIVACY.md` and the sheet word for word.
- [x] 4.4 Feedback dialog with "Include app.log" off by default. The `mailto:` link to
      salut@pantopix.com has the subject prefix `[Terminescu]`; the issue link uses
      `PUBLIC_REPO`. Both carry the versions, and the log is revealed in Finder only when the
      box is ticked (D7). Verify: Rust test `feedback_links_carry_versions`, and
      `feedback.spec.ts` checks `host.openedUrls` and no reveal without the box.

## 5. Uninstall

- [x] 5.1 Help › Uninstall Terminescu Helpers, in the order of D9: confirm, hook, daemon,
      `bin/` and `run/`, data opt-in, quit. Verify: core test `uninstall_removes_helpers_keeps_db`
      on a temp `TERMINESCU_HOME`; `uninstall.spec.ts` for the dialog.

## 6. Updates

- [ ] 6.1 Add `tauri-plugin-updater` 2.14.0 (at least two weeks old; `AppHandle::restart`
      replaces `tauri-plugin-process`), the public key from `TERMINESCU_UPDATER_PUBKEY` at build
      time and the endpoint from `update_feed()`, the 24 h check, Check for Updates in the app
      menu and the "Restart to update" prompt (D4). Verify: `updates.spec.ts`; a test feed
      updates 0.1.0 to 0.1.1, and a tampered signature is refused (manual, steps in
      docs/releasing.md, waits for the update key).

## 7. Release pipeline

- [ ] 7.1 `.github/workflows/release.yml`: build `app,dmg` ad-hoc signed, write
      `latest.json` and `SHA256SUMS`, add the Open Anyway text to the notes, draft the
      release. The Developer ID path switches on when the Apple secrets exist (D2). Verify:
      a `workflow_dispatch` run uploads a DMG, and `codesign -dv` on the app says
      `Signature=adhoc`.
- [ ] 7.2 README install section with the Open Anyway steps and a screenshot, plus
      `docs/releasing.md`: the updater key, its rotation, publishing a draft, and the Apple
      secrets for later. Verify: manual read-through by the owner.
- [ ] 7.3 First release v0.2.0, unsigned. Verify: download it on a second Mac account, use
      Open Anyway once, and the app starts; the next launch shows no warning; an update to
      v0.2.1 installs from the feed.
- [x] 7.4 Tick foundation tasks 9.1 and 9.2 "Dropped (moved to add-public-release)". Verify:
      `openspec validate add-terminescu-foundation --strict`.

## Later: notarized releases (after the Apple account)

Not planned for now (owner, 2026-11-08): releases stay ad-hoc signed.

- Spike: sign and notarize one build by hand. Confirm the pty, the sockets and WebGL under
  the hardened runtime, and settle `Entitlements.plist` (D2, D3). Check that `spctl -a -vv`
  says "accepted, source=Notarized Developer ID".
- Set the Apple secrets, then release. A download opens with no "could not verify" warning.
- Homebrew cask.

## Workflow follow-up

- Archive `add-terminescu-foundation` before this change, then fold the `setup` consent rule
  into `claude-awareness` › Hook bridge when archiving.
