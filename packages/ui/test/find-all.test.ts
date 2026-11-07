// Find in all panes (task 8.46): one hit per logical line, wrapped rows joined, a cap on hits.
import { describe, expect, it } from "vitest";
import { findAll, type Rows } from "../src/find-all";

const rows = (list: (string | [string, true])[]): Rows => ({
  length: list.length,
  row: (i) => {
    const r = list[i];
    if (r === undefined) return undefined;
    return typeof r === "string" ? { text: r, wrapped: false } : { text: r[0], wrapped: true };
  },
});

describe("findAll", () => {
  it("finds case-insensitively in each pane, joining wrapped rows", () => {
    const { hits, more } = findAll(
      [
        { paneId: "p1", rows: rows(["deploy acme/api", "ok", "re-dep", ["loy web  ", true]]) },
        { paneId: "p2", rows: rows(["DEPLOY done"]) },
      ],
      "deploy",
    );
    expect(more).toBe(false);
    expect(hits).toEqual([
      { paneId: "p1", rows: [0, 0], text: "deploy acme/api", at: 0 },
      { paneId: "p1", rows: [2, 3], text: "re-deploy web", at: 3 },
      { paneId: "p2", rows: [0, 0], text: "DEPLOY done", at: 0 },
    ]);
  });

  it("stops at the limit and says there are more", () => {
    const { hits, more } = findAll([{ paneId: "p", rows: rows(Array.from({ length: 5 }, () => "x")) }], "x", 3);
    expect(hits).toHaveLength(3);
    expect(more).toBe(true);
  });

  it("finds nothing for a blank query", () => {
    expect(findAll([{ paneId: "p", rows: rows(["a b"]) }], " ").hits).toEqual([]);
  });
});
