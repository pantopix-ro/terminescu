# Design: harden-plugin-host

## Context

Rows close through `host_command`, which notes the close for ⇧⌘T (`reopen::note`) before it
runs. The runtime's own `pane_exited` calls `Runtime::command` and skips that. Plugins hear
`SideTabClosed` on their worker thread, after the save already deleted their row data.

## Decisions

### D1. The runtime hands exit closes to a sink

`Runtime::on_exit_close(sink)`. When set, `pane_exited` gives the sink `ClosePane` instead of
running it. The desktop sink is `host_command`'s note, run, settle. Rejected: teaching the
runtime about the closed list; it lives in the app.

### D2. The close event carries the plugin's own row data

`CoreEvent::SideTabClosed { side_tab, data }`, `data` a map of the plugin's row data keys. The
host fills it per plugin at dispatch, which runs inside the command before the save, through a
reader the app sets (`PluginHost::set_row_data`). Rejected: a "closed rows" table; the data
lives one event long.

### D3. Reopen

A row reopens on `tabBySide[row]` when that tab came back, else its first tab. A failed reopen
puts its entry back and the error reads "Reopen: <why>". A second failure in a row drops the
entry, so one that can never come back does not block the older ones. The list is an `Arc<Mutex<ClosedList>>`
in `AppState`, shared with the save closure that asks `is_parked`.

### D4. A cap of 4 calls per plugin

Each slot counts calls whose thread still runs. A call over the cap returns
`PluginError::Busy` at once and counts as no failure. The count drops when the thread ends, not
at the budget, so a hung call holds its place. Rejected: a call queue, which deadlocks calls
that wait on each other.

### D5. Due timers first

The worker takes a due timer before the next event. A plugin that schedules 1 s ticks now gets
them on time under a flood of output events.

### D6. The pane view tag lives on the document

`DocView.view: Option<{kind, source}>`, saved in `documents.view_json` (migration). `openFile`
takes the tag; an open without one clears it. The app reads tags from the documents, not from
React state.

### D7. Pane bars are a plugin slot

`UiPlugin.paneBar?: ComponentType<PaneBarProps>`, drawn above each terminal pane by every
enabled plugin. Props: pane id, program, resume offer, the app's hook setting, and `resume`,
`dismiss`, `setHook`. The Claude plugin draws both bars. With it off, no bar shows.

## Risks / Trade-offs

- [The exit sink runs on the daemon thread] → it takes the same locks as `host_command`, in
  the same order; a test closes a row by exit.
- [A busy refusal reaches the UI] → the plugin's half reads it as an error, as today's timeout.
- [Migration] → one nullable column; old rows read as untagged.

## Review fixes
- A worker runs at most one due timer between two events while events wait, so a timer that sets itself again cannot starve the queue.
- ⇧⌘T counts only windows that still have a view; a window closed with the red button gets no reopen, the key's window does.
- A successful reopen clears the failure count, so an entry drops only after two failures in a row.
