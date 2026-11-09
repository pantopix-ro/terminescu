import { expect, test, type Page } from "@playwright/test";

// Task 8.71: a window shows two or more sidebar rows side by side. Each shown row is marked
// open, the selected one carries the orange outline, and a window's only row joins another
// window as a row of its own.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
  await page.locator(".term-pane").first().click();
}

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());
const sideIds = (page: Page) => page.locator(".ao-st").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.side!));
const openIds = (page: Page) =>
  page.locator(".side-item[data-open] .ao-st").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.side!).sort());
const selected = (page: Page) => page.locator('.ao-st[aria-current="true"]').getAttribute("data-side");
const paneOf = async (page: Page, side: string) =>
  ((await workspace(page)).windows[0].sideTabs.find((s: any) => s.id === side).tabs[0].layout.id as string);

async function addRow(page: Page) {
  const before = (await sideIds(page)).length;
  await page.getByRole("button", { name: "Add a side tab" }).click();
  await expect.poll(async () => (await sideIds(page)).length).toBe(before + 1);
  return (await sideIds(page)).at(-1)!;
}

/** Drags a sidebar row to the right edge of the shown panes. */
async function rowToRight(page: Page, side: string) {
  const from = (await page.locator(`[data-side="${side}"]`).boundingBox())!;
  const to = (await page.locator(".app-pane").boundingBox())!;
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2 + 10, from.y + from.height / 2, { steps: 3 });
  await page.mouse.move(to.x + to.width * 0.97, to.y + to.height / 2, { steps: 8 });
  await expect(page.locator(".dock-preview")).toHaveAttribute("data-side", "right");
  await page.mouse.up();
}

test("rows shown side by side are open, and the selected one follows the pane clicked", async ({ page }) => {
  await start(page);
  const [first] = await sideIds(page);
  const second = await addRow(page);
  await expect.poll(() => selected(page)).toBe(second);
  await rowToRight(page, first!);

  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  await expect.poll(() => openIds(page)).toEqual([first!, second].sort());
  await expect(page.locator(".ao-side.split")).toHaveCount(1);
  await expect.poll(() => selected(page)).toBe(first);
  // Each area is headed by its row, the selected one underlined (task 8.73).
  await expect(page.locator(".view-head")).toHaveCount(2);
  await expect(page.locator(".view-area.on")).toHaveAttribute("data-area", first!);
  // The orange outline is the focus colour.
  const ring = await page.locator(`[data-side="${first}"]`).evaluate((e) => getComputedStyle(e).boxShadow);
  expect(ring).toContain("rgb(229, 163, 58)");

  // A pane of the other row selects that row; both stay open, each a simple row.
  await page.locator(`.app-pane .ao-tp[data-pane="${await paneOf(page, second)}"]`).click();
  await expect.poll(() => selected(page)).toBe(second);
  await expect.poll(() => openIds(page)).toEqual([first!, second].sort());
  await expect(page.locator(".ao-st-kids")).toHaveCount(0);

  // A row picked in the sidebar takes the selected area's place.
  const third = await addRow(page);
  await expect.poll(() => selected(page)).toBe(third);
  await expect.poll(() => openIds(page)).toEqual([first!, third].sort());
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);

  // The palette shows one row again.
  await page.keyboard.press("Meta+p");
  await page.keyboard.type("show only this side tab");
  await page.keyboard.press("Enter");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
  await expect(page.locator(".side-item[data-open]")).toHaveCount(0);
  await expect(page.locator(".view-head")).toHaveCount(0);
});

test("a group's area head names the group and its tab, and its pixel tip lists the group", async ({ page }) => {
  await start(page);
  const [first] = await sideIds(page);
  await page.keyboard.press("Meta+t");
  await expect(page.locator(".ao-st-kids [data-kid]")).toHaveCount(2);
  const second = await addRow(page);
  await rowToRight(page, first!);
  const head = page.locator(`.view-area[data-area="${first}"] > .view-head`);
  await expect(head.locator(".view-group")).toHaveCount(1);
  await expect(head.locator(".view-tab")).toHaveCount(1);
  await expect(page.locator(`.view-area[data-area="${second}"] > .view-head .view-group`)).toHaveCount(0);

  await head.hover();
  const tip = page.locator(".px-tip");
  await expect(tip).toBeVisible();
  await expect(tip.locator(".px-tip-name")).toContainText("›");
  await expect(tip.locator(".px-tip-sub").first()).toHaveText("group of 2");
  // One line per tab, the shown one marked (task 8.74).
  await expect(tip.locator(".px-tip-line")).toHaveCount(2);
  await expect(tip.locator(".px-tip-line", { hasText: "▸" })).toHaveCount(1);
  // A click on the other head selects that row, and the tip goes.
  await page.locator(`.view-area[data-area="${second}"] > .view-head`).click();
  await expect.poll(() => selected(page)).toBe(second);
});

test("closing the selected shown row selects the other one", async ({ page }) => {
  await start(page);
  const [first] = await sideIds(page);
  const second = await addRow(page);
  await rowToRight(page, first!);
  await expect.poll(() => selected(page)).toBe(first);
  await page.evaluate((id) => (window as any).__terminescu.host.send({ type: "CloseSideTab", sideTabId: id }), first);
  await expect.poll(() => selected(page)).toBe(second);
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
});

test("a window's only row dropped on another window joins it as a simple row", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+t");
  await expect(page.locator(".ao-tabs [role=tab]")).toHaveCount(2);
  const b = await page.locator(".ao-tabs [role=tab]").nth(1).getAttribute("data-tab");
  await page.evaluate((b) => (window as any).__terminescu.host.send({ type: "MoveTabToNewWindow", tabId: b }), b);
  await expect(page.locator(".ao-tabs [role=tab]")).toHaveCount(1);
  const away = (await workspace(page)).windows[1].sideTabs[0].id as string;

  const box = (await page.locator(".app-pane .ao-tp").first().boundingBox())!;
  const middle = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  await page.evaluate(({ b, middle }) => (window as any).__terminescu.host.dragDrop({ tabId: b, ...middle }), { b, middle });
  await expect.poll(async () => (await workspace(page)).windows.length).toBe(1);
  await expect.poll(() => sideIds(page)).toHaveLength(2);
  expect((await sideIds(page))[1]).toBe(away);
  await expect.poll(() => selected(page)).toBe(away);
  await expect(page.locator(".ao-st-kids")).toHaveCount(0);
  await expect(page.locator(".toast")).toHaveCount(0);
});

test("the tab bar starts with the selected group's name, and its pixel tip lists the tabs", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  await expect(page.locator(".ao-tabs-group")).toHaveCount(0);
  await page.keyboard.press("Meta+t");
  await expect(page.locator(".ao-tabs [role=tab]")).toHaveCount(2);
  const [side] = await sideIds(page);
  const group = page.locator(".ao-tabs-group");
  await expect(group).toHaveCount(1);
  // The bar names the group as the sidebar does.
  const name = (await group.innerText()).trim();
  expect(name.length).toBeGreaterThan(0);
  await expect(page.locator(`[data-side="${side}"]`)).toContainText(name);
  await group.hover();
  const tip = page.locator(".ao-tabs .px-tip");
  await expect(tip.locator(".px-tip-line")).toHaveCount(2);
  await expect(tip.locator(".px-tip-line", { hasText: "▸" })).toHaveCount(1);
  // Back to one tab, the bar has no group to name.
  await page.keyboard.press("Meta+w");
  await expect(page.locator(".ao-tabs [role=tab]")).toHaveCount(1);
  await expect(group).toHaveCount(0);
});
