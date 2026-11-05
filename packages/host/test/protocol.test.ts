import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { PatchBatch, Snapshot, Workspace } from "../src/protocol";
import { applyBatch, PatchError } from "../src/reducer";

const fixture = JSON.parse(
  readFileSync(new URL("../fixtures/host-protocol.json", import.meta.url), "utf8"),
) as { start: Snapshot; batches: PatchBatch[]; end: Workspace };

describe("host protocol fixture", () => {
  it("turns start plus batches into end, like the Rust core", () => {
    const end = fixture.batches.reduce((ws, b) => applyBatch(ws, b), fixture.start.workspace);
    expect(end).toEqual(fixture.end);
  });

  it("leaves the input workspace untouched", () => {
    const before = structuredClone(fixture.start.workspace);
    applyBatch(fixture.start.workspace, fixture.batches[0]!);
    expect(fixture.start.workspace).toEqual(before);
  });

  it("rejects a patch that does not fit", () => {
    const bad: PatchBatch = { rev: 2, patches: [{ type: "TabClosed", tabId: "nope" }] };
    expect(() => applyBatch(fixture.start.workspace, bad)).toThrow(PatchError);
  });
});
