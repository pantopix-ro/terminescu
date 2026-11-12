import { expect, test, type Page } from "@playwright/test";

// Task 8.15 on the mock host: a double click on a side tab's or a tab's name edits it in place,
// in the sidebar and in the tab bar.

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
}

test("a double click on a one-tab row's name renames its tab in a block over the row", async ({ page }) => {
  await start(page);
  const row = page.locator(".ao-st").first();
  await row.locator(".t").dblclick();
  // Task 8.89: the row is its tab, and the name is edited in a wrapping block as wide as the row.
  const input = page.getByLabel("Tab name");
  await expect(input).toBeFocused();
  expect(await input.evaluate((e) => e.tagName)).toBe("TEXTAREA");
  const [box, rowBox] = [(await input.boundingBox())!, (await row.boundingBox())!];
  expect(box.width).toBeGreaterThan(rowBox.width - 32);
  expect(box.height).toBeGreaterThanOrEqual(40);
  await input.fill("acme/api");
  await input.press("Enter");
  await expect(page.locator(".ao-st .t").first()).toHaveText("acme/api");
  const side = (await workspace(page)).windows[0].sideTabs[0];
  expect([side.title, side.tabs[0].title]).toEqual([null, "acme/api"]);
});

test("a double click on a tab's name in the tab bar renames only there", async ({ page }) => {
  await start(page);
  await page.locator(".ao-tab .n").first().dblclick();
  await expect(page.getByLabel("Tab name")).toHaveCount(1);
  await page.getByLabel("Tab name").fill("logs");
  await page.getByLabel("Tab name").press("Enter");
  await expect(page.locator(".ao-tab .n").first()).toHaveText("logs");
  expect((await workspace(page)).windows[0].sideTabs[0].tabs[0].title).toBe("logs");
});

test("a double click on a tab's name in an open folder renames it; Escape keeps the old name", async ({ page }) => {
  await start(page);
  const side = (await workspace(page)).windows[0].sideTabs[0];
  await mock(page, "send", { type: "CreateTab", sideTabId: side.id });
  const kid = page.locator(".ao-st-kid .kid-n").last();
  await kid.dblclick();
  const input = page.getByLabel("Tab name");
  await input.fill("discarded");
  await input.press("Escape");
  await expect(input).toHaveCount(0);
  await kid.dblclick();
  await page.getByLabel("Tab name").fill("k9s");
  await page.getByLabel("Tab name").press("Enter");
  await expect(page.locator(".ao-st-kid .kid-n").last()).toHaveText("k9s");
});
