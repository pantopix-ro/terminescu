import { expect, test, type Page } from "@playwright/test";

// Task 8.8 on the mock host: ⌘=, ⌘+ and ⌘- step the zoom of the whole UI, ⌘0 resets it, and
// the level lives in the settings. The mock zooms with CSS; the app zooms its webview.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
}

const zoomOf = (page: Page) => page.evaluate(async () => (await (window as any).__terminescu.host.settings()).zoomPercent);
const rootZoom = (page: Page) => page.evaluate(() => document.documentElement.style.zoom);

test("⌘= and ⌘+ zoom in, ⌘- zooms out, ⌘0 resets", async ({ page }) => {
  await start(page);
  expect(await zoomOf(page)).toBe(100);

  await page.keyboard.press("Meta+Equal");
  await expect.poll(() => zoomOf(page)).toBe(110);
  await expect(page.locator(".app-note")).toHaveText("Zoom 110%");
  expect(await rootZoom(page)).toBe("1.1");

  // ⌘+ is ⌘⇧= on the keyboard.
  await page.keyboard.press("Meta+Shift+Equal");
  await expect.poll(() => zoomOf(page)).toBe(125);

  await page.keyboard.press("Meta+Minus");
  await page.keyboard.press("Meta+Minus");
  await page.keyboard.press("Meta+Minus");
  await expect.poll(() => zoomOf(page)).toBe(90);

  await page.keyboard.press("Meta+0");
  await expect.poll(() => zoomOf(page)).toBe(100);
  expect(await rootZoom(page)).toBe("");
  // The level hides on its own.
  await expect(page.locator(".app-note")).toBeHidden({ timeout: 4000 });
});

test("the zoom stops at 50% and 200%", async ({ page }) => {
  await start(page);
  for (let i = 0; i < 8; i++) await page.keyboard.press("Meta+Equal");
  await expect.poll(() => zoomOf(page)).toBe(200);
  await page.keyboard.press("Meta+Equal");
  await expect.poll(() => zoomOf(page)).toBe(200);
  for (let i = 0; i < 12; i++) await page.keyboard.press("Meta+Minus");
  await expect.poll(() => zoomOf(page)).toBe(50);
});

test("the zoom chords show in Settings › Keys like any action", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Comma");
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: "Keys" }).click();
  await expect(dialog.locator('[data-action="zoom_in"] kbd')).toHaveText("⌘=");
  await expect(dialog.locator('[data-action="zoom_out"] kbd')).toHaveText("⌘-");
  await expect(dialog.locator('[data-action="zoom_reset"] kbd')).toHaveText("⌘0");
});
