/** The DOM events the app sends to the focused pane for `⌘⇧P` and `⌘E` (D12, D13). */
export const PIN_EVENT = "terminescu:pin";
export const EXTRACT_EVENT = "terminescu:extract";
/** A `CustomEvent<string>` whose text the focused terminal pastes, with no Enter (D13, D21). */
export const INSERT_EVENT = "terminescu:insert";
