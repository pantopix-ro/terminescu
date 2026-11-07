import { DEFAULT_SETTINGS, type TerminalKeys, type Tint } from "@terminescu/host";
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

/** The colours of an imported profile, as `term_import::Colors` in the core. */
export interface SchemeColors {
  foreground: string;
  background: string;
  cursor?: string | null;
  selection?: string | null;
  bold?: string | null;
  /** 16 `#rrggbb` values, normal 0-7 then bright 8-15. */
  ansi: string[];
}

export type TerminalThemeName = "terminescu" | "basic" | "solarizedDark" | "solarizedLight" | "imported";

const SOLARIZED_ANSI = [
  "#073642", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#2aa198", "#eee8d5",
  "#002b36", "#cb4b16", "#586e75", "#657b83", "#839496", "#6c71c4", "#93a1a1", "#fdf6e3",
];

/** The built-in schemes (task 8.47): Terminal.app's Basic profile and Solarized. */
const SCHEMES: Record<Exclude<TerminalThemeName, "terminescu" | "imported">, SchemeColors> = {
  basic: {
    foreground: "#000000",
    background: "#ffffff",
    cursor: "#7f7f7f",
    selection: "#b4d5fe",
    // As term_import's DEFAULT_ANSI.
    ansi: [
      "#000000", "#990000", "#00a600", "#999900", "#0000b2", "#b200b2", "#00a6b2", "#bfbfbf",
      "#666666", "#e50000", "#00d900", "#e5e500", "#0000ff", "#e500e5", "#00e5e5", "#e5e5e5",
    ],
  },
  solarizedDark: { foreground: "#839496", background: "#002b36", cursor: "#93a1a1", selection: "#073642", ansi: SOLARIZED_ANSI },
  solarizedLight: { foreground: "#657b83", background: "#fdf6e3", cursor: "#586e75", selection: "#eee8d5", ansi: SOLARIZED_ANSI },
};

const HEX = /^#[0-9a-f]{6}$/i;

function fromScheme(c: SchemeColors): ITheme {
  const t: ITheme = {
    background: c.background,
    foreground: c.foreground,
    cursor: c.cursor ?? c.foreground,
    cursorAccent: c.background,
    selectionBackground: c.selection ?? colors.selection,
  };
  ANSI_SLOTS.forEach((slot, i) => (t[slot] = c.ansi[i]));
  return t;
}

/** The xterm theme for a scheme name; Imported without valid colours falls back to the app's. */
export function schemeTheme(name: TerminalThemeName, imported?: SchemeColors | null): ITheme {
  if (name === "imported") {
    const ok = imported && imported.ansi.length === 16 && [imported.foreground, imported.background, ...imported.ansi].every((c) => HEX.test(c));
    return ok ? fromScheme(imported) : xtermTheme;
  }
  return name === "terminescu" ? xtermTheme : fromScheme(SCHEMES[name]);
}

const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as [number, number, number];
const toHex = (c: number[]) => "#" + c.map((v) => Math.round(v).toString(16).padStart(2, "0")).join("");

/** A light background (Basic, Solarized Light) by its relative luminance. */
export function isLight(hex: string): boolean {
  if (!HEX.test(hex)) return false;
  const [r, g, b] = rgb(hex);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b > 140;
}

/** How much of a status tint a light scheme takes: the tints are dark, full strength hides text. */
export const LIGHT_TINT = 0.35;

/** `#rrggbb` mixed with `over` at `share` (0 to 1), as CSS color-mix in sRGB. */
export function mix(base: string, over: string, share: number): string {
  if (!HEX.test(base) || !HEX.test(over)) return base;
  const a = rgb(base);
  const b = rgb(over);
  return toHex(a.map((v, i) => v + (b[i]! - v) * share));
}

/** `#rrggbb` at `percent` opacity as rgba(), or the hex itself at 100. */
export function withOpacity(hex: string, percent: number): string {
  if (percent >= 100 || !HEX.test(hex)) return hex;
  const [r, g, b] = rgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${percent / 100})`;
}

/**
 * The theme for a pane (tasks 8.14, 8.47): the scheme, with a status tint replacing its
 * background so a side tab's terminals read as one group (a light scheme takes part of the
 * tint), and the window opacity on that background. Text colours stay.
 */
export function themeFor(tint?: Tint | null, base: ITheme = xtermTheme, opacity = 100): ITheme {
  if (!tint && opacity >= 100) return base;
  const bg = base.background ?? colors.panel;
  const tinted = !tint ? bg : isLight(bg) ? mix(bg, colors[`tint-${tint}`], LIGHT_TINT) : colors[`tint-${tint}`];
  return { ...base, background: withOpacity(tinted, opacity), cursorAccent: tinted };
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
  /** Chords the pane turns into shell editing keys (task 8.53). */
  terminalKeys: TerminalKeys;
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
  terminalKeys: DEFAULT_SETTINGS.terminalKeys,
};

/** The defaults match Terminal.app's Basic profile: SF Mono Regular 11, line spacing 1.0. */
export const terminalFont: TerminalFont = terminalFontOptions({
  terminalFont: "",
  terminalFontSize: 11,
  terminalLineHeight: 100,
  terminalLetterSpacing: 0,
  terminalFontWeight: 400,
});
