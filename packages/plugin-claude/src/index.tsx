// The Claude plugin's UI half (add-plugin-architecture 3.4): the transcript pane, read through
// the plugin's Rust half. The agent dot and the amber row stay the app's own.

import type { TranscriptChunk } from "@terminescu/host";
import { definePlugin, type PaneViewProps } from "@terminescu/plugin-api";
import { quoteOf } from "@terminescu/ui";
import { TranscriptPane } from "./TranscriptPane";
import "./transcript.css";

export { TranscriptPane, type Reveal, type TextEntry } from "./TranscriptPane";

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
  paneViews: [{ kind: "transcript", render: Transcript }],
});
