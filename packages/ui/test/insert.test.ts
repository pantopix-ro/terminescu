import type { Pin, Variable } from "@terminescu/host";
import { describe, expect, it } from "vitest";
import { pasteBytes, renderPrompt, typeable, shellPaths } from "../src/insert";

const v = (name: string, mode: Variable["mode"], value: string | null): Variable => ({
  id: name,
  sideTabId: "st-1",
  name,
  value,
  mode,
  source: "typed",
  originTabId: null,
  expiresAt: null,
});

describe("pasteBytes", () => {
  it("plain insert writes no \\r, even for CRLF text", () => {
    for (const bracketed of [false, true]) {
      expect(pasteBytes("PROJ-482", bracketed)).not.toContain("\r");
      expect(pasteBytes("line one\r\nline two\rthree", bracketed)).not.toContain("\r");
    }
    expect(pasteBytes("PROJ-482", false)).toBe("PROJ-482");
  });

  it("brackets multi-line text even when the program did not ask, and strips paste markers", () => {
    expect(pasteBytes("a\nb", false)).toBe("\x1b[200~a\nb\x1b[201~");
    expect(pasteBytes("a\tb", false)).toBe("\x1b[200~a\tb\x1b[201~");
    expect(pasteBytes("x\x1b[201~rm -rf ~\n", true)).toBe("\x1b[200~xrm -rf ~\n\x1b[201~");
  });

  it("insert and send adds one \\r after the paste", () => {
    expect(pasteBytes("a\nb", true, true)).toBe("\x1b[200~a\nb\x1b[201~\r");
  });
});

describe("renderPrompt", () => {
  const pins = [{ exact: "Use the audit table.\nKeep it small." } as Pin];
  it("fills values, secrets as $NAME, pins as quotes, and lists what is missing", () => {
    const r = renderPrompt(
      "Fix {{ticket}} on {{ branch }} with {{API_TOKEN}}.\n{{pins}}\n{{owner}} {{owner}} {{not a name}}",
      [v("ticket", "saved", "PROJ-482"), v("branch", "session", "feature/audit-export"), v("API_TOKEN", "secret", null)],
      pins,
    );
    expect(r.text).toBe(
      "Fix PROJ-482 on feature/audit-export with $API_TOKEN.\n> Use the audit table.\n> Keep it small.\n{{owner}} {{owner}} {{not a name}}",
    );
    expect(r.missing).toEqual(["owner"]);
    expect(renderPrompt("{{owner}}", [], [], { owner: "acme" })).toEqual({ text: "acme", missing: [] });
  });
});

describe("save as prompt", () => {
  it("offers {{branch}} for the branch value, longest values first, secrets by $NAME", async () => {
    const { detectVariables, templatize } = await import("../src/detect");
    const vars = [
      v("branch", "session", "feature/audit-export"),
      v("feature", "saved", "feature"),
      v("id", "saved", "42"),
      v("API_TOKEN", "secret", null),
    ];
    const text = "Open a PR from feature/audit-export, the feature branch, with $API_TOKEN. 42";
    const found = detectVariables(text, vars);
    expect(found.map((f) => f.name)).toEqual(["branch", "API_TOKEN", "feature"]);
    expect(templatize(text, found)).toBe("Open a PR from {{branch}}, the {{feature}} branch, with {{API_TOKEN}}. 42");
    expect(templatize(text, found.filter((f) => f.name !== "feature"))).toBe(
      "Open a PR from {{branch}}, the feature branch, with {{API_TOKEN}}. 42",
    );
  });
});

describe("a paste end marker cannot be rebuilt", () => {
  it("drops every control but \\n and \\t, so a split marker stays plain text", () => {
    const evil = "echo hi\x1b[2\x1b[201~01~\nrm -rf ~/work\n";
    const bytes = pasteBytes(evil, false);
    expect(bytes.startsWith("\x1b[200~")).toBe(true);
    expect(bytes.endsWith("\x1b[201~")).toBe(true);
    // Only the outer markers hold an ESC.
    expect(bytes.split("\x1b").length - 1).toBe(2);
    expect(bytes).toBe("\x1b[200~echo hi\nrm -rf ~/work\n\x1b[201~");
    expect(typeable("a\x07b\x9bc\td\r\ne")).toBe("ab" + "c\td\ne");
  });
});

describe("shellPaths", () => {
  it("leaves plain paths and quotes the rest", () => {
    expect(shellPaths(["/Users/owner/acme-api/README.md"])).toBe("/Users/owner/acme-api/README.md ");
    expect(shellPaths(["/Users/owner/My Files/a b.txt", "/tmp/x"])).toBe("'/Users/owner/My Files/a b.txt' /tmp/x ");
    expect(shellPaths(["/tmp/it's.txt"])).toBe(`'/tmp/it'\\''s.txt' `);
    expect(shellPaths(["/tmp/$HOME;rm"])).toBe("'/tmp/$HOME;rm' ");
  });
});
