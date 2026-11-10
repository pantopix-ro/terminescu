// The diff tab (add-git-plugin 3.2): the shared diff pane over the working file, compared with
// the file at HEAD. It reads the base again when the Rust half reports new facts, as after a
// commit.

import { useMemo } from "react";
import type { PaneViewProps, TextView } from "@terminescu/plugin-api";
import { DiffPane, type DiffBase } from "@terminescu/ui";

export const DIFF_KIND = "git-diff";

/** What a diff tab compares: a file of the working tree at `root`. */
export interface DiffSource {
  root: string;
  /** Relative to the root. */
  path: string;
}

export const diffView = (source: DiffSource): TextView => ({ kind: DIFF_KIND, source: JSON.stringify(source) });

export function parseSource(source: string): DiffSource | undefined {
  try {
    const s = JSON.parse(source) as Partial<DiffSource>;
    return typeof s.root === "string" && typeof s.path === "string" ? { root: s.root, path: s.path } : undefined;
  } catch {
    return undefined;
  }
}

/** The Rust half's `baseText` answer. */
export type BaseText = DiffBase;

const NOTES = { reading: "Reading HEAD…", created: "New file: not in HEAD.", changed: "Changes since HEAD." };

export function DiffView({ source, bridge, app }: PaneViewProps) {
  const src = useMemo(() => parseSource(source), [source]);
  return (
    <DiffPane
      readKey={source}
      read={() => (src ? bridge.call<BaseText>("baseText", { root: src.root, path: src.path }) : Promise.reject(new Error("the tab names no file")))}
      subscribe={(again) => bridge.on("facts", again)}
      notes={NOTES}
      renderText={app.renderText}
    />
  );
}
