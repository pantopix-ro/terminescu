import { expect, test, type Page } from "@playwright/test";

// Owner requests of 2026-11-05 on the mock host: the sidebar header row is gone, the wordmark
// sits beside the traffic lights and moves the window, and sort moved to the menu of the empty
// space and to the palette.

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
}

test("the wordmark replaces the header row and is a drag region", async ({ page }) => {
  await start(page);
  const mark = page.getByLabel("Terminescu", { exact: true });
  await expect(mark).toHaveText("TERMINESCU");
  await expect(page.locator(".app-traffic")).toHaveAttribute("data-tauri-drag-region", "true");
  await expect(mark).toHaveAttribute("data-tauri-drag-region", "true");
  await expect(page.locator(".side-head")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "New side tab" })).toHaveCount(0);
  // The strip matches the tab bar, so the two bottom lines meet.
  const strip = await page.locator(".app-traffic").boundingBox();
  const tabs = await page.locator(".ao-tabs").boundingBox();
  expect(strip!.height).toBe(tabs!.height);
  // ESCU is amber; the mark sits centred between the last traffic light (68 pt) and the border.
  await expect(mark.locator("b")).toHaveText("ESCU");
  const [m, side] = [(await mark.boundingBox())!, (await page.locator(".app-side").boundingBox())!];
  const border = side.x + side.width - 2;
  expect(Math.abs(m.x - 68 - (border - m.x - m.width))).toBeLessThanOrEqual(2);
});

test("a right click under the rows adds a side tab or changes the sort", async ({ page }) => {
  await start(page);
  await page.locator(".side-fill").click({ button: "right" });
  await expect(page.getByRole("menu", { name: "Side tabs" })).toBeVisible();
  await page.getByRole("menuitem", { name: /^New side tab/ }).click();
  await expect.poll(async () => (await workspace(page)).windows[0].sideTabs.length).toBe(2);

  await page.locator(".side-fill").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Sort side tabs" }).click();
  await expect(page.getByRole("menuitem", { name: "manual ✓" })).toBeVisible();
  await page.getByRole("menuitem", { name: "name", exact: true }).click();
  await page.locator(".side-fill").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Sort side tabs" }).click();
  await expect(page.getByRole("menuitem", { name: "name ✓" })).toBeVisible();
});

test("the palette changes the sort", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+p");
  await page.getByRole("group", { name: "Sort side tabs" }).getByRole("option", { name: "background" }).click();
  await page.locator(".side-fill").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Sort side tabs" }).click();
  await expect(page.getByRole("menuitem", { name: "background ✓" })).toBeVisible();
});
