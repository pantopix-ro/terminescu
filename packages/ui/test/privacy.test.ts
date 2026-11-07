import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "vitest";
import { PRIVACY_POINTS } from "../src/privacy";

// PRIVACY.md and Help › Privacy state the same facts (add-public-release task 4.3).
test("PRIVACY.md lists the app's privacy points, word for word", () => {
  const md = readFileSync(join(__dirname, "../../../PRIVACY.md"), "utf8");
  const bullets = md
    .split(/\n(?=- )/)
    .filter((b) => b.startsWith("- "))
    .map((b) => b.slice(2).split("\n\n")[0]!.replace(/\s+/g, " ").replaceAll("`", "").trim());
  expect(bullets).toEqual([...PRIVACY_POINTS]);
});
