import { describe, expect, it } from "vitest";
import { createMockHost } from "../src/mock";
import { keyOf } from "../src/prompt-store";
import { variablesOf } from "../src/protocol";

// The mock's library keeps the core's rules (crates/core/tests/prompts.rs), so the app's
// Playwright runs see the same behaviour as the real one.
describe("prompt library on the mock host", () => {
  it("lists variables once and derives keys like the core", () => {
    expect(variablesOf("{{ticket}} on {{ branch }}, again {{ticket}}; {{not a name}} {{pins}}")).toEqual(["ticket", "branch", "pins"]);
    expect(keyOf("Review the plan!")).toBe("review-the-plan");
    expect(keyOf("  ??  ")).toBe("prompt");
  });

  it("searches tags, names and bodies, and restores into a new version", async () => {
    const host = createMockHost();
    const keys = async (q: string, archived = false) => (await host.searchPrompts(q, archived)).map((p) => p.key);
    expect(await keys("#plan")).toEqual(["review-plan"]);
    expect(await keys("CONFIDENCE")).toEqual(["review-plan"]);
    expect(await keys("develop #git")).toEqual(["open-prs"]);
    expect(await keys("#pla")).toEqual([]);

    const [p] = await host.searchPrompts("#plan");
    await host.savePrompt(p!.id, "v2");
    await host.savePrompt(p!.id, "v3");
    const restored = await host.restorePrompt(p!.id, 1);
    expect(restored.version).toBe(4);
    expect(restored.body).toBe(p!.body);
    expect((await host.promptVersions(p!.id)).map((v) => v.version)).toEqual([4, 3, 2, 1]);

    const copy = await host.duplicatePrompt(p!.id);
    expect([copy.key, copy.name, copy.version]).toEqual(["review-plan-copy", "Review plan copy", 1]);
    await host.archivePrompt(copy.id, true);
    expect(await keys("copy")).toEqual([]);
    expect(await keys("copy", true)).toEqual(["review-plan-copy"]);
  });
});
