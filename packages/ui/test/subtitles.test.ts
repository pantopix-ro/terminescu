import { describe, expect, it } from "vitest";
import type { Tab } from "@terminescu/host";
import { paneName, paneSubtitle, shortPath, subtitlePane, tabSubtitles, withLiveNames } from "../src/subtitles";

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
    // The tab is named "claude" (task 8.62), so its subtitle leaves the word out; a renamed tab keeps it.
    expect(tabSubtitles([tab], { t1: "p2" }, labels, { p2: running })).toEqual({ t1: "~/b · Plan" });
    expect(tabSubtitles([{ ...tab, title: "api" }], { t1: "p2" }, labels, { p2: running })).toEqual({ t1: "~/b · claude · Plan" });
    expect(tabSubtitles([tab], {}, labels, {})).toEqual({ t1: "~/a" });
  });

  it("names a tab after Claude, else the running command, else the foreground process", () => {
    expect(paneName({ program: "npm", process: "node" }, false)).toBe("npm");
    expect(paneName({ process: "zsh" }, false)).toBe("zsh");
    expect(paneName({ process: "2.1.0" }, true)).toBe("claude");
    expect(paneName(undefined, false)).toBeUndefined();
    const tab: Tab = { id: "t1", title: null, layout: { type: "pane", id: "p1" } };
    const [side] = withLiveNames([{ tabs: [tab] }], {}, { p1: { process: "zsh" } }, {});
    expect(side!.tabs[0]).toMatchObject({ live: "zsh" });
  });

  it("names a Claude tab after its session and leaves a title that repeats it out of the subtitle", () => {
    // Task 8.88: `/rename`, else Claude's own name; `claude` before the session has one.
    expect(paneName({ process: "2.1.0" }, true, "billing")).toBe("billing");
    expect(paneName({ process: "2.1.0" }, true, null)).toBe("claude");
    expect(paneName({ program: "npm" }, false, "billing")).toBe("npm");
    const tab: Tab = { id: "t1", title: null, layout: { type: "pane", id: "p1" } };
    const agents = { p1: { ...running, name: "Fix PROJ-482" } };
    const labels = { p1: { path: "~/work/api", title: "Fix PROJ-482" } };
    expect(withLiveNames([{ tabs: [tab] }], {}, labels, agents)[0]!.tabs[0]).toMatchObject({ live: "Fix PROJ-482" });
    expect(tabSubtitles([tab], {}, labels, agents)).toEqual({ t1: "~/work/api" });
    // A title that says something else stays, and a tab the person named keeps it too.
    expect(tabSubtitles([tab], {}, { p1: { ...labels.p1, title: "Retry webhooks" } }, agents)).toEqual({ t1: "~/work/api · Retry webhooks" });
    expect(tabSubtitles([{ ...tab, title: "mine" }], {}, labels, agents)).toEqual({ t1: "~/work/api · claude · Fix PROJ-482" });
  });
});
