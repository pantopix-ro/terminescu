import { expect, test, type Page } from "@playwright/test";
import { addTab } from "./add-tab";

// Task 8.72: a row dragged into the gap between rows moves there (hand-sorted order only); the
// middle of a row still merges.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
  await page.locator(".term-pane").first().click();
}

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());
const sideIds = (page: Page) => page.locator(".ao-st").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.side!));
const tabsOf = async (page: Page, side: string) =>
  ((await workspace(page)).windows.flatMap((w: any) => w.sideTabs).find((s: any) => s.id === side)?.tabs ?? []).map((t: any) => t.id) as string[];

/** Three one-tab rows. */
async function rows(page: Page) {
  await start(page);
  await page.keyboard.press("Meta+Shift+KeyN");
  await page.keyboard.press("Meta+Shift+KeyN");
  await expect.poll(() => sideIds(page)).toHaveLength(3);
  return sideIds(page);
}

/** Drags a row's centre to a point; `check` runs while it hovers there. */
async function dragRow(page: Page, side: string, to: { x: number; y: number }, check?: () => Promise<void>) {
  const b = (await page.locator(`[data-side="${side}"]`).boundingBox())!;
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2 + 10, b.y + b.height / 2, { steps: 3 });
  await page.mouse.move(to.x, to.y, { steps: 8 });
  await expect(page.locator(".ao-tab.ghost")).toBeVisible();
  await check?.();
  await page.mouse.up();
}

const gapAt = async (page: Page, i: number) => {
  const b = (await page.locator(`.side-gap[data-gap="${i}"]`).boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
};

test("a row dropped in a gap moves there, with a line while it hovers", async ({ page }) => {
  const [a, b, c] = await rows(page);
  await dragRow(page, c!, await gapAt(page, 0), async () => {
    await expect(page.locator(".side-gap .ao-drop-line")).toHaveCount(1);
    await expect(page.locator(".ao-st.drop-into")).toHaveCount(0);
  });
  await expect.poll(() => sideIds(page)).toEqual([c, a, b]);
  // To the end: the gap under the last row.
  await dragRow(page, c!, await gapAt(page, 3));
  await expect.poll(() => sideIds(page)).toEqual([a, b, c]);
  // The gaps on either side of the row show no line: it would stay put.
  await dragRow(page, b!, await gapAt(page, 1), async () => {
    await expect(page.locator(".side-gap .ao-drop-line")).toHaveCount(0);
  });
  await expect.poll(() => sideIds(page)).toEqual([a, b, c]);
});

test("the middle of a row still merges", async ({ page }) => {
  const [a, b] = await rows(page);
  const box = (await page.locator(`[data-side="${b}"]`).boundingBox())!;
  await dragRow(page, a!, { x: box.x + box.width / 2, y: box.y + box.height / 2 }, async () => {
    await expect(page.locator(".ao-st.drop-into")).toHaveCount(1);
    await expect(page.locator(".side-gap .ao-drop-line")).toHaveCount(0);
  });
  await expect.poll(async () => (await tabsOf(page, b!)).length).toBe(2);
});

test("a folder reorders as a row and keeps its tabs", async ({ page }) => {
  const [a, b, c] = await rows(page);
  await page.locator(`[data-side="${a}"]`).click();
  await addTab(page);
  const tabs = await tabsOf(page, a!);
  expect(tabs).toHaveLength(2);
  await dragRow(page, a!, await gapAt(page, 3));
  await expect.poll(() => sideIds(page)).toEqual([b, c, a]);
  expect(await tabsOf(page, a!)).toEqual(tabs);
});

test("sorted by name, a row shows no gap line and does not move", async ({ page }) => {
  const [a, b, c] = await rows(page);
  const order = await sideIds(page);
  await page.locator(".side-fill").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Sort side tabs" }).click();
  await page.getByRole("menuitem", { name: /^name/ }).click();
  const shown = await sideIds(page);
  const last = shown[shown.length - 1]!;
  await dragRow(page, last, await gapAt(page, 0), async () => {
    await expect(page.locator(".side-gap .ao-drop-line")).toHaveCount(0);
  });
  expect(await sideIds(page)).toEqual(shown);
  expect((await workspace(page)).windows[0].sideTabs.map((s: any) => s.id)).toEqual(order);
  expect(new Set([a, b, c]).size).toBe(3);
});
