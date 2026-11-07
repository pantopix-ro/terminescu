// Terminal links (task 8.49): email addresses and the menu each kind of link gets.
import { describe, expect, it } from "vitest";
import { findEmails, linkActions } from "../src/links";

describe("findEmails", () => {
  it("finds plain and mailto addresses, not URL user parts", () => {
    expect(findEmails("mail dev@example.com.").map((e) => e.text)).toEqual(["dev@example.com"]);
    expect(findEmails("to mailto:ops+alerts@acme.example.org now")).toEqual([{ text: "ops+alerts@acme.example.org", start: 10, end: 37 }]);
    expect(findEmails("https://user@example.com/x")).toEqual([]);
    expect(findEmails("git@github")).toEqual([]);
    // Package versions and git remotes are no addresses.
    expect(findEmails("added react@18.3.1 and pnpm@9.1.0")).toEqual([]);
    expect(findEmails("clone git@github.com:acme/api.git")).toEqual([]);
  });
});

describe("linkActions", () => {
  it("gives each kind its apps", () => {
    expect(linkActions("url").map((a) => a.id)).toEqual(["browser", "copy"]);
    expect(linkActions("path").map((a) => a.id)).toEqual(["text", "reveal", "default", "copy"]);
    expect(linkActions("email").map((a) => a.id)).toEqual(["mail", "copy"]);
  });
});
