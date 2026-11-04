# PinCard

One pinned item in a side tab's context: a URL, a whole Claude message, any text selected inside a message, or a block of terminal output.

- A selection pin keeps the entry id, the character range and the quoted text with a few words around it, so it finds its place again in the transcript.

- Head: kind icon, a `pixel-label` kind (`Artifact`, `PR`, `Excerpt`, `Output`, `Url`), and a number. `⌥1` to `⌥9` copies that pin from anywhere in the side tab.
- Body: a `body-strong` title, then the link in `link` or the excerpt prefixed with a magenta `>`.
- Meta: origin (`auto` for extraction rules, else the tab and speaker) and time. A pin from a Claude session opens its transcript at that entry.
- Pins render into a prompt through `{{pins}}`.
