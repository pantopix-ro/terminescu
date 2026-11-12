# Proposal: add-public-release

## Why

Terminescu runs on one Mac today, built from source. Other people should be able to download
a DMG from GitHub, open it without a Gatekeeper warning, learn who made it and under which
license, and reach PANTOPIX RO when something breaks. The app also adds its hook to Claude
Code's settings on every launch without asking. That is fine for the owner, but not for a
stranger's machine.

## What Changes

- **License.** The repository ships under FSL-1.1-ALv2 (the Functional Source License).
  The code is public. Nobody may offer a competing product built from it, and each release
  turns Apache 2.0 two years after it ships. Outside contributions need a signed CLA, so the
  license can change later.
- **Public home.** The repository moves to a PANTOPIX RO organization on GitHub. Releases,
  issues and the update feed live there.
- **DMG in CI, unsigned for now.** The owner chose not to pay for an Apple Developer account
  yet. A GitHub Actions workflow builds an ad-hoc signed DMG and attaches it to a GitHub
  Release. The README and the release notes show the one-time "Open Anyway" step. When
  Developer ID secrets are set, the same workflow signs with the hardened runtime, notarizes
  and staples instead. This replaces foundation task 9.1.
- **Auto-update.** The Tauri updater checks the GitHub Releases feed and verifies each update
  with its own signing key. It asks before it restarts. This replaces foundation task 9.2.
- **About window.** It shows the version and build, "Made by PANTOPIX RO" with a link to
  pantopix.ro, the license, the open-source credits their licenses require, and the line
  "Works with Claude Code. Not affiliated with Anthropic."
- **Help menu.** It holds Send Feedback, Report an Issue, Privacy and Licenses. Send Feedback
  opens an email to `salut@pantopix.com` whose subject starts with `[Terminescu]`. Report an
  Issue opens a pre-filled GitHub issue. Both carry the app and macOS versions. `app.log` goes
  along only when the person ticks a box.
- **Privacy note.** In the app and in the repository: no telemetry and no network calls
  except the update check. It also says what the hook reads and where variables and secrets
  live.
- **First run.** A first-run screen explains the hook and asks before adding it. Without the
  hook the terminal works; Claude status and the transcript pane do not. **BREAKING** for the
  owner's install: the hook is no longer added on every launch, only after consent. An
  existing entry counts as consent.
- **Uninstall.** Help › Uninstall Terminescu Helpers removes the hook entries from Claude
  Code's settings, the copied CLI and daemon binaries and, if the person chooses, the support
  folder. The person then drags the app to the Trash. Settings › Shell can turn the hook off
  and on.

Out of scope:
- The paid tier: a hosted relay for the remote view, owned by `add-remote-view`.
- A Homebrew cask: a follow-up once the first signed release exists.
- Crash reporting: there is no telemetry.

## Capabilities

### New Capabilities
- `distribution`: how a build reaches people: the signed and notarized DMG, GitHub Releases,
  the signed update feed and the update prompt.
- `app-identity`: who made the app and under what terms: About, license and credits, the
  privacy note, the not-affiliated line and the feedback routes.
- `setup`: what the app installs outside its bundle and with whose consent: the first-run
  screen, the hook consent and toggle, and the uninstall of the helpers.

### Modified Capabilities
None in `openspec/specs/`. The hook bridge requirement ("The app SHALL install `terminescu
hook`…") lives in the unarchived `add-terminescu-foundation` change. `setup` adds the consent
condition on top of it. Archive the foundation first, then fold `setup`'s consent rule into
`claude-awareness` when this change archives.

## Impact

- `apps/desktop/src-tauri`:
  - `tauri.release.conf.json`: DMG target, updater artifacts, ad-hoc signing; entitlements
    come with the notarized releases.
  - `main.rs`, `public.rs`, `updates.rs`: Help menu, About, the update check; the hook
    install moves behind consent.
  - New `tauri-plugin-updater`, at least two weeks old. The updater's public key is built
    in from a repository variable.
- `crates/core`: `hook_install::uninstall`, a `hook` setting with its consent state,
  `feedback` links and the helpers cleanup in `uninstall`.
- `packages/ui`: the About, Licenses, Privacy, Feedback and first-run sheets; the hook toggle
  in Settings › Shell.
- New repository files: `LICENSE.md` (FSL), `PRIVACY.md`, `CONTRIBUTING.md`, `CLA.md`,
  `deny.toml`, `about.toml`, `docs/releasing.md`, and `.github/workflows/` `licenses.yml`
  and `release.yml`. The credits file `licenses.json` is generated at build time.
- Secrets: the Apple certificate, notarization credentials and the updater private key live
  in GitHub repository secrets and never in the repository.
- Foundation tasks 9.1 and 9.2 get ticked "Dropped (moved to add-public-release)".

## Unresolved questions

1. When to buy the Apple Developer account (99 USD/year). Until then, macOS warns that it
   "could not verify" the app. Recommendation: buy it under PANTOPIX RO as an organization
   (needs a D-U-N-S number) before announcing the app widely.
2. GitHub organization slug. Answered 2026-11-11: `lzrmihnea/terminescu` for now, kept in one
   constant; a later move to `pantopix/terminescu` changes that constant.
   Moved 2026-11-12: the repo now lives at `pantopix-ro/terminescu`.
3. CLA tool. Recommendation: the CLA Assistant GitHub app with a short individual CLA.
