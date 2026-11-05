/** The DOM events the app sends to the focused pane for `⌘⇧P` and `⌘E` (D12, D13). */
export const PIN_EVENT = "terminescu:pin";
export const EXTRACT_EVENT = "terminescu:extract";
/** A `CustomEvent<InsertDetail>` whose text the focused terminal pastes (D13, D21). */
export const INSERT_EVENT = "terminescu:insert";

/** Text for `INSERT_EVENT`; Enter goes after it only with `send`. */
export interface InsertDetail {
  text: string;
  send?: boolean;
  /** Focus stays where it was, as for "Send to pane" from a text pane (D29). */
  stay?: boolean;
}
