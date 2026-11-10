// The diff tabs' layout: side by side or unified, the same for every diff tab of every plugin,
// kept per Mac.

export type DiffMode = "side" | "unified";

export const DIFF_MODE_KEY = "terminescu.diffMode";
/** Sent on `window` when the layout changes, so every open diff tab follows. */
export const DIFF_MODE_EVENT = "terminescu-diff-mode";

export const parseDiffMode = (value: string | null | undefined): DiffMode => (value === "unified" ? "unified" : "side");

export const toggledDiffMode = (mode: DiffMode): DiffMode => (mode === "side" ? "unified" : "side");

type Store = Pick<Storage, "getItem" | "setItem">;

const storage = (): Store | undefined => {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
};

export function loadDiffMode(store: Store | undefined = storage()): DiffMode {
  try {
    return parseDiffMode(store?.getItem(DIFF_MODE_KEY));
  } catch {
    return "side";
  }
}

/** Keeps the layout and tells the open diff tabs. */
export function saveDiffMode(mode: DiffMode, store: Store | undefined = storage()) {
  try {
    store?.setItem(DIFF_MODE_KEY, mode);
  } catch {
    // Storage can be off; the layout then lasts until the window closes.
  }
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent<DiffMode>(DIFF_MODE_EVENT, { detail: mode }));
}
