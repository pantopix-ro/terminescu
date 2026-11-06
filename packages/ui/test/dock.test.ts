import { describe, expect, it } from "vitest";
import { dockBox, dockZone } from "../src/dock";

// Task 8.32: the four triangles of a pane, its middle, and the half a dropped tab takes.

const box = { left: 100, top: 50, width: 400, height: 200 };

describe("dock zones", () => {
  it("picks the side whose triangle holds the point", () => {
    expect(dockZone(box, 480, 150)).toBe("right");
    expect(dockZone(box, 110, 150)).toBe("left");
    expect(dockZone(box, 300, 60)).toBe("up");
    expect(dockZone(box, 300, 240)).toBe("down");
    // Near a corner, the diagonal decides.
    expect(dockZone(box, 480, 240)).toBe("right");
    expect(dockZone(box, 140, 245)).toBe("down");
  });

  it("does nothing in the middle and outside the pane", () => {
    expect(dockZone(box, 300, 150)).toBe("center");
    expect(dockZone(box, 330, 170)).toBe("center");
    expect(dockZone(box, 90, 150)).toBeUndefined();
    expect(dockZone({ ...box, width: 0 }, 100, 50)).toBeUndefined();
  });

  it("previews the half the tab takes", () => {
    expect(dockBox(box, "right")).toEqual({ left: 300, top: 50, width: 200, height: 200 });
    expect(dockBox(box, "left")).toEqual({ left: 100, top: 50, width: 200, height: 200 });
    expect(dockBox(box, "up")).toEqual({ left: 100, top: 50, width: 400, height: 100 });
    expect(dockBox(box, "down")).toEqual({ left: 100, top: 150, width: 400, height: 100 });
  });
});
