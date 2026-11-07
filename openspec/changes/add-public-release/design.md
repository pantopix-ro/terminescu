# Design: add-public-release

## Context

See proposal.md › Why. The state this design builds on:

- `tauri.conf.json` bundles only the `app` target, with `macOSPrivateApi: true` for the
  see-through terminals. That rules out the App Store but not Developer ID distribution.
- At launch `main.rs` copies the daemon to `bin/ptyd-<version>-<hash>` and the CLI to
  `bin/terminescu` under the support folder, then calls `hook_install::install` every time.
  The copy step clears the quarantine attribute (`launcher::clear_quarantine`).
- `hook_install::apply` already removes every earlier Terminescu entry before adding fresh
  ones. A remove-only pass is the same code with no add.
- The shell integration ships inside the bundle and loads through `ZDOTDIR` per pane, so no
  dotfile is edited. Uninstall has no `.zshrc` lines to undo.
- Secret values live only in the daemon's memory (D35). Saved variables, commands and the
  layout live in the SQLite database in the support folder.
- There is no Help menu and no `.github/` folder yet. The About item is the predefined Tauri
  one.

## Goals / Non-Goals

**Goals:**
- A stranger downloads, opens, updates and removes the app without reading a README.
- Every credential lives in GitHub secrets. A fork can build unsigned.

**Non-Goals:**
- Intel builds: macOS 14+ on Apple silicon, as today.
- Mac App Store: the private API and the helpers outside the sandbox rule it out.
- Licence keys or a paywall.

## Decisions

### D1. FSL-1.1-ALv2, CLA for contributions
`LICENSE` holds the FSL text with "Apache 2.0" as the future license and "PANTOPIX RO" as the
licensor. `CONTRIBUTING.md` asks for the CLA through the CLA Assistant GitHub app.
Rejected:
- **MIT**: anyone could ship a renamed copy commercially.
- **GPL with a CLA**: it still allows a commercial fork that stays open.
- **Closed**: it loses the showcase, and the trust the hook and the secrets need.

### D2. Release workflow on GitHub Actions, macOS runner, unsigned until an Apple account exists
The owner chose not to pay for an Apple Developer account yet (2026-11-07). Developer ID
signing and notarization need one, at 99 USD/year. A free Apple ID cannot notarize.

`.github/workflows/release.yml` runs on a `v*` tag:
1. `pnpm tauri build --bundles app,dmg` with `TAURI_SIGNING_PRIVATE_KEY`. That is the
   updater key, which is free and separate from Apple.
2. Signing depends on the secrets:
   - **No `APPLE_CERTIFICATE`:** the signing identity is `-`. Tauri ad-hoc signs the
     sidecars and the app, which Apple silicon requires, and the release notes get the
     Open Anyway steps.
   - **With `APPLE_CERTIFICATE`, `APPLE_SIGNING_IDENTITY` and `APPLE_API_KEY`:** Tauri signs
     with the hardened runtime and `Entitlements.plist`, then notarizes and staples. A failed
     notarization fails the job.
3. The job writes `latest.json`, the DMG, the `.app.tar.gz` update and `SHA256SUMS`.
4. A draft GitHub Release carries them all. The owner publishes the draft by hand.

Entitlements, for the signed path only: none beyond the hardened runtime defaults. Spike 7.0
confirms that the pty spawn, the unix sockets and WebGL run under it, and adds
`com.apple.security.cs.allow-jit` only if WKWebView needs it.

Rejected: shipping only source and asking people to build. Nobody outside the team would
try it.

Rejected: building on the owner's laptop runners (CI_RUNS_ON in Kaisha). The signing
certificate would then sit on a personal machine, and the clamshell sleep kills jobs.

### D3. The copied daemon and CLI keep the bundle's signature
`launcher::install` copies the bytes as they are, so the Developer ID signature travels with
them. Clearing the quarantine attribute stays: Gatekeeper then trusts the signed copy without
a second prompt.
Rejected: running the daemon from inside the bundle. An update replaces the bundle while the
shells run, and D32 relies on the versioned copy.

### D4. Tauri updater on the GitHub Releases feed
`tauri-plugin-updater` reads `https://github.com/<org>/terminescu/releases/latest/download/latest.json`
and verifies the minisign signature against the public key in `tauri.conf.json`. The check
runs at launch, every 24 h and from the menu.

`tauri-plugin-process` handles the restart. The daemon keeps the shells (D32), and the new
app reattaches.

`<org>` is one constant, `PUBLIC_REPO` in `crates/protocol`, so the org slug changes in one
place.
Rejected: Sparkle. It needs a second signing scheme and an appcast server for no gain over
the plugin Tauri maintains.

### D5. Hook consent as a setting
A new `hook: "unasked" | "on" | "off"` field in `Settings` (serde default `"unasked"`).

At launch:
- `"on"`: install as today.
- `"off"`: do nothing.
- `"unasked"`: if `hook_install::present()` finds an entry, set `"on"` (owner's install,
  scenario "Owner's existing install"). Otherwise the first-run screen asks.

