# SelectionToolbar

Floats above text selected in a terminal or transcript pane. It is how text leaves a session and lands in the side tab's context.

- Extract `⌘E`: saves the selection as a variable. The name is suggested from the content (`pr_api` for a GitHub PR URL in the api repo, `branch` for a branch name) and editable inline. `↵` saves.
- Pin `⌘⇧P`: saves the selection as a pin, linked back to the transcript entry when the pane is a Claude session.
- Send to…: pastes the selection into another tab's prompt, as bracketed paste, without pressing Enter.
- Copy `⌘C`: plain copy.
- Selection paints in `selection`. The toolbar sits 6px above the selection end and never covers the selected text.
