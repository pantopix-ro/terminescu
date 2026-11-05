# Spike 0.1: reattach

Run on 2026-11-04 with a throwaway daemon (Rust, `portable-pty`, framed Unix-socket protocol,
byte ring with resize markers) and `@xterm/headless` 6.0 as the viewer. The pane ran a login
shell, then `claude` 2.1.318 on Haiku answering a short prompt. A "live" xterm stayed attached
from the first byte and served as the truth; each reattach started a fresh xterm and replayed.

**Decision: the daemon keeps a headless screen per pane (`alacritty_terminal`) and replays a
serialized snapshot. Raw replay alone is rejected.** D7, D20 and D31 change accordingly.

## Raw replay

| Case | Result |
|---|---|
| Same width (100 cols), ring untrimmed | Identical to live: 0 rows differ, same cursor, same buffer |
| Replay at 100, then resize view and pty to 70 | Identical to live after claude's redraw (2.7 KB) |
| Fresh attach after that resize (ring holds both widths) | Identical to live |
| Ring trimmed to its last 16 KB | Wrong: normal buffer instead of alternate, 4 rows differ, frames pile into scrollback |
| Trimmed, then resize rows 30 → 29 → 30 | Visible rows right (7.4 KB repaint), buffer still wrong |
| Trimmed tail with `ESC[?1049h` prepended | Alternate screen right, main-screen history gone (0 of 124 lines) |
| Resize to the same size | 0 bytes: no SIGWINCH, no redraw |

`claude` 2.1.318 runs in the alternate screen and repaints all of it on SIGWINCH. Raw replay is
exact while the ring holds the whole stream. Once the ring drops its head, the replay loses the
alternate-screen switch, the shell history above `claude`, and any other mode set early
(bracketed paste, mouse, cursor keys). An 8 MB ring fills during a long Claude session, because
every spinner tick and repaint lands in it.

## Headless screen model

A second tool fed the same recorded stream (60 KB, two resizes) into `alacritty_terminal` 0.25.1
and serialized the grid: main-screen history and screen with SGR attributes, then
`ESC[?1049h` and the alternate screen with absolute positioning, then cursor and modes.

| Check | Result |
|---|---|
| Alternate screen rows | 0 of 30 differ |
| Main-screen history | 124 of 124 lines match |
| Cursor and active buffer | Same |
| Colors and attributes, every non-blank cell | 0 of 2,447 differ |
| Snapshot size | 6.4 KB for a 60 KB stream; 1.2 MB for 10,000 lines of a 50 MB log |
| Parse speed | 50 MB log in 664 ms, about 79 MB/s on one core under heavy load |
| Memory | 90 MB peak with the 50 MB input in memory; the 10,000 × 120 grid takes about 30-40 MB |

## Consequences

- Attach sends one snapshot at the pty's current size. The view creates its xterm at that size,
  writes the snapshot, then resizes to its own size; a size change redraws through SIGWINCH.
- No resize nudge after replay: the snapshot is already exact, and a same-size resize does
  nothing anyway.
- `alacritty_terminal` keeps the inactive grid private. The spike reached it through `swap_alt`,
  which destroys the alternate screen. Task 2.1 needs an accessor: a small upstream patch, a
  vendored copy, or a check of `wezterm-term` or `vt100` instead.
- Memory grows with scrollback. 10 full panes at 120 columns cost about 300-400 MB, so the
  scrollback limit stays a setting (10,000 lines by default).
- The daemon started from a Claude Code shell passed `CLAUDE_CODE_*` variables to the pane, and
  `claude` there warned "Transcript saving is off". The daemon strips `CLAUDE*` variables from
  each pane's environment.

## Daemon notes for task 2.1

- macOS limits a Unix socket path to 104 bytes. `~/Library/Application Support/Terminescu/run/ptyd-1.sock`
  is 73 bytes for this home folder; the daemon checks the length and falls back to
  `/tmp/terminescu-<uid>/` with the same `0700` folder.
- Subscriber writer threads in the spike never ended, because the pane kept their senders.
  The real daemon drops a subscriber when its socket closes.
- The owner's zsh already emits OSC 133 (prompt marks), OSC 7 (cwd) and iTerm2's OSC 1337
  (`RemoteHost`, `CurrentDir`). The daemon reads OSC 7 and OSC 133 (D7) and passes OSC 1337
  through to the screen model, which ignores it.

## Caveat for task 2.1

The serializer here reached the inactive grid with `Term::swap_alt`. Swapping from the main
screen back to the alternate screen calls `reset_region` on the alternate grid, so a second
attach in a running daemon would erase Claude's screen. The daemon uses accessors on a vendored
copy instead (D31). The serializer also trims trailing blanks on wrapped rows, which rejoins
the next row too early; the real one keeps them.
