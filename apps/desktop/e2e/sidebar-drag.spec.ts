import { expect, test, type Page } from "@playwright/test";

// Task 8.70: a tab listed under its folder drags as a tab of the tab bar does, and the only tab
// of a side tab takes its side tab, with its variables, wherever it goes.

type Point = { x: number; y: number };

async function start(page: Page, url = "/") {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto(url);
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
  await page.locator(".term-pane").first().click();
}

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());
const kids = (page: Page) => page.locator("[data-kid]").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.kid!));
const sideIds = (page: Page) => page.locator(".ao-st").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.side!));
const tabIds = (page: Page) => page.locator(".ao-tabs [role=tab]").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.tab!));
const tabsOf = async (page: Page, side: string) =>
  ((await workspace(page)).windows.flatMap((w: any) => w.sideTabs).find((s: any) => s.id === side)?.tabs ?? []).map((t: any) => t.id) as string[];

const center = async (page: Page, sel: string): Promise<Point> => {
  const b = (await page.locator(sel).boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
};

/** Drags from an element to a point; `check` runs while the drag hovers there. */
async function drag(page: Page, from: string, to: Point | (() => Promise<Point>), check?: () => Promise<void>) {
  const box = (await page.locator(from).boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 10, box.y + box.height / 2, { steps: 3 });
  const at = typeof to === "function" ? await to() : to;
  await page.mouse.move(at.x, at.y, { steps: 8 });
  await expect(page.locator(".ao-tab.ghost")).toBeVisible();
  await check?.();
  await page.mouse.up();
}

/** A folder of `n` tabs, the first one shown. */
async function folder(page: Page, n: number) {
  await start(page);
  for (let i = 1; i < n; i++) await page.keyboard.press("Meta+t");
  await expect.poll(() => kids(page)).toHaveLength(n);
  const ids = await kids(page);
  await page.locator(`[data-kid="${ids[0]}"]`).click();
  return ids;
}

const setVar = (page: Page, side: string, name: string, value: string) =>
  page.evaluate(
    ({ side, name, value }) =>
      (window as any).__terminescu.host.addVariable(side, { name, value, mode: "saved", ttlSecs: null, source: "typed", originTabId: null }),
    { side, name, value },
  );
const varsOf = (page: Page, side: string) =>
  page.evaluate(
    async (side) => ((await (window as any).__terminescu.host.variables(side)) as { name: string; value: string }[]).map((v) => `${v.name}=${v.value}`),
    side,
  );

test("a listed tab dropped on a pane's edge leaves its folder as a row beside, with its plain variables", async ({ page }) => {
  const [a, b] = await folder(page, 2);
  const [side] = await sideIds(page);
  await setVar(page, side!, "repo", "acme/api");
  await page.evaluate(
    (side) =>
      (window as any).__terminescu.host.addVariable(side, { name: "token", value: "hidden", mode: "secret", ttlSecs: null, source: "typed", originTabId: null }),
    side,
  );
  const pane = (await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane"))!;
  const box = (await page.locator(`.app-pane .ao-tp[data-pane="${pane}"]`).boundingBox())!;
  await drag(page, `[data-kid="${b}"]`, { x: box.x + box.width * 0.95, y: box.y + box.height / 2 }, async () => {
    await expect(page.locator(".dock-preview")).toHaveAttribute("data-side", "right");
  });
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  await expect.poll(() => sideIds(page)).toHaveLength(2);
  const [, beside] = await sideIds(page);
  expect(await tabsOf(page, side!)).toEqual([a]);
  expect(await tabsOf(page, beside!)).toEqual([b]);
  // A secret never reaches the window, so it stays with the folder.
  await expect.poll(() => varsOf(page, beside!)).toEqual(["repo=acme/api"]);
  await expect(page.locator(".side-item[data-open]")).toHaveCount(2);
});

test("a listed tab reorders inside its folder, a line showing where it lands", async ({ page }) => {
  const [a, b, c] = await folder(page, 3);
  await drag(page, `[data-kid="${c}"]`, await center(page, `[data-kid="${a}"]`), async () => {
    await expect(page.locator(`[data-kid="${a}"]`)).toHaveClass(/drop-before/);
  });
  await expect.poll(() => kids(page)).toEqual([c, a, b]);
  // From above, it lands below the tab it is dropped on.
  await drag(page, `[data-kid="${c}"]`, await center(page, `[data-kid="${b}"]`), async () => {
    await expect(page.locator(`[data-kid="${b}"]`)).toHaveClass(/drop-after/);
  });
  await expect.poll(() => kids(page)).toEqual([a, b, c]);
});

test("a listed tab moves into another row, to a new side tab and to a new window", async ({ page }) => {
  const [, b, c, d] = await folder(page, 4);
  const [first] = await sideIds(page);
  await page.getByRole("button", { name: "Add a side tab" }).click();
  await expect.poll(() => sideIds(page)).toHaveLength(2);
  const [, second] = await sideIds(page);
  await page.locator(`[data-side="${first}"]`).click();
  await expect.poll(() => kids(page)).toHaveLength(4);

  await drag(page, `[data-kid="${b}"]`, await center(page, `[data-side="${second}"]`));
  await expect.poll(() => tabsOf(page, second!)).toHaveLength(2);
  expect((await tabsOf(page, second!))[1]).toBe(b);

  await page.locator(`[data-side="${first}"]`).click();
  await drag(page, `[data-kid="${c}"]`, async () => {
    await expect(page.locator(".side-end")).toBeVisible();
    return center(page, ".side-end");
  });
  await expect.poll(() => sideIds(page)).toHaveLength(3);
  expect(await tabsOf(page, (await sideIds(page))[2]!)).toEqual([c]);

  await page.locator(`[data-side="${first}"]`).click();
  await page.evaluate(() => (window as any).__terminescu.host.setDragOutcome("newWindow"));
  await drag(page, `[data-kid="${d}"]`, await center(page, ".app-pane"));
  await expect.poll(async () => (await workspace(page)).windows.length).toBe(2);
  expect((await workspace(page)).windows[1].sideTabs[0].tabs.map((t: any) => t.id)).toEqual([d]);
  const begun = await page.evaluate(() => (window as any).__terminescu.host.dragsBegun as string[]);
  expect(begun.at(-1)).toMatch(new RegExp(`^${d} `));
});

test("a side tab's only tab merges its variables into the row it joins, and Undo splits them again", async ({ page }) => {
  await start(page, "/?top-tabs");
  await page.getByRole("button", { name: "Add a side tab" }).click();
  await expect.poll(() => sideIds(page)).toHaveLength(2);
  const [first, second] = await sideIds(page);
  await setVar(page, second!, "repo", "acme/api");
  await page.locator(`[data-side="${second}"]`).click();
  const [only] = await tabIds(page);

  await drag(page, `.ao-tabs [data-tab="${only}"]`, await center(page, `[data-side="${first}"]`));
  await expect(page.locator(".toast")).toContainText("Merged");
  await expect.poll(() => sideIds(page)).toEqual([first]);
  await expect.poll(() => varsOf(page, first!)).toEqual(["repo=acme/api"]);

  await page.locator(".toast").getByRole("button", { name: "Undo" }).click();
  await expect.poll(() => sideIds(page)).toEqual([first, second]);
  await expect.poll(() => varsOf(page, second!)).toEqual(["repo=acme/api"]);
});

test("a side tab's only tab dropped on a gap moves its side tab, variables and all", async ({ page }) => {
  await start(page, "/?top-tabs");
  await page.getByRole("button", { name: "Add a side tab" }).click();
  await expect.poll(() => sideIds(page)).toHaveLength(2);
  const [first, second] = await sideIds(page);
  await setVar(page, second!, "repo", "acme/api");
  await page.locator(`[data-side="${second}"]`).click();
  const [only] = await tabIds(page);

  await drag(page, `.ao-tabs [data-tab="${only}"]`, await center(page, '.side-gap[data-gap="0"]'));
  await expect.poll(() => sideIds(page)).toEqual([second, first]);
  expect(await varsOf(page, second!)).toEqual(["repo=acme/api"]);
  await expect(page.locator(".toast")).toHaveCount(0);
});

test("a tab from another window, its side tab's only one, shows beside as its row, variables kept", async ({ page }) => {
  await start(page, "/?top-tabs");
  await page.keyboard.press("Meta+t");
  await expect.poll(() => tabIds(page)).toHaveLength(2);
  const [a, b] = await tabIds(page);
  await page.evaluate((b) => (window as any).__terminescu.host.send({ type: "MoveTabToNewWindow", tabId: b }), b);
  await expect.poll(() => tabIds(page)).toEqual([a]);
  const [first] = await sideIds(page);
  const away = (await workspace(page)).windows[1].sideTabs[0].id as string;
  await setVar(page, away, "repo", "acme/api");

  const box = (await page.locator(".app-pane .ao-tp").first().boundingBox())!;
  const edge = { x: box.x + box.width * 0.95, y: box.y + box.height / 2 };
  await page.evaluate(({ b, edge }) => (window as any).__terminescu.host.dragDrop({ tabId: b, ...edge }), { b, edge });
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  await expect.poll(async () => (await workspace(page)).windows.length).toBe(1);
  await expect.poll(() => sideIds(page)).toEqual([first, away]);
  expect(await tabsOf(page, away)).toEqual([b]);
  expect(await varsOf(page, away)).toEqual(["repo=acme/api"]);
  expect(await varsOf(page, first!)).toEqual([]);
  // Nothing merged: no toast, and the arrival is selected.
  await expect(page.locator(".toast")).toHaveCount(0);
  await expect(page.locator(`[data-side="${away}"]`)).toHaveAttribute("aria-current", "true");
});
