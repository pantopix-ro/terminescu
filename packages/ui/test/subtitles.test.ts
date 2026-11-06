import { describe, expect, it } from "vitest";
import type { Tab } from "@terminescu/host";
import { paneSubtitle, shortPath, subtitlePane, tabSubtitles } from "../src/subtitles";

// Task 8.30: a tab's subtitle follows its focused terminal.

const running = { session: "s-1", state: "running", lastMessage: null } as const;

describe("tab subtitles", () => {
  it("keeps the path and adds the program, or claude and its title", () => {
    expect(paneSubtitle({ path: "~/work/api", program: "npm" }, false)).toBe("~/work/api · npm");
    expect(paneSubtitle({ path: "~/work/api", program: "claude", title: "Fix PROJ-482" }, true)).toBe("~/work/api · claude · Fix PROJ-482");
    expect(paneSubtitle({ path: "~/work/api", program: "claude" }, true)).toBe("~/work/api · claude");
    expect(paneSubtitle({ path: "~" }, false)).toBe("~");
    expect(paneSubtitle(undefined, false)).toBeUndefined();
  });

  it("shortens a long path from the left, keeping its last folders", () => {
    expect(shortPath("~/work/api")).toBe("~/work/api");
    expect(shortPath("~/work/clients/acme/services/billing-api", 28)).toBe("…/acme/services/billing-api");
    expect(shortPath("~/x/" + "a".repeat(40), 10)).toBe("…aaaaaaaaa");
  });

  it("follows the focused terminal, else the first one", () => {
    const tab: Tab = {
      id: "t1",
      title: null,
      layout: {
        type: "split",
        id: "sp",
        dir: "row",
        sizes: [1, 1, 1],
        children: [
          { type: "pane", id: "doc", doc: "d1" },
          { type: "pane", id: "p1" },
          { type: "pane", id: "p2" },
        ],
      },
    };
    expect(subtitlePane(tab, "p2")).toBe("p2");
    expect(subtitlePane(tab, "doc")).toBe("p1");
    expect(subtitlePane(tab, undefined)).toBe("p1");
    const labels = { p1: { path: "~/a" }, p2: { path: "~/b", program: "claude", title: "Plan" } };
    expect(tabSubtitles([tab], { t1: "p2" }, labels, { p2: running })).toEqual({ t1: "~/b · claude · Plan" });
    expect(tabSubtitles([tab], {}, labels, {})).toEqual({ t1: "~/a" });
  });
});
