# Releasing

This page is for the person who cuts a release. A release is a draft GitHub Release with the
DMG, the signed update archive, `latest.json` and `SHA256SUMS`. The workflow is
`.github/workflows/release.yml`.

## A DMG without a release

Actions › Release › Run workflow, with "Draft a GitHub Release" off, runs the checks, builds
the DMG and attaches it to the run as the `Terminescu-<version>` artifact for 14 days. Nothing
is published. Use it to try a build on another Mac.

Without the update key below, the workflow still builds the DMG, but that build never updates
itself, and a drafted release says so in its notes. With the key, a drafted release must come
from the repository named in `PUBLIC_REPO` (`crates/protocol/src/public.rs`), because installed
apps read their update feed there.

## Once: the update key

The app installs an update only when its minisign signature matches the public key built into
the app. Make the key pair on your Mac:

```
pnpm --filter @terminescu/desktop tauri signer generate -w ~/.tauri/terminescu-updater.key
```

Pick a password when it asks. Then, in the repository's Settings › Secrets and variables ›
Actions:

- variable `TERMINESCU_UPDATER_PUBKEY`: the contents of `~/.tauri/terminescu-updater.key.pub`
- secret `TAURI_SIGNING_PRIVATE_KEY`: the contents of `~/.tauri/terminescu-updater.key`
- secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the password

Keep a copy of the private key and its password in the company password manager. A lost key
means every installed copy stops taking updates until people download a new DMG by hand.

A build without `TERMINESCU_UPDATER_PUBKEY`, such as a local `pnpm tauri:build`, never checks
for updates. Check for Updates then says that updates are off in this build.

## Rotating the key

Releases signed with a new key reach only apps that carry the new public key. So rotate in
two releases:

1. Make the new pair. Set the new public key in `TERMINESCU_UPDATER_PUBKEY` but keep signing
   with the old private key. Release. Installed apps update to a build that trusts the new key.
2. Wait until most people have that release. Swap the private key secrets for the new pair and
   release again.

If the old private key leaked, skip step 2's wait and ask people to download the DMG.

## Cutting a release

1. Raise `version` in `apps/desktop/src-tauri/tauri.conf.json` and the workspace `Cargo.toml`.
2. Merge to `main`, then push a tag: `git tag v0.2.0 && git push origin v0.2.0`. The workflow
   also runs from Actions › Release › Run workflow.
3. When the job is green, open the draft release. Check that it has the DMG,
   `Terminescu.app.tar.gz`, its `.sig`, `latest.json` and `SHA256SUMS`, and that the job log
   shows `Signature=adhoc`.
4. Publish the draft. Installed apps find it at their next daily check, or at once through
   Check for Updates. The feed is `releases/latest/download/latest.json`, so a draft never
   reaches anyone.

## Checking the updater by hand

Run this before the first public release and after any change to `updates.rs`.

1. Build 0.1.0 with the key: `TERMINESCU_UPDATER_PUBKEY="$(cat ~/.tauri/terminescu-updater.key.pub)"`
   plus the two `TAURI_SIGNING_*` variables, then `pnpm tauri build --config
   src-tauri/tauri.release.conf.json` in `apps/desktop`. Install that app.
2. Raise the version to 0.1.1 and build again. Put `Terminescu.app.tar.gz`, its `.sig` and a
   `latest.json` pointing at them on a release of a test repository, and point
   `PUBLIC_REPO` at that repository for both builds.
3. Open 0.1.0 and pick Check for Updates. "Terminescu 0.1.1 is ready" appears. Restart to
   update, and the panes come back on the same shells.
4. Change one byte of the `.sig` and check again from 0.1.0. The app refuses the update, stays
   on 0.1.0, and `app.log` says `update 0.1.1 refused`.

## Checking the first run by hand

Run this before the first public release and after any change to the first-run sheet or
`hook_install.rs`. It needs a Mac user account with Claude Code, where losing the app's data
does not matter. Quit Terminescu first.

1. Back up `~/.claude/settings.json`, then move away the app data:
   `~/Library/Application Support/ro.pantopix.terminescu` and `~/Library/WebKit/ro.pantopix.terminescu`.
2. Open the release build. "Welcome to Terminescu" shows, with the hook box ticked. Tab moves
   only between its controls.
3. Pick Continue. `~/.claude/settings.json` now holds a `terminescu hook` entry for each hook
   event, with the CLI path quoted, and every other hook in it is unchanged.
4. In a pane, run `claude` and send a prompt. The side tab shows Claude working, then turns
   amber when Claude asks for permission, and the transcript pane follows the session.
5. Quit and open the app again. The welcome screen does not come back.
6. Settings › Shell › turn the hook off. The entries leave `settings.json`; other hooks stay.
7. Turn it on again, then Help › Uninstall Terminescu Helpers. The entries leave
   `settings.json`, the shells end, and `~/Library/Application Support/Terminescu` is gone.
8. Restore the `settings.json` backup and the app data.

## Later: Developer ID and notarization

Once PANTOPIX RO has an Apple Developer account, add these secrets. The same workflow then
signs with the hardened runtime and notarizes, and the "could not verify" warning goes away.

- `APPLE_CERTIFICATE`: the Developer ID Application certificate as base64 `.p12`
- `APPLE_CERTIFICATE_PASSWORD`: its export password
- `APPLE_SIGNING_IDENTITY`: for example `Developer ID Application: PANTOPIX RO (TEAMID)`
- `APPLE_ID`, `APPLE_PASSWORD` (an app-specific password) and `APPLE_TEAM_ID`

Run the spike in `openspec/changes/add-public-release/tasks.md` (Later) first: the pty, the
sockets and WebGL must work under the hardened runtime, which needs an `Entitlements.plist`.
Then remove the Open Anyway steps from the README and `.github/release-notes.md`.
