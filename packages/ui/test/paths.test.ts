import { describe, expect, it } from "vitest";
import { rankFiles } from "../src/files";
import { findPaths } from "../src/paths";

describe("findPaths", () => {
  const paths = (line: string) => findPaths(line).map((p) => p.path);

  it("finds paths with a folder or an extension and drops line numbers", () => {
    expect(paths("error in src/main.rs:12:4: expected ;")).toEqual(["src/main.rs"]);
    expect(paths("modified:   ./README.md")).toEqual(["./README.md"]);
    expect(paths("see ~/notes.md, and /etc/hosts.")).toEqual(["~/notes.md", "/etc/hosts"]);
    expect(paths("wrote (packages/ui/src/files.ts)")).toEqual(["packages/ui/src/files.ts"]);
  });

  it("leaves out URLs, versions, fractions and plain words", () => {
    expect(paths("https://example.com/a.md v1.2.3 1/2 done ... ok")).toEqual([]);
  });

  it("gives offsets that cover the path and its line number", () => {
    const line = "  at src/app.ts:3";
    const [p] = findPaths(line);
    expect(line.slice(p!.start, p!.end)).toBe("src/app.ts:3");
  });
});

describe("rankFiles", () => {
  const files = ["docs/guide/readme-notes.txt", "README.md", "src/reader.ts", "src/main.rs", "packages/ui/src/TextPane.tsx"];

  it("puts file names that start with the query first", () => {
    expect(rankFiles(files, "rea").slice(0, 3)).toEqual(["README.md", "src/reader.ts", "docs/guide/readme-notes.txt"]);
  });

  it("matches letters in order and every word", () => {
    expect(rankFiles(files, "txp")).toEqual(["packages/ui/src/TextPane.tsx"]);
    expect(rankFiles(files, "src main")).toEqual(["src/main.rs"]);
  });

  it("keeps the order for an empty query and caps the list", () => {
    expect(rankFiles(files, " ", 2)).toEqual(files.slice(0, 2));
  });
});
