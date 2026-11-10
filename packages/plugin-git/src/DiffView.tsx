// The diff tab (add-git-plugin 3.2): the shared diff pane over the working file, compared with
// the file at HEAD. It reads the base again when the Rust half reports new facts for its own
// repository, as after a commit (polish-git-plugin D3). A renamed file compares with its old
// path; a deleted one shows read-only (D7).

import { useMemo } from "react";
import type { PaneViewProps, TextView } from "@terminescu/plugin-api";
import { DiffPane, type DiffBase } from "@terminescu/ui";

export const DIFF_KIND = "git-diff";

/** What a diff tab compares: a file of the working tree at `root`. */
export interface DiffSource {
  root: string;
  /** Relative to the root. */
  path: string;
  /** The path at HEAD of a renamed file. */
  from?: string;
  /** The file is gone from the working tree. */
  deleted?: boolean;
}

export const diffView = (source: DiffSource): TextView => ({ kind: DIFF_KIND, source: JSON.stringify(source) });

export function parseSource(source: string): DiffSource | undefined {
  try {
    const s = JSON.parse(source) as Partial<DiffSource>;
    if (typeof s.root !== "string" || typeof s.path !== "string") return undefined;
    return {
      root: s.root,
      path: s.path,
      ...(typeof s.from === "string" && s.from ? { from: s.from } : {}),
      ...(s.deleted === true ? { deleted: true } : {}),
    };
  } catch {
    return undefined;
  }
}

/** The Rust half's `baseText` answer. */
export type BaseText = DiffBase;

const NOTES = { reading: "Reading HEAD…", created: "New file: not in HEAD.", changed: "Changes since HEAD." };

const trimmed = (root: string) => root.replace(/\/+$/, "");

/** A `facts` event for this repository, or for none (a pane that left it). */
export function factsFor(root: string, payload: unknown): boolean {
  const r = (payload as { root?: string | null } | null)?.root;
  return typeof r !== "string" || trimmed(r) === trimmed(root);
}

export function DiffView({ source, bridge, app }: PaneViewProps) {
  const src = useMemo(() => parseSource(source), [source]);
  return (
    <DiffPane
      readKey={source}
      read={() =>
        src ? bridge.call<BaseText>("baseText", { root: src.root, path: src.path, from: src.from ?? null }) : Promise.reject(new Error("the tab names no file"))
      }
      subscribe={(again) => bridge.on("facts", (p) => src && factsFor(src.root, p) && again())}
      notes={NOTES}
      renderText={app.renderText}
      deleted={src?.deleted === true}
    />
  );
}
