import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { colors } from "../src/generated/colors";
import { ANSI_SLOTS, xtermTheme } from "../src/xterm-theme";

const css = readFileSync(new URL("../src/generated/tokens.css", import.meta.url), "utf8");
const cssVars = Object.fromEntries(
  [...css.matchAll(/--([\w-]+):\s*([^;]+);/g)].map((m) => [m[1]!, m[2]!.trim()]),
);
function cssColor(name: string): string {
  const v = cssVars[name];
  if (v === undefined) throw new Error(`no --${name} in tokens.css`);
  const alias = /^var\(--([\w-]+)\)$/.exec(v);
  return alias ? cssColor(alias[1]!) : v;
}

describe("tokens", () => {
  it("gives the xterm theme the 16 ANSI colors of the CSS tokens", () => {
    ANSI_SLOTS.forEach((slot, i) => {
      expect(xtermTheme[slot], slot).toBe(cssColor(`ansi-${i}`));
    });
  });

  it("keeps the generated colors in step with tokens.json", () => {
    const json = JSON.parse(
      readFileSync(new URL("../../../design/system/project/tokens.json", import.meta.url), "utf8"),
    ) as { color: { tokens: { name: string }[] } };
    expect(Object.keys(colors)).toEqual(json.color.tokens.map((t) => t.name));
    for (const name of Object.keys(colors) as (keyof typeof colors)[]) {
      expect(colors[name], name).toBe(cssColor(name));
    }
  });

  it("loads nothing from the network", () => {
    for (const file of ["tokens.css", "components.css"]) {
      const text = readFileSync(new URL(`../src/generated/${file}`, import.meta.url), "utf8");
      expect(text, file).not.toMatch(/@import\s+url\(["']?https?:|url\(["']?https?:/);
    }
  });

  it("never uses amber for terminal yellow (D16)", () => {
    expect(xtermTheme.yellow).not.toBe(colors.amber);
    expect(xtermTheme.brightYellow).not.toBe(colors.amber);
  });
});
