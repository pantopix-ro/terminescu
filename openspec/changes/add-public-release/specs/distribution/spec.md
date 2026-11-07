# Spec Delta

## Purpose
How a Terminescu build reaches other people's Macs: a signed, notarized DMG on GitHub
Releases and a signed update feed the app checks.

## ADDED Requirements

### Requirement: Release DMG
Each release SHALL be a DMG holding the app, attached to a GitHub Release of the public
repository with its SHA-256 sum. Without Developer ID secrets, the app and its bundled daemon
and CLI SHALL be ad-hoc signed. The release notes and the README SHALL then show the
one-time Privacy & Security › Open Anyway step.

#### Scenario: Unsigned release opens after Open Anyway
- **WHEN** a person downloads the ad-hoc signed DMG, starts the app, sees the macOS warning and chooses Open Anyway in Privacy & Security
- **THEN** the app starts, and later launches start with no warning

### Requirement: Notarized when an Apple account exists
With Developer ID secrets set, a release SHALL be signed under the hardened runtime,
notarized and stapled, and SHALL NOT be published if notarization fails. Signing credentials
SHALL NOT be stored in the repository.

#### Scenario: Notarized release opens cleanly
- **WHEN** the release was built with the Developer ID secrets and a person opens the downloaded app
- **THEN** macOS shows its standard "downloaded from the Internet" prompt naming the developer, with no "could not verify" warning

#### Scenario: Notarization fails
- **WHEN** the secrets are set and Apple rejects the notarization
- **THEN** the workflow fails and no release is drafted

### Requirement: Signed updates
The app SHALL check the release feed at launch and every 24 hours, and from Terminescu › Check
for Updates. It SHALL install an update only when the update's signature matches the public
key built into the app. It SHALL ask before it restarts and SHALL keep running shells across
the restart. The check SHALL be the app's only network call that a person did not start.

#### Scenario: Update available
- **WHEN** a newer signed release is published and the app checks
- **THEN** the app offers "Restart to update" with the new version, and after the restart the panes reattach to the same shells

#### Scenario: Tampered update
- **WHEN** the feed points at a file whose signature does not match
- **THEN** the app refuses it, keeps the current version and logs the reason

#### Scenario: Offline
- **WHEN** the check cannot reach the feed
- **THEN** the app shows nothing and tries again at the next interval
