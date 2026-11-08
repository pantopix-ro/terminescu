import { expect, test, type Page } from "@playwright/test";

// Task 8.47 on the mock host: Settings › Terminal › Colours picks a scheme, imports one from
// Terminal.app or a file, and sets the opacity; every terminal changes at once.

const option = (page: Page, key: string) =>
  page.evaluate((key) => [...(window as any).__terminescu.terminals.values()][0].options[key], key);
const background = async (page: Page) => ((await option(page, "theme")) as { background: string }).background;
const rootHas = (page: Page, attr: string) => page.evaluate((a) => document.documentElement.hasAttribute(a), attr);

async function openColours(page: Page) {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Comma");
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: "Terminal" }).click();
  return dialog;
}

/** How much the layers under the first terminal cover the desktop: 1 is solid, 0 is clear. */
const coverage = (page: Page, from = ".term-pane .xterm-viewport") =>
  page.evaluate((from) => {
    let clear = 1;
    for (let el: Element | null = document.querySelector(from); el; el = el.parentElement) {
      const m = getComputedStyle(el).backgroundColor.match(/[\d.]+/g)?.map(Number) ?? [];
      // rgb() and color(srgb …) list three channels, then the alpha when there is one.
      const alpha = m.length >= 4 ? m[3]! : m.length === 3 ? 1 : 0;
      clear *= 1 - alpha;
    }
    return 1 - clear;
  }, from);

test("a scheme and the opacity apply live, and Reset brings the defaults back", async ({ page }) => {
  const dialog = await openColours(page);
  const panel = await background(page);
  expect(await rootHas(page, "data-scheme")).toBe(false);
  // Imported shows only once something was imported.
  await expect(dialog.getByLabel("Colour theme").locator("option")).toHaveText(["Terminescu", "Terminal.app Basic", "Solarized Dark", "Solarized Light"]);

  await dialog.getByLabel("Colour theme").selectOption("solarizedDark");
  await expect.poll(() => background(page)).toBe("#002b36");
  expect(((await option(page, "theme")) as { red: string }).red).toBe("#dc322f");
  expect(await rootHas(page, "data-scheme")).toBe(true);

  await dialog.getByLabel("Terminal opacity").fill("80");
  // xterm stays clear and one layer paints the scheme at 80% (task 8.63): what covers the desktop
  // under a terminal, from xterm's viewport up to the page, adds up to 80%, not near 100%.
  await expect.poll(() => background(page)).toBe("rgba(0, 43, 54, 0)");
  await expect.poll(() => coverage(page)).toBeCloseTo(0.8, 2);
  expect(await option(page, "allowTransparency")).toBe(true);
  expect(await rootHas(page, "data-see-through")).toBe(true);
  // The sidebars share the opacity (task 8.64): a row, its name and the context panel read about
  // 80%, a row's light and tint add a few percent; a menu inside the sidebar stays solid.
  expect(await coverage(page, ".app-side .ao-side")).toBeCloseTo(0.8, 2);
  const row = await coverage(page, ".app-side .ao-st .t");
  expect(row).toBeGreaterThan(0.79);
  expect(row).toBeLessThan(0.87);
  await page.keyboard.press("Escape");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Backslash");
  await expect(page.locator("aside.ctx-pinned")).toBeVisible();
  expect(await coverage(page, "aside.ctx-pinned .ctx-scroll")).toBeCloseTo(0.8, 1);
  await page.getByRole("button", { name: "Choose side tab kind" }).click();
  expect(await page.locator(".app-side .tab-menu").evaluate((el) => getComputedStyle(el).backgroundColor)).toMatch(/^rgb\(/);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Meta+Backslash");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Comma");
  const dialog2 = page.getByRole("dialog");
  await dialog2.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: "Terminal" }).click();
  expect(await rootHas(page, "data-see-through")).toBe(true);

  await dialog2.getByRole("button", { name: "Reset to defaults" }).click();
  await expect.poll(() => background(page)).toBe(panel);
  expect(await option(page, "allowTransparency")).toBe(false);
  expect(await rootHas(page, "data-see-through")).toBe(false);
});

test("Import from Terminal.app takes its colours, font, cursor and opacity", async ({ page }) => {
  const dialog = await openColours(page);
  await dialog.getByRole("button", { name: "From Terminal.app" }).click();
  await expect(dialog.getByRole("status").filter({ hasText: "Imported Ocean" })).toHaveText(
    "Imported Ocean: colours, Menlo, 12 px, block cursor, 90% opacity.",
  );
  await expect.poll(() => background(page)).toBe("rgba(27, 43, 74, 0)");
  await expect.poll(() => coverage(page)).toBeCloseTo(0.9, 2);
  await expect(dialog.getByLabel("Colour theme")).toHaveValue("imported");
  expect(await option(page, "cursorStyle")).toBe("block");
  expect(await option(page, "fontFamily")).toMatch(/^"Menlo"/);

  // A file: an .itermcolors preset brings only colours; anything else is refused.
  await page.evaluate(() => (window as any).__terminescu.host.dialogAnswers.push("/Users/owner/themes/Harbor.itermcolors"));
  await dialog.getByRole("button", { name: "File…" }).click();
  await expect(dialog.getByRole("status").filter({ hasText: "Harbor" })).toHaveText("Imported Harbor: colours.");
  await page.evaluate(() => (window as any).__terminescu.host.dialogAnswers.push("/Users/owner/notes.txt"));
  await dialog.getByRole("button", { name: "File…" }).click();
  await expect(page.locator(".app-error")).toContainText("not a Terminal.app profile");
});
