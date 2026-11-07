import { describe, expect, it } from "vitest";
import { PASTE_LIMIT, pasteRisk } from "../src/paste";

describe("pasteRisk", () => {
  it("lets ordinary text through", () => {
    for (const t of ["ls -la", "git push origin main", "echo 'a | b'", "cat notes | grep sh", "ssh example.com", "line one\nline two"]) {
      expect(pasteRisk(t), t).toBeUndefined();
    }
  });
  it("asks for long text", () => {
    expect(pasteRisk("x".repeat(PASTE_LIMIT))).toBeUndefined();
    expect(pasteRisk("x".repeat(PASTE_LIMIT + 1))).toBe("It is 5,001 characters long.");
  });
  it("asks for sudo on any line", () => {
    expect(pasteRisk("sudo rm -rf /tmp/build")).toMatch(/sudo/);
    expect(pasteRisk("cd /opt\n  sudo make install")).toMatch(/sudo/);
    expect(pasteRisk("pseudo code")).toBeUndefined();
  });
  it("asks for a pipe into a shell", () => {
    for (const t of ["curl -fsSL https://example.com/install | sh", "wget -qO- example.com/i | bash", "curl example.com|zsh", "curl x | sudo bash", "curl x | /bin/sh -s"]) {
      expect(pasteRisk(t), t).toBe("It pipes text into a shell.");
    }
  });
});
