export { colors, type ColorToken } from "./generated/colors";
export { ANSI_SLOTS, terminalFont, xtermTheme } from "./xterm-theme";
export { ContextPanel, HOVER_MS, type ContextMode, type ContextPanelProps } from "./ContextPanel";
export { focusPane } from "./pane-registry";
export { Palette, type PaletteItem, type PaletteProps } from "./Palette";
export {
  DROP_END,
  dropGap,
  dropInto,
  SideTabList,
  sideTabName,
  tabChord,
  tabName,
  type SideTabListProps,
} from "./SideTabList";
export { dragSizes, PaneSlot, SplitView, type SplitViewProps } from "./SplitView";
export { TabBar, type TabBarProps } from "./TabBar";
export { TerminalPane, type TerminalPaneProps } from "./TerminalPane";
export { WEBGL_LIMIT, webglBudget } from "./webgl-budget";
export { DOT, tabAgent, type Agents } from "./agents";
export { anchor, quoteOf, QUOTE_CONTEXT, type Quote } from "./anchor";
export { suggestName } from "./names";
export { EXTRACT_EVENT, PIN_EVENT, TranscriptPane, type Reveal, type TextEntry } from "./TranscriptPane";
