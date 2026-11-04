# Spike 0.3: daemon lifetime

Run on 2026-11-04 with a throwaway Tauri 2.12 app (`TerminescuSpike.app`) that bundles the spike
daemon as a resource. The app copies it to
`~/Library/Application Support/TerminescuSpike/bin/ptyd-<build>` and starts it with
`std::process::Command`, `pre_exec(setsid)` and stdin, stdout and stderr detached (D32).

**Result: go. The daemon and every shell survive quit, crash and bundle replacement.**

## Steps and outcome

| Step | Outcome |
|---|---|
| Launch build b1 with `open`; it starts the daemon, three panes printing `tick N` every second and a login shell, then quits through `app.exit(0)` | `RunEvent::Exit` logged; daemon alive with PPID 1 (launchd), its own session and process group |
| Check the panes | All 4 alive, ticks still counting |
| Delete the bundle, copy build b2 in its place, launch with `open` | b2 connects to the b1 daemon (`started: false`), handshake on protocol 1 |
| b2 lists and reattaches pane 1 | 4 panes alive; replay shows `tick 1` to `tick 47` with no gap, live ticks continue |
| Launch b2, then `kill -9` the app | Daemon and ticks unaffected |
| Login shell started by a daemon that `open` launched | `brew` and `node` resolve, so `PATH` comes from the zsh startup files; `LANG` reads `C.UTF-8` |

## Findings for task 2.1

- `setsid` plus a copy outside the bundle is enough. No launchd agent is needed for P1.
- `std::fs::copy` keeps `com.apple.quarantine`. A quarantined, ad-hoc signed copy still ran when
  the app executed it, but `spctl` rejects it. The app clears the quarantine attribute on its
  copy, and release builds are signed and notarized so the copied binary keeps a valid signature.
- The copy also sidesteps App Translocation, which runs an unmoved downloaded app from a random
  read-only path that changes per launch.
- The daemon inherits the app's environment. It sets `LANG` from the macOS locale when unset, as
  Terminal.app does, and strips `CLAUDE*` variables (see `reattach.md`).
- Protocol N-1 support was not exercised; the handshake check is a unit test in task 2.1.
