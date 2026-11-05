import { expect, test, type Page } from "@playwright/test";

// Group 3 on the mock host: splits, keys, side tab groups, tab drags, the palette and the
// context panel. Chords arrive as key events here; in the app the menu sends the same ids.


async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect.poll(() => firstLine(page)).toMatch(/^mock \S+ \$ $/);
  await page.locator(".term-pane").first().click();
}

const firstLine = (page: Page) =>
  page.evaluate(() => {
    const t = [...(window as any).__terminescu.terminals.values()][0];
    return t?.buffer.active.getLine(0)?.translateToString(true) ?? "";
  });

/** Pane ids of the active tab, in reading order. */
const shownPanes = (page: Page) =>
  page.locator(".app-pane .ao-tp").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.pane!));

const focusedPane = (page: Page) =>
  page.evaluate(() => {
    const ta = document.activeElement?.closest<HTMLElement>(".term-pane");
    return ta?.dataset.pane;
  });

const lineOf = (page: Page, pane: string, y: number) =>
  page.evaluate(
    ({ pane, y }) => (window as any).__terminescu.terminals.get(pane)?.buffer.active.getLine(y)?.translateToString(true) ?? "",
    { pane, y },
  );

const mock = (page: Page) =>
  page.evaluate(() => {
    const h = (window as any).__terminescu.host;
    return { ws: h.workspace(), batches: h.batches.length as number, resizes: [...h.resizes] as string[] };
  });

const sideNames = (page: Page) => page.locator(".ao-st .t").allTextContents();
const tabIds = (page: Page) => page.locator(".ao-tabs [role=tab]").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.tab!));

test("⌘D and ⌘⇧D split, keys go to the focused pane, ⌘⌥ arrows move focus, ⌘W closes", async ({ page }) => {
  await start(page);
  const [first] = await shownPanes(page);
  await page.evaluate((id) => ((window as any).__first = (window as any).__terminescu.terminals.get(id)), first);

  await page.keyboard.press("Meta+d");
  await expect.poll(() => shownPanes(page)).toHaveLength(2);
  const [, right] = await shownPanes(page);
  await expect.poll(() => focusedPane(page)).toBe(right);
  await expect.poll(() => lineOf(page, right!, 0)).toBe(`mock ${right} $ `);
  // The first terminal survives the split: same xterm.js instance, its screen intact.
  expect(await page.evaluate((id) => (window as any).__terminescu.terminals.get(id) === (window as any).__first, first)).toBe(true);

  await page.keyboard.type("hello");
  await page.keyboard.press("Enter");
  await expect.poll(() => lineOf(page, right!, 1)).toBe("you typed: hello");
  expect(await lineOf(page, first!, 1)).toBe("");

  await page.keyboard.press("Meta+Shift+d");
  await expect.poll(() => shownPanes(page)).toHaveLength(3);
  const below = (await shownPanes(page))[2]!;
  await expect.poll(() => focusedPane(page)).toBe(below);
  const boxes = await Promise.all([right, below].map((id) => page.locator(`.ao-tp[data-pane="${id}"]`).boundingBox()));
  expect(boxes[1]!.y).toBeGreaterThan(boxes[0]!.y);

  await page.keyboard.press("Meta+Alt+ArrowLeft");
  await expect.poll(() => focusedPane(page)).toBe(first);
  await page.keyboard.press("Meta+Alt+ArrowRight");
  await expect.poll(() => focusedPane(page)).not.toBe(first);

  await page.locator(`.ao-tp[data-pane="${below}"] .term-pane`).click();
  await page.keyboard.press("Meta+w");
  await expect.poll(() => shownPanes(page)).toEqual([first, right]);
  await expect.poll(() => focusedPane(page)).toBe(right);
  await page.keyboard.press("Meta+w");
  await expect.poll(() => shownPanes(page)).toEqual([first]);
  await expect(page.locator(".ao-tp-head")).toHaveCount(0);
});

