# Reattach fixture

`claude-session.bin` is the raw pty output of spike 0.1 (2026-11-04): a zsh login shell, `seq 1 120`,
then `claude` 2.1.318 answering one prompt, with resizes at the offsets in
`claude-session.events.json`. User, host and paths are replaced with invented ones.
`claude-session.snapshot.bin` is the daemon's snapshot after those bytes; regenerate it with
`UPDATE_SNAPSHOT=1 cargo test -p terminescu-ptyd golden`. `packages/ui` replays both files into xterm.js headless
and checks the screens match.
