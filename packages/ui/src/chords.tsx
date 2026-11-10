// The chord each action has now (add-configurable-shortcuts D6), so a hint never names a chord
// the person moved or turned off. The app provides the bound chords; without a provider the
// defaults from actions.json read.

import { createContext, useContext } from "react";
import { ACTIONS, chordLabel } from "@terminescu/host";

/** An action's chord as actions.json spells it; undefined when it has none or is off. */
export type ChordOf = (id: string) => string | undefined;

const defaults: ChordOf = (id) => ACTIONS.find((a) => a.id === id)?.accel || undefined;

export const ChordContext = createContext<ChordOf>(defaults);

/** How `id`'s chord reads now, as ⌥⌘U; undefined when the action has none. */
export function useChord(id: string): string | undefined {
  const accel = useContext(ChordContext)(id);
  return accel ? chordLabel(accel) : undefined;
}

/** `text` with ` (⌥⌘U)` after it when `chord` is set. */
export const withChord = (text: string, chord: string | undefined) => (chord ? `${text} (${chord})` : text);

/** How each action's chord reads now, for a list that names several. */
export function useChordOf(): (id: string) => string | undefined {
  const of = useContext(ChordContext);
  return (id) => {
    const accel = of(id);
    return accel ? chordLabel(accel) : undefined;
  };
}

/** How `id`'s default chord reads, for code outside React. */
export const defaultChord = (id: string) => {
  const accel = defaults(id);
  return accel ? chordLabel(accel) : undefined;
};
