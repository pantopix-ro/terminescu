// The Claude plugin's UI half (add-plugin-architecture 3.4): the transcript pane, read through
// the plugin's Rust half, the todo mirror (add-claude-insights 2.2), what a session changed
// (3.2) and Settings › Claude (3.4). The agent dot and the
// amber row stay the app's own.

import type { TranscriptChunk } from "@terminescu/host";
import { definePlugin, type PaneViewProps } from "@terminescu/plugin-api";
import { quoteOf } from "@terminescu/ui";
import { openMemory, openUserMemory, resume, revealTranscripts } from "./actions";
import { CLAUDE_DIFF, ChangesSection, ClaudeDiffView } from "./ChangesSection";
import { SettingsPage } from "./SettingsPage";
import { TodoSection } from "./TodoSection";
import { TranscriptPane } from "./TranscriptPane";
import "./transcript.css";

export { TranscriptPane, type Reveal, type TextEntry } from "./TranscriptPane";
export { SettingsPage, typedNumber, type ClaudeSettings } from "./SettingsPage";
export { TodoSection, type Todo, type TodosAnswer } from "./TodoSection";
export {
  ChangesSection,
  changeMark,
  CLAUDE_DIFF,
  claudeDiff,
  ClaudeDiffView,
  parseClaudeDiff,
  splitUnder,
  type ClaudeChange,
  type ClaudeChanges,
  type ClaudeDiffSource,
} from "./ChangesSection";
export { openMemory, openUserMemory, resume, revealTranscripts, startedAt, type PastSession } from "./actions";

function Transcript({ paneId, source, bridge, app }: PaneViewProps) {
  return (
    <TranscriptPane
      paneId={paneId}
      session={source}
      read={(offset) => bridge.call<TranscriptChunk>("read", { session: source, offset })}
      pin={(entry, range) => {
        const q = range ? quoteOf(entry.text, ...range) : undefined;
        return app.pin({
          entryId: entry.id,
          exact: q ? q.exact : entry.text,
          prefix: q?.prefix ?? null,
          suffix: q?.suffix ?? null,
          start: range?.[0] ?? null,
          end: range?.[1] ?? null,
        });
      }}
      extract={app.extract}
      savePrompt={app.savePrompt}
      reveal={app.reveal}
      onFocus={app.focus}
    />
  );
}

export const claude = definePlugin({
  id: "claude",
  name: "Claude",
  paneViews: [
    { kind: "transcript", render: Transcript },
    { kind: CLAUDE_DIFF, render: ClaudeDiffView },
  ],
  panelSections: [
    { id: "todos", title: "Claude's plan", render: TodoSection },
    { id: "changes", title: "What Claude changed", render: ChangesSection },
  ],
  actions: [resume, openMemory, openUserMemory, revealTranscripts],
  settingsPage: SettingsPage,
});
