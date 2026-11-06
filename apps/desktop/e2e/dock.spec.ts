import { expect, test, type Page } from "@playwright/test";

// Task 8.32: a tab dropped on a side of a pane splits that pane, the tab's panes taking the
// half; a pane header dragged to the tab bar makes the pane a tab again.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
  await page.locator(".term-pane").first().click();
}

type Layout = { type: "pane"; id: string } | { type: "split"; dir: "row" | "column"; children: Layout[] };
type Point = { x: number; y: number };

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());
const tabs = async (page: Page) => (await workspace(page)).windows[0].sideTabs[0].tabs as { id: string; layout: Layout }[];
const tabIds = (page: Page) => page.locator(".ao-tabs [role=tab]").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.tab!));
const shownPanes = (page: Page) =>
  page.locator(".app-pane .ao-tp").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.pane!));

/** Two tabs, the first one shown: returns [shown pane, the second tab's id, its pane]. */
async function twoTabs(page: Page) {
  await start(page);
  const [pane] = await shownPanes(page);
  await page.keyboard.press("Meta+t");
  await expect.poll(() => tabIds(page)).toHaveLength(2);
  const [first, second] = await tabIds(page);
  await page.locator(`.ao-tabs [data-tab="${first}"]`).click();
  await expect.poll(() => shownPanes(page)).toEqual([pane]);
  const other = (await tabs(page)).find((t) => t.id === second)!;
  return { pane: pane!, first: first!, second: second!, otherPane: (other.layout as { id: string }).id };
}

/** A point at fractions of a pane's box. */
async function at(page: Page, pane: string, fx: number, fy: number): Promise<Point> {
  const b = (await page.locator(`.app-pane .ao-tp[data-pane="${pane}"]`).boundingBox())!;
  return { x: b.x + b.width * fx, y: b.y + b.height * fy };
}

/** Drags from an element to a point and waits for the preview's side before `release`. */
async function drag(page: Page, from: string, to: Point, side: string | null, release: "drop" | "escape" = "drop", small = false, target?: string) {
  const box = (await page.locator(from).boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 10, box.y + box.height / 2, { steps: 3 });
  await page.mouse.move(to.x, to.y, { steps: 8 });
  await expect(page.locator(".ao-tab.ghost")).toBeVisible();
  if (side === null) await expect(page.locator(".dock-preview")).toHaveCount(0);
  else await expect(page.locator(".dock-preview")).toHaveAttribute("data-side", side);
  if (small) await expect(page.locator(".dock-preview")).toHaveClass(/\bsmall\b/);
  if (side && side !== "center") await expect(page.locator(".app-pane .ao-tp.dock-target")).toHaveCount(1);
  if (small) await expect(page.locator(".app-pane .ao-tp.dock-target")).toHaveClass(/too-small/);
  if (target) await expect(page.locator(".app-pane .ao-tp.dock-target")).toHaveAttribute("data-pane", target);
  if (side === "center") await expect(page.locator(".app-pane .ao-tp.dock-target")).toHaveCount(0);
  if (release === "escape") await page.keyboard.press("Escape");
  await page.mouse.up();
}

for (const [side, fx, fy, dir, order] of [
  ["right", 0.95, 0.5, "row", "after"],
  ["left", 0.05, 0.5, "row", "before"],
  ["down", 0.5, 0.95, "column", "after"],
  ["up", 0.5, 0.05, "column", "before"],
] as const) {
  test(`a tab dropped on the ${side} edge splits the pane ${dir === "row" ? "side by side" : "top and bottom"}`, async ({ page }) => {
    const { pane, second, otherPane } = await twoTabs(page);
    await drag(page, `.ao-tabs [data-tab="${second}"]`, await at(page, pane, fx, fy), side);
    // The preview covers the half the tab takes.
    await expect(page.locator(".dock-preview")).toHaveCount(0);
    await expect.poll(() => tabIds(page)).toHaveLength(1);
    const [tab] = await tabs(page);
    expect(tab!.layout).toMatchObject({ type: "split", dir, children: order === "after" ? [{ id: pane }, { id: otherPane }] : [{ id: otherPane }, { id: pane }] });
    await expect.poll(() => shownPanes(page)).toHaveLength(2);
  });
}

