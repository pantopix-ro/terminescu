import { describe, expect, it } from "vitest";
import { suggestRename } from "../src/protocol";

describe("suggestRename (D5)", () => {
  it("takes the ticket number from the source's name", () => {
    expect(suggestRename("branch", "PROJ-477", new Set())).toBe("branch_477");
  });
  it("falls back to _2 and skips names in use", () => {
    expect(suggestRename("branch", "audit export", new Set())).toBe("branch_2");
    expect(suggestRename("branch", null, new Set(["branch_2", "branch_2_2"]))).toBe("branch_2_3");
    expect(suggestRename("branch", "PROJ-477", new Set(["branch_477"]))).toBe("branch_477_2");
  });
});