test("dragging a split border resizes on release and stops at the minimum", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+d");
  await expect.poll(() => shownPanes(page)).toHaveLength(2);
  const bar = page.locator(".split-bar").first();
  const box = (await bar.boundingBox())!;
  const before = (await mock(page)).batches;
  await page.mouse.move(box.x + 1, box.y + 200);
  await page.mouse.down();
  await page.mouse.move(box.x - 150, box.y + 200, { steps: 5 });
  expect((await mock(page)).batches, "no command while dragging").toBe(before);
  await page.mouse.up();
  await expect.poll(async () => (await mock(page)).ws.windows[0].sideTabs[0].tabs[0].layout.sizes?.[0]).toBeLessThan(500);

  const again = (await bar.boundingBox())!;
  await page.mouse.move(again.x + 1, again.y + 200);
  await page.mouse.down();
  await page.mouse.move(0, again.y + 200, { steps: 5 });
  await page.mouse.up();
  await expect.poll(async () => (await mock(page)).ws.windows[0].sideTabs[0].tabs[0].layout.sizes).toEqual([50, 950]);
});

test("⌘T, ⌘N, ⌘1-⌘9 and ⌃⌘1-⌃⌘9 select; a side tab with tabs lists them", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+t");
  await page.keyboard.press("Meta+t");
  await expect.poll(() => tabIds(page)).toHaveLength(3);
  const ids = await tabIds(page);
  await expect(page.locator(".ao-tabs .ao-tab.active")).toHaveAttribute("data-tab", ids[2]!);
  await expect(page.locator(".ao-st-kid")).toHaveCount(3);
  await expect(page.locator(".ao-st-kid").last()).toContainText("⌘9");

  await page.keyboard.press("Meta+1");
  await expect(page.locator(".ao-tabs .ao-tab.active")).toHaveAttribute("data-tab", ids[0]!);
  await page.keyboard.press("Meta+9");
  await expect(page.locator(".ao-tabs .ao-tab.active")).toHaveAttribute("data-tab", ids[2]!);
  await page.keyboard.press("Meta+2");
  await expect(page.locator(".ao-tabs .ao-tab.active")).toHaveAttribute("data-tab", ids[1]!);

  await page.keyboard.press("Meta+n");
  await expect(page.locator(".ao-st")).toHaveCount(2);
  await expect(page.locator(".ao-st").last()).toHaveClass(/active/);
  // The other group folds to its row; the chevron opens it again.
  await expect(page.locator(".ao-st-kid")).toHaveCount(0);
  await page.getByLabel("Expand side tab").click();
  await expect(page.locator(".ao-st-kid")).toHaveCount(3);

  await page.keyboard.press("Control+Meta+1");
  await expect(page.locator(".ao-st").first()).toHaveClass(/active/);
  await expect(page.locator(".ao-tabs .ao-tab.active")).toHaveAttribute("data-tab", ids[1]!);
  await page.keyboard.press("Control+Meta+9");
  await expect(page.locator(".ao-st").last()).toHaveClass(/active/);
  // Plain keys still reach the shell.
  await page.locator(".app-pane .term-pane").click();
  await page.keyboard.type("t");
  await page.keyboard.press("Enter");
  const pane = (await shownPanes(page))[0]!;
  await expect.poll(() => lineOf(page, pane, 1)).toBe("you typed: t");
});

type Point = { x: number; y: number };

/** Drags a tab; `to` is read once the drag has started, when the drop targets show. */
async function drag(page: Page, from: string, to: Point | (() => Promise<Point>), release: "drop" | "escape" = "drop") {
  const box = (await page.locator(from).boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 10, box.y + box.height / 2, { steps: 3 });
  const at = typeof to === "function" ? await to() : to;
  await page.mouse.move(at.x, at.y, { steps: 8 });
  await expect(page.locator(".ao-tab.ghost")).toBeVisible();
  if (release === "escape") await page.keyboard.press("Escape");
  await page.mouse.up();
}

