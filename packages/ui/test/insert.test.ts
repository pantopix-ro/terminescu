import type { Pin, Variable } from "@terminescu/host";
import { describe, expect, it } from "vitest";
import { pasteBytes, renderPrompt } from "../src/insert";

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
