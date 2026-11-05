import { describe, expect, it } from "vitest";
import { varOptions, varRefs } from "../src/varchips";

const vars = [
  { name: "ticket", mode: "saved" },
  { name: "API_TOKEN", mode: "secret" },
];

describe("variable chips", () => {
  it("flags a misspelt name and keeps the known one a chip", () => {
    const refs = varRefs("Fix {{ticket}} before {{tikcet}} and {{API_TOKEN}}, {{pins}}", vars);
    expect(refs.map((r) => [r.name, r.kind])).toEqual([
      ["ticket", "known"],
      ["tikcet", "unknown"],
      ["API_TOKEN", "secret"],
      ["pins", "known"],
    ]);
    expect(refs[1]).toMatchObject({ from: 22, to: 32 });
  });

  it("offers a secret only as $NAME", () => {
    const opts = varOptions(vars);
    expect(opts.map((o) => o.apply)).toEqual(["{{ticket}}", "$API_TOKEN", "{{pins}}"]);
    expect(opts.some((o) => o.apply.includes("{{API_TOKEN"))).toBe(false);
  });
});