const center = async (page: Page, sel: string) => {
  const b = (await page.locator(sel).boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
};

test("tab drags: reorder, into a side tab, to a new side tab, Esc cancels", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+t");
  await page.keyboard.press("Meta+t");
  await expect.poll(() => tabIds(page)).toHaveLength(3);
  const [a, b, c] = await tabIds(page);

  await drag(page, `[data-tab="${c}"]`, await center(page, `[data-tab="${a}"]`));
  await expect.poll(() => tabIds(page)).toEqual([c, a, b]);

  const before = (await mock(page)).batches;
  await drag(page, `[data-tab="${a}"]`, await center(page, ".side-list"), "escape");
  await page.waitForTimeout(100);
  expect((await mock(page)).batches, "Esc cancels the drag").toBe(before);
  await expect(page.locator(".ao-tab.ghost")).toHaveCount(0);

  await expect(page.locator(".side-end")).toBeHidden();
  await drag(page, `[data-tab="${a}"]`, async () => {
    await expect(page.locator(".side-end")).toBeVisible();
    return center(page, ".side-end");
  });
  await expect(page.locator(".ao-st")).toHaveCount(2);
  await expect(page.locator(".ao-st").last()).toHaveClass(/active/);
  await expect.poll(() => tabIds(page)).toEqual([a]);

  await page.keyboard.press("Control+Meta+1");
  await expect.poll(() => tabIds(page)).toEqual([c, b]);
  const target = page.locator(".ao-st").last();
  await drag(page, `[data-tab="${b}"]`, await center(page, ".ao-st >> nth=-1"));
  await expect(target).toHaveClass(/active/);
  await expect.poll(() => tabIds(page)).toEqual([a, b]);
  await page.keyboard.press("Control+Meta+1");
  await expect.poll(() => tabIds(page)).toEqual([c]);
});

test("the palette runs actions and moves the tab", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+t");
  await expect.poll(() => tabIds(page)).toHaveLength(2);
  const tab = (await tabIds(page))[1]!;

  await page.keyboard.press("Meta+k");
  await expect(page.locator(".palette-input")).toBeFocused();
  await page.keyboard.type("split right");
  await page.keyboard.press("Enter");
  await expect(page.locator(".palette")).toHaveCount(0);
  await expect.poll(() => shownPanes(page)).toHaveLength(2);

  await page.keyboard.press("Meta+k");
  await page.locator(".palette-item", { hasText: "New side tab" }).filter({ hasNotText: "⌘" }).click();
  await expect(page.locator(".ao-st")).toHaveCount(2);
  await expect.poll(() => tabIds(page)).toEqual([tab]);
  await expect.poll(() => shownPanes(page)).toHaveLength(2);

  await page.keyboard.press("Meta+k");
  await page.keyboard.press("Escape");
  await expect(page.locator(".palette")).toHaveCount(0);
  await expect.poll(() => focusedPane(page)).toBeTruthy();
});

test("the context panel slides over the panes without resizing them; ⌘\\ pins it", async ({ page }) => {
  await start(page);
  await expect(page.locator(".ao-edge")).toBeVisible();
  const before = (await mock(page)).resizes.length;
  const width = await page.locator(".app-pane").evaluate((e) => e.getBoundingClientRect().width);

  await page.locator(".ao-edge").hover();
  await expect(page.locator(".ctx-over")).toBeVisible();
  await page.waitForTimeout(400);
  expect((await mock(page)).resizes.length, "no pane resize while the panel slides in").toBe(before);
  expect(await page.locator(".app-pane").evaluate((e) => e.getBoundingClientRect().width)).toBe(width);
  await expect(page.locator(".ctx-over")).toContainText("No variables yet.");

  await page.mouse.move(400, 400);
  await expect(page.locator(".ctx-over")).toHaveCount(0);

  await page.locator(".app-pane .term-pane").click();
  await page.keyboard.press("Meta+\\");
  await expect(page.locator(".ctx-pinned")).toBeVisible();
  await expect.poll(async () => page.locator(".app-pane").evaluate((e) => e.getBoundingClientRect().width)).toBeLessThan(width);
  await page.reload();
  await expect(page.locator(".ctx-pinned"), "the mode survives a reload").toBeVisible();
});