`hook_install::uninstall(path)` is `apply` without the add. The toggle in Settings › Shell
calls `install` or `uninstall`.

No SQLite migration: settings are one JSON row (`settings::KEY`), and serde fills the new
field.

Rejected: a separate `consent` table. One more table for one value.

### D6. First run is "hook unasked and first run not done"
The screen shows while `hook` is `unasked` and `firstRunDone` is false; finishing it, either
way, sets `firstRunDone`. A Mac whose Claude Code settings already hold the hook resolves to
`on` at launch and marks the first run done, so the owner never sees it. The screen is a
sheet in the first window: Terminescu, the helpers list, a hook switch (on by default, with
"Not now"), and a link to Privacy. The toggle in Settings › Shell changes `hook`, and
`settings_set` edits Claude Code's settings before it saves, so a failed edit saves nothing.
Rejected: keying the screen on a missing support folder. The launch creates the folder before
the window opens, and a crash mid-screen would skip it for good. Also rejected: a separate
window, which races the first window's focus and its menu state.

### D7. Help menu and About as app sheets
`actions.json` gains a `Help` menu:

| Item | Action id |
|---|---|
| Send Feedback… | `feedback_email` |
| Report an Issue… | `feedback_issue` |
| Privacy | `privacy` |
| Licenses | `licenses` |
| Uninstall Terminescu Helpers… | `uninstall` |

None of them has a chord. The predefined About item becomes a custom `about` action that
opens an About sheet, so it can show the build, the link and the not-affiliated line.

Feedback builds the link in Rust:
- Email: `mailto:salut@pantopix.com?subject=[Terminescu] Feedback&body=<versions>`.
- Issue: `https://github.com/<org>/terminescu/issues/new?title=&body=<versions>`.

The link opens through the existing `open_url`. "Include app.log" reveals the log in Finder.
`mailto:` cannot carry an attachment, and a pre-filled GitHub issue cannot upload one.

Rejected:
- **An in-app form posting to a server**: it would be a second network call and needs a
  backend.
- **A support email on pantopix.ro**: the owner chose `salut@pantopix.com`.

### D8. Credits generated from the lock files
- **Rust**: `cargo about generate` writes the crates' licenses. `about.toml` holds the
  allowed list: MIT, Apache-2.0, BSD-2/3, ISC, Zlib, Unicode-3.0, MPL-2.0.
- **npm**: a small script reads pnpm's `licenses list --json --prod` for the desktop app.

Both feed `THIRD_PARTY_LICENSES.md`, which is bundled as a resource and rendered in the
Licenses sheet. `cargo deny check licenses` runs in CI and in the release job.

`cargo-about` and `cargo-deny` are CI tools, not app dependencies. Each is pinned to a
version at least two weeks old.

Rejected: a hand-written list. It goes stale with the first dependency bump.

### D9. Uninstall order
The steps run in this order:
1. Confirm, with "Also delete my Terminescu data" unticked.
2. `hook_install::uninstall`.
3. Ask the daemon to end, when shells are open and the person confirmed.
4. Remove `bin/` and `run/`.
5. Remove the support folder if ticked.
6. Quit with a final note: "Drag Terminescu to the Trash."

The hook goes first, so a failure later never leaves Claude calling a missing binary. The
hook already exits 0 when the CLI is gone (D34), but Claude would still log an error per
event.

## Risks / Trade-offs

- [Unsigned releases show "Apple could not verify Terminescu is free of malware"] → On macOS 15+
  there is no Control-click bypass; the person opens System Settings › Privacy & Security ›
  Open Anyway once. The README shows it with a screenshot. An app that also edits Claude
  Code's settings may scare people off at this step, so buy the account before a wide
  announcement.
- [Each ad-hoc build has a new code identity] → macOS may ask again for the notification
  permission after an update. Updates arrive through the app, not a browser, so they carry
  no quarantine flag and show no Gatekeeper prompt.
- [Homebrew's main cask repository is dropping apps that fail Gatekeeper] → No cask until the
  app is notarized.
- [Notarization rejects a sidecar or an entitlement] → Spike 9.0 notarizes a throwaway build
  before the workflow is final.
- [An update restarts the app while a Claude prompt is typed] → The app asks first and never
  restarts on its own.
- [The FSL is not OSI open source, and some developers object] → The README says it in one
  line and names the two-year Apache conversion.
- [`app.log` holds paths and pane titles] → The box is off by default and the dialog says so.
  Nothing is uploaded by the app.
- [The repo transfer breaks clone URLs] → GitHub redirects the old URL. Update the remote and
  docs/getting-started.md.

## Migration Plan

1. Transfer the repository to the org, add `LICENSE`, and make it public only after the scan
   for real data passes over the whole history (task 1.3).
2. The owner's Mac: the `unasked` → `on` rule keeps the hook with no question.
3. Rollback: delete a bad release, and the feed falls back to the previous `latest.json`.
   Installed apps keep their version.