test("the preview covers the half the tab takes", async ({ page }) => {
  const { pane, second } = await twoTabs(page);
  const box = (await page.locator(`.app-pane .ao-tp[data-pane="${pane}"]`).boundingBox())!;
  const p = await at(page, pane, 0.95, 0.5);
  const tab = (await page.locator(`.ao-tabs [data-tab="${second}"]`).boundingBox())!;
  await page.mouse.move(tab.x + tab.width / 2, tab.y + tab.height / 2);
  await page.mouse.down();
  await page.mouse.move(tab.x + tab.width / 2 + 10, tab.y + tab.height / 2, { steps: 3 });
  await page.mouse.move(p.x, p.y, { steps: 8 });
  await expect(page.locator(".dock-preview")).toHaveAttribute("data-side", "right");
  await page.waitForTimeout(150);
  const shown = (await page.locator(".dock-preview").boundingBox())!;
  expect(Math.abs(shown.x - (box.x + box.width / 2))).toBeLessThan(2);
  expect(Math.abs(shown.width - box.width / 2)).toBeLessThan(2);
  await page.keyboard.press("Escape");
  await page.mouse.up();
});

test("the middle of a pane, Esc and the shown tab itself change nothing", async ({ page }) => {
  const { pane, first, second } = await twoTabs(page);
  await drag(page, `.ao-tabs [data-tab="${second}"]`, await at(page, pane, 0.5, 0.5), "center");
  await drag(page, `.ao-tabs [data-tab="${second}"]`, await at(page, pane, 0.95, 0.5), "right", "escape");
  await drag(page, `.ao-tabs [data-tab="${first}"]`, await at(page, pane, 0.95, 0.5), null);
  await expect.poll(() => tabIds(page)).toEqual([first, second]);
  await expect.poll(() => shownPanes(page)).toEqual([pane]);
});

test("a pane too small to halve shows red and refuses the drop", async ({ page }) => {
  const { pane, second } = await twoTabs(page);
  // Split the shown pane until the core refuses: the last pane cannot halve again.
  for (let i = 0; i < 8; i++) await page.keyboard.press("Meta+d");
  await expect.poll(async () => (await shownPanes(page)).length).toBeGreaterThan(3);
  await page.waitForTimeout(200);
  const thin = (await shownPanes(page)).at(-1)!;
  await drag(page, `.ao-tabs [data-tab="${second}"]`, await at(page, thin, 0.9, 0.5), "right", "drop", true);
  await expect(page.locator(".dock-preview.small")).toHaveCount(0);
  await expect.poll(() => tabIds(page)).toHaveLength(2);
  expect(pane).toBeTruthy();
});

test("a pane header dragged to the tab bar becomes a tab; the palette docks and undocks", async ({ page }) => {
  const { pane, second, otherPane } = await twoTabs(page);
  await page.keyboard.press("Meta+k");
  await page.keyboard.type("split right with tab");
  await page.keyboard.press("Enter");
  await expect.poll(() => tabIds(page)).toHaveLength(1);
  await expect.poll(() => shownPanes(page)).toEqual([pane, otherPane]);
  expect(second).toBeTruthy();

  const end = (await page.locator(".ao-tabs [data-tab]").last().boundingBox())!;
  await drag(page, `.ao-tp[data-pane="${otherPane}"] .ao-tp-head`, { x: end.x + end.width + 20, y: end.y + end.height / 2 }, null);
  await expect.poll(() => tabIds(page)).toHaveLength(2);
  await expect.poll(() => shownPanes(page)).toEqual([otherPane]);
  // ⌘⇧B moves the focused pane to a new tab; it needs a split, so dock again first.
  await page.keyboard.press("Meta+k");
  await page.keyboard.type("split down with tab");
  await page.keyboard.press("Enter");
  await expect.poll(() => shownPanes(page)).toHaveLength(2);
  await page.keyboard.press("Meta+Shift+b");
  await expect.poll(() => tabIds(page)).toHaveLength(2);
  await expect.poll(() => shownPanes(page)).toHaveLength(1);
});

test("the pane that splits is outlined; ⌘⇧B on a lone pane does nothing", async ({ page }) => {
  const { pane, second } = await twoTabs(page);
  await page.keyboard.press("Meta+Shift+b");
  await expect.poll(() => tabIds(page)).toHaveLength(2);
  await page.keyboard.press("Meta+d");
  await expect.poll(() => shownPanes(page)).toHaveLength(2);
  const [, right] = await shownPanes(page);
  // Over the left pane's bottom edge: only that pane is outlined, not the one beside it.
  await drag(page, `.ao-tabs [data-tab="${second}"]`, await at(page, pane, 0.5, 0.95), "down", "escape", false, pane);
  await expect(page.locator(".app-pane .ao-tp.dock-target")).toHaveCount(0);
  expect(right).toBeTruthy();
});
