import { expect, test, type Locator, type Page } from "@playwright/test";

// Task 8.51 on the mock host: the selected tab opens into its pane, and the selected sidebar row
// reaches the panes with no border between them.

const box = async (l: Locator) => (await l.boundingBox())!;
const sideRight = async (page: Page) => {
  const b = await box(page.locator(".app-side"));
  return b.x + b.width;
};
// Whether the pointer 5px inside the sidebar's right edge hits the row; the last 4px are the
// width handle, and rows that keep a gap end 8px short.
const atEdge = (page: Page, l: Locator) =>
  l.evaluate((el) => {
    const r = el.getBoundingClientRect();
    const side = document.querySelector(".app-side")!.getBoundingClientRect();
    return el.contains(document.elementFromPoint(side.right - 5, r.top + r.height / 2));
  });

test("the active tab covers the tab bar's bottom line", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  await page.getByRole("button", { name: "New tab", exact: true }).click();
  const bar = await box(page.locator(".app-main .ao-tabs"));
  const active = await box(page.locator(".app-main .ao-tab.active"));
  const other = await box(page.locator(".app-main .ao-tab:not(.active)").first());
  expect(Math.round(active.y + active.height)).toBe(Math.round(bar.y + bar.height));
  expect(Math.round(other.y + other.height)).toBe(Math.round(bar.y + bar.height - 2));
});

test("a selected one-tab row reaches the panes, the others keep a gap", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  await page.getByRole("button", { name: "Add a side tab" }).click();
  const rows = page.locator(".ao-st");
  await expect(rows).toHaveCount(2);
  const edge = await sideRight(page);
  const active = page.locator(".ao-st.active");
  const other = page.locator(".ao-st:not(.active)");
  const a = await box(active);
  const o = await box(other);
  expect(Math.round(a.x + a.width)).toBe(Math.round(edge));
  expect(o.x + o.width).toBeLessThan(edge - 4);
  expect(await atEdge(page, active)).toBe(true);
  expect(await atEdge(page, other)).toBe(false);
});

test("in a folder only the selected tab reaches the panes, and each tab wears its colour", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  for (let i = 0; i < 2; i++) await page.getByRole("button", { name: "New tab", exact: true }).click();
  const kids = page.locator(".ao-st-kid");
  await expect(kids).toHaveCount(3);
  const edge = await sideRight(page);
  const header = await box(page.locator(".ao-st.active"));
  expect(header.x + header.width).toBeLessThan(edge - 4);

  const active = page.locator(".ao-st-kid.active");
  const a = await box(active);
  expect(Math.round(a.x + a.width)).toBe(Math.round(edge));
  expect(await atEdge(page, active)).toBe(true);
  const other = page.locator(".ao-st-kid:not(.active)").first();
  const o = await box(other);
  expect(o.x + o.width).toBeLessThan(edge - 4);
  expect(await atEdge(page, other)).toBe(false);

  const bgs = new Set(await kids.evaluateAll((els) => els.map((el) => getComputedStyle(el).backgroundColor)));
  expect(bgs.size).toBe(3);
});

test("a selected row's status and chord stay in line with the other rows", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  await page.getByRole("button", { name: "Add a side tab" }).click();
  for (let i = 0; i < 2; i++) await page.getByRole("button", { name: "New tab", exact: true }).click();
  const right = async (l: Locator) => {
    const b = await box(l);
    return Math.round(b.x + b.width);
  };
  const kids = page.locator(".ao-st-kid");
  await expect(kids).toHaveCount(3);
  const ends = await kids.evaluateAll((els) => els.map((el) => Math.round((el.lastElementChild as HTMLElement).getBoundingClientRect().right)));
  expect(new Set(ends).size).toBe(1);
  // The selected one-line row's chip, shown under the mouse, ends where the folder's does.
  await page.locator(".ao-st").first().click();
  await page.locator(".ao-st").first().hover();
  expect(await right(page.locator(".ao-st.single .single-chip"))).toBe(await right(page.locator(".ao-st .m > :last-child")));
});
