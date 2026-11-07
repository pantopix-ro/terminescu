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

/** SF Mono (task 8.24): WebKit names the system's mono font `ui-monospace`. */
const SYSTEM_MONO = 'ui-monospace, "SF Mono", SFMono-Regular, Menlo, monospace';

/** The font fields of the app settings that shape the terminal (task 8.36). */
export interface TerminalFontSettings {
  terminalFont: string;
  terminalFontSize: number;
  terminalLineHeight: number;
  terminalLetterSpacing: number;
  terminalFontWeight: number;
}

/** The xterm.js font options; the font in Settings › Terminal comes first, SF Mono after it. */
export function terminalFontOptions(s: TerminalFontSettings) {
  const weight = s.terminalFontWeight;
  return {
    fontFamily: s.terminalFont ? `"${s.terminalFont}", ${SYSTEM_MONO}` : SYSTEM_MONO,
    fontSize: s.terminalFontSize,
    lineHeight: s.terminalLineHeight / 100,
    letterSpacing: s.terminalLetterSpacing,
    fontWeight: weight,
    fontWeightBold: Math.min(900, weight + 300),
  };
}
export type TerminalFont = ReturnType<typeof terminalFontOptions>;

/** The behaviour fields of the app settings that shape the terminal (task 8.42). */
export interface TerminalBehaviourSettings {
  terminalCursor: "bar" | "block" | "underline";
  terminalCursorBlink: boolean;
  terminalScrollback: number;
  optionAsMeta: "both" | "left" | "right" | "off";
  bell: "off" | "visual" | "sound";
  copyOnSelect: boolean;
  pasteWarning: boolean;
  raiseContrast: boolean;
}

/** The xterm.js options among them; the rest the pane handles itself. */
export function terminalBehaviourOptions(b: TerminalBehaviourSettings) {
  return {
    cursorStyle: b.terminalCursor,
    cursorBlink: b.terminalCursorBlink,
    cursorInactiveStyle: "outline" as const,
    scrollback: b.terminalScrollback,
    macOptionIsMeta: b.optionAsMeta === "both",
    minimumContrastRatio: b.raiseContrast ? 4.5 : 1,
  };
}

/** As Terminal.app's Basic profile: a blinking bar, both Option keys as Meta, no bell. */
export const terminalBehaviour: TerminalBehaviourSettings = {
  terminalCursor: "bar",
  terminalCursorBlink: true,
  terminalScrollback: 10_000,
  optionAsMeta: "both",
  bell: "off",
  copyOnSelect: false,
  pasteWarning: true,
  raiseContrast: false,
};

/** The defaults match Terminal.app's Basic profile: SF Mono Regular 11, line spacing 1.0. */
export const terminalFont: TerminalFont = terminalFontOptions({
  terminalFont: "",
  terminalFontSize: 11,
  terminalLineHeight: 100,
  terminalLetterSpacing: 0,
  terminalFontWeight: 400,
});
