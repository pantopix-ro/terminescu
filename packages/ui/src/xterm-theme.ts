import type { Tint } from "@terminescu/host";
import type { ITheme } from "@xterm/xterm";
import { colors } from "./generated/colors";

/** The xterm.js theme, built from the same tokens as the CSS (D16). */
export const xtermTheme: ITheme = {
  background: colors.panel,
  foreground: colors.fg,
  cursor: colors.fg,
  cursorAccent: colors.panel,
  selectionBackground: colors.selection,
  black: colors["ansi-0"],
  red: colors["ansi-1"],
  green: colors["ansi-2"],
  yellow: colors["ansi-3"],
  blue: colors["ansi-4"],
  magenta: colors["ansi-5"],
  cyan: colors["ansi-6"],
  white: colors["ansi-7"],
  brightBlack: colors["ansi-8"],
  brightRed: colors["ansi-9"],
  brightGreen: colors["ansi-10"],
  brightYellow: colors["ansi-11"],
  brightBlue: colors["ansi-12"],
  brightMagenta: colors["ansi-13"],
  brightCyan: colors["ansi-14"],
  brightWhite: colors["ansi-15"],
};

/**
 * The theme for a pane whose status has a background (task 8.14): the status tint replaces the
 * panel colour, so a side tab's terminals read as one group. Text colours stay.
 */
export function themeFor(tint?: Tint | null): ITheme {
  if (!tint) return xtermTheme;
  const background = colors[`tint-${tint}`];
  return { ...xtermTheme, background, cursorAccent: background };
}

/** xterm's 16 ANSI slots in index order. */
export const ANSI_SLOTS = [
  "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow",
  "brightBlue", "brightMagenta", "brightCyan", "brightWhite",
] as const satisfies readonly (keyof ITheme)[];

export const terminalFont = {
  fontFamily: '"Geist Mono", ui-monospace, "SF Mono", Menlo, monospace',
  fontSize: 13,
  lineHeight: 18 / 13,
} as const;
