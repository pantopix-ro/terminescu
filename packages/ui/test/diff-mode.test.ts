import { describe, expect, it } from "vitest";
import { DIFF_MODE_KEY as MODE_KEY, loadDiffMode as loadMode, parseDiffMode as parseMode, saveDiffMode as saveMode, toggledDiffMode as toggled } from "../src/diff-mode";

const memory = () => {
  const data = new Map<string, string>();
  return { data, getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v) };
};

describe("diff layout", () => {
  it("reads side by side unless unified was kept", () => {
    expect(parseMode(null)).toBe("side");
    expect(parseMode("garbage")).toBe("side");
    expect(parseMode("unified")).toBe("unified");
  });

  it("toggles between the two layouts", () => {
    expect(toggled("side")).toBe("unified");
    expect(toggled(toggled("side"))).toBe("side");
  });

  it("keeps the layout under its key", () => {
    const store = memory();
    expect(loadMode(store)).toBe("side");
    saveMode(toggled(loadMode(store)), store);
    expect(store.data.get(MODE_KEY)).toBe("unified");
    expect(loadMode(store)).toBe("unified");
  });

  it("falls back to side by side when storage throws", () => {
    const broken = {
      getItem: () => {
        throw new Error("off");
      },
      setItem: () => {
        throw new Error("off");
      },
    };
    expect(loadMode(broken)).toBe("side");
    expect(() => saveMode("unified", broken)).not.toThrow();
    expect(loadMode(undefined)).toBe("side");
  });
});
