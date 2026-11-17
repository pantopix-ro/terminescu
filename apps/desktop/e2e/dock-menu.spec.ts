import { expect, test, type Page } from "@playwright/test";

// add-dock-menu: the window reports its rows for the Dock menu and selects a picked row.

const rows = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.dockRows());

test("the window reports its rows as the sidebar names them", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".term-pane")).toHaveCount(1);
  await page.keyboard.press("Meta+Shift+N");
  await expect(page.locator(".term-pane:visible")).toHaveCount(1);
  await expect.poll(async () => (await rows(page)).length).toBe(2);
  const list = await rows(page);
  expect(list.filter((r: any) => r.active)).toHaveLength(1);
  expect(list[1].active).toBe(true);
  const names = await page.locator(".ao-st .t").allTextContents();
  expect(list.map((r: any) => r.name)).toEqual(names.slice(0, 2));
});

test("a Dock menu pick selects the row", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".term-pane")).toHaveCount(1);
  await page.keyboard.press("Meta+Shift+N");
  await expect.poll(async () => (await rows(page)).length).toBe(2);
  const first = (await rows(page))[0].id;
  await page.evaluate((id) => (window as any).__terminescu.host.dockPick(id), first);
  await expect.poll(async () => (await rows(page)).find((r: any) => r.active)?.id).toBe(first);
  // A row that is gone does nothing.
  await page.evaluate(() => (window as any).__terminescu.host.dockPick("gone"));
  await expect.poll(async () => (await rows(page)).find((r: any) => r.active)?.id).toBe(first);
});
