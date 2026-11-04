# TextPane

A pane that edits one document: a file, an Untitled buffer, a context variable, a prompt version, or an editor request from another program. CodeMirror 6 underneath; the text on disk stays exactly as typed, and color comes from decorations.

- Lines (`.ao-ed-l`) with a gutter number and the text. `.cur` marks the cursor line.
- Markdown: markers in `faint`, the first heading in `blue`, code in `cyan`, list bullets in `magenta`.
- `{{variable}}` from the side tab's context renders as `.ao-vchip`. An unknown name gets `.unknown` (red wavy underline). A secret only ever appears as `$NAME` with `.secret`.
- `.ao-complete` lists context variables after `{{`, value previews dim, secrets offered as `$NAME`.
- The pane bar (`.ao-ed-bar`): unsaved state, line and column, encoding and line endings, the send action.
- `.ao-bar-info` (blue) says the file changed on disk, with Reload and Keep mine. `.ao-bar-wait` (amber) says a program waits for this text: `⌘↵` sends it back.
- `⌘S` saves; an Untitled buffer asks where. Unsaved text survives an app restart.
