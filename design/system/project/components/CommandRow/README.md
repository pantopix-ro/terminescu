# CommandRow

A saved shell command in the sidebar, under the side tabs. The side tab's own commands come first, then global ones from the library.

- Text may hold `{{variables}}` from the side tab's context; they resolve when the command runs.
- The hint on the right is the default target: `new tab`, `pane` (the focused pane) or `side tab`. Click runs there; `⌥`-click asks.
- Run sends Enter. Insert (`⌘I` on the row) pastes the command without Enter.
- A launcher side tab is a side tab made of commands: it runs them in its tabs when it opens or after a reboot, shows a restart action, and takes its status from those processes (`2/2 up`).
