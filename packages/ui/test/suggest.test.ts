import { describe, expect, it } from "vitest";
import { insertText, parseTtl, suggestName, timeLeft, ttlText } from "../src/names";

describe("suggestName", () => {
  it("names a PR URL after its repo and spots branches and links", () => {
    expect(suggestName("https://github.com/acme/api/pull/918")).toBe("pr_api");
    expect(suggestName(" https://github.com/acme/Web-App/pull/12 ")).toBe("pr_web_app");
    expect(suggestName("feature/audit-export")).toBe("branch");
    expect(suggestName("https://example.com/a")).toBe("url");
    expect(suggestName("batches of 5,000 rows")).toBe("text");
  });
  it("calls the first artifact plan and the next ones artifact_n", () => {
    const url = "https://claude.ai/artifact/00000000-0000-4000-8000-000000000001";
    expect(suggestName(url)).toBe("plan");
    expect(suggestName(url, ["plan"])).toBe("artifact");
    expect(suggestName(url, ["plan", "artifact"])).toBe("artifact_2");
  });
  it("never suggests a name already in the side tab", () => {
    expect(suggestName("https://github.com/acme/api/pull/919", ["pr_api"])).toBe("pr_api_2");
    expect(suggestName("hotfix/x", ["branch", "branch_2"])).toBe("branch_3");
  });
});

describe("parseTtl and timeLeft", () => {
  it("reads 30m, 1h and 2d and refuses the rest", () => {
    expect(parseTtl("30m")).toBe(1800);
    expect(parseTtl(" 1h ")).toBe(3600);
    expect(parseTtl("2d")).toBe(172_800);
    for (const bad of ["", "0m", "1 hour", "-1h", "1.5h"]) expect(parseTtl(bad)).toBeUndefined();
  });
  it("counts down in the largest unit", () => {
    const now = 1_000_000;
    expect(timeLeft(now + 52 * 60_000, now)).toBe("52m left");
    expect(timeLeft(now + 20_000, now)).toBe("20s left");
    expect(timeLeft(now - 5, now)).toBe("0s left");
  });
});

describe("insertText", () => {
  it("types a secret as $NAME and never its value; drops a trailing line break", () => {
    expect(insertText({ name: "API_TOKEN", mode: "secret", value: null })).toBe("$API_TOKEN");
    expect(insertText({ name: "API_TOKEN", mode: "secret", value: "tok-example" })).toBe("$API_TOKEN");
    expect(insertText({ name: "branch", mode: "session", value: "feature/audit-export\n" })).toBe("feature/audit-export");
    expect(insertText({ name: "ticket", mode: "saved", value: "PROJ-482" })).toBe("PROJ-482");
  });
});

describe("ttlText", () => {
  it("writes the largest whole unit, and parseTtl reads it back", () => {
    for (const [secs, text] of [[172_800, "2d"], [3600, "1h"], [2700, "45m"], [90, "90s"]] as const) {
      expect(ttlText(secs)).toBe(text);
      expect(parseTtl(text)).toBe(secs);
    }
  });
});
