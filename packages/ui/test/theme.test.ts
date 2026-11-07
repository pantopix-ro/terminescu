// Colour schemes and opacity (task 8.47): a tint replaces a dark scheme's background, mixes into
// a light one, and the opacity goes on the result.
import { describe, expect, it } from "vitest";
import { colors } from "../src/generated/colors";
import { isLight, mix, schemeTheme, themeFor, withOpacity, xtermTheme } from "../src/xterm-theme";

describe("themes", () => {
  it("keeps the app's theme by default and for Imported without valid colours", () => {
    expect(themeFor()).toBe(xtermTheme);
    expect(schemeTheme("terminescu")).toBe(xtermTheme);
    expect(schemeTheme("imported", { foreground: "#fff", background: "#000000", ansi: [] })).toBe(xtermTheme);
  });

  it("tints a dark scheme fully and a light one in part", () => {
    const dark = schemeTheme("solarizedDark");
    expect(themeFor("blue", dark).background).toBe(colors["tint-blue"]);
    const light = schemeTheme("basic");
    expect(isLight(light.background!)).toBe(true);
    expect(themeFor("blue", light).background).toBe(mix("#ffffff", colors["tint-blue"], 0.35));
  });

  it("puts the opacity on the background only", () => {
    const t = themeFor(null, schemeTheme("solarizedLight"), 50);
    expect(t.background).toBe("rgba(253, 246, 227, 0.5)");
    expect(t.foreground).toBe("#657b83");
    expect(withOpacity("#102030", 100)).toBe("#102030");
  });
});
