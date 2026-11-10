// The diff tab's layout: side by side or unified, the same for every diff tab, kept per Mac.

export type DiffMode = "side" | "unified";

export const MODE_KEY = "terminescu.git.diffMode";
/** Sent on `window` when the layout changes, so every open diff tab follows. */
export const MODE_EVENT = "terminescu-git-diff-mode";

export const parseMode = (value: string | null | undefined): DiffMode => (value === "unified" ? "unified" : "side");

export const toggled = (mode: DiffMode): DiffMode => (mode === "side" ? "unified" : "side");

type Store = Pick<Storage, "getItem" | "setItem">;

const storage = (): Store | undefined => {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
};

export function loadMode(store: Store | undefined = storage()): DiffMode {
  try {
    return parseMode(store?.getItem(MODE_KEY));
  } catch {
    return "side";
  }
}

/** Keeps the layout and tells the open diff tabs. */
export function saveMode(mode: DiffMode, store: Store | undefined = storage()) {
  try {
    store?.setItem(MODE_KEY, mode);
  } catch {
    // Storage can be off; the layout then lasts until the window closes.
  }
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent<DiffMode>(MODE_EVENT, { detail: mode }));
}
