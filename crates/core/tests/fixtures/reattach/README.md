# Reattach fixture

`claude-session.bin` is the raw pty output of spike 0.1 (2026-11-04): a zsh login shell, `seq 1 120`,
then `claude` 2.1.318 answering one prompt, with resizes at the offsets in
`claude-session.events.json`. User, host and paths are replaced with invented ones.
Task 2.1 feeds it into the daemon's screen model and compares the snapshot with xterm.js headless
fed the same bytes.
