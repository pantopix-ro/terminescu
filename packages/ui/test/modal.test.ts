import { describe, expect, it } from "vitest";
import { nextStop } from "../src/modal";

describe("nextStop", () => {
  const stops = ["name", "value", "cancel", "add"];
  it("moves on and wraps at either end", () => {
    expect(nextStop(stops, "name", false)).toBe("value");
    expect(nextStop(stops, "add", false)).toBe("name");
    expect(nextStop(stops, "name", true)).toBe("add");
  });
  it("enters at the edge from outside the dialog", () => {
    expect(nextStop(stops, null, false)).toBe("name");
    expect(nextStop(stops, "terminal", true)).toBe("add");
  });
  it("has nowhere to go in a dialog with no stops", () => {
    expect(nextStop([], null, false)).toBeUndefined();
  });
});
