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

const sideIds = (page: Page) => page.locator(".ao-st").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.side!));

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

test("dropping a side tab onto another merges them; without a clash, a toast offers undo", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Add a side tab" }).click();
  await expect.poll(() => sideIds(page)).toHaveLength(2);
  const [first, second] = await sideIds(page);
  await setVar(page, first!, "repo", "acme/api");
  await setVar(page, second!, "pr_api", "https://example.com/acme/api/pull/12");

  await drag(page, `[data-side="${second}"]`, await center(page, `[data-side="${first}"]`));
  await expect(page.locator(".toast")).toContainText("Merged");
  await expect.poll(() => sideIds(page)).toEqual([first]);
  await expect.poll(() => varsOf(page, first!)).toEqual(["repo=acme/api", "pr_api=https://example.com/acme/api/pull/12"]);

  await page.locator(".toast").getByRole("button", { name: "Undo" }).click();
  await expect.poll(() => sideIds(page)).toEqual([first, second]);
  await expect.poll(() => varsOf(page, second!)).toEqual(["pr_api=https://example.com/acme/api/pull/12"]);
  await expect.poll(() => varsOf(page, first!)).toEqual(["repo=acme/api"]);
});

test("a clash asks: keep both renames the source's variable", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Add a side tab" }).click();
  await expect.poll(() => sideIds(page)).toHaveLength(2);
  const [first, second] = await sideIds(page);
  await setVar(page, first!, "branch", "feature/PROJ-482");
  await setVar(page, second!, "branch", "feature/PROJ-477");

  await drag(page, `[data-side="${second}"]`, await center(page, `[data-side="${first}"]`));
  const dialog = page.getByRole("dialog", { name: "Merge side tabs" });
  await expect(dialog.getByRole("row", { name: "Clash branch" })).toBeVisible();
  await expect(dialog.getByRole("textbox")).toHaveValue("branch_2");
  // The field has the focus: type over the suggestion as a person would.
  const retype = async (text: string) => {
    await page.keyboard.press("Meta+a");
    await page.keyboard.type(text);
  };
  await expect(dialog.getByRole("textbox")).toBeFocused();
  // dnd-kit keeps its document listeners for 50 ms after a drop; keys in that time are lost.
  await page.waitForTimeout(100);
  await retype("2bad");
  await expect(dialog.getByRole("alert")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Keep both" })).toBeDisabled();
  await retype("branch_477");
  await dialog.getByRole("button", { name: "Keep both" }).click();
  await expect(page.locator(".toast")).toContainText("branch → branch_477");
  await expect.poll(() => varsOf(page, first!)).toEqual(["branch=feature/PROJ-482", "branch_477=feature/PROJ-477"]);
  await expect.poll(() => sideIds(page)).toEqual([first]);
});

// Task 8.1 on the mock host: the core's cursor tracking is played by `setDragOutcome`,
// `dragHover` and `dragDrop`; the real cross-window drag stays a manual check.
test("a tab dropped outside every window moves to a new window; the palette does the same", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+t");
  await page.keyboard.press("Meta+t");
  await expect.poll(() => tabIds(page)).toHaveLength(3);
  const [a, b, c] = await tabIds(page);

  await page.evaluate(() => (window as any).__terminescu.host.setDragOutcome("newWindow"));
  // Over a tab: dnd-kit alone would reorder, but the core saw the release outside.
  await drag(page, `[data-tab="${c}"]`, await center(page, `[data-tab="${a}"]`));
  await expect.poll(() => tabIds(page)).toEqual([a, b]);
  let ws = (await mock(page)).ws;
  expect(ws.windows).toHaveLength(2);
  expect(ws.windows[1].sideTabs[0].tabs.map((t: any) => t.id)).toEqual([c]);
  const begun = await page.evaluate(() => (window as any).__terminescu.host.dragsBegun as string[]);
  expect(begun.at(-1)).toMatch(new RegExp(`^${c} `));

  // The next drag is local again.
  await drag(page, `[data-tab="${b}"]`, await center(page, `[data-tab="${a}"]`));
  await expect.poll(() => tabIds(page)).toEqual([b, a]);

  await page.keyboard.press("Meta+k");
  await page.locator(".palette-item", { hasText: "New window" }).click();
  await expect.poll(() => tabIds(page)).toHaveLength(1);
  ws = (await mock(page)).ws;
  expect(ws.windows).toHaveLength(3);
});

test("a tab from another window lights the row under it and drops into it; its window closes", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+t");
  await expect.poll(() => tabIds(page)).toHaveLength(2);
  const [a, b] = await tabIds(page);
  // Two side tabs here; tab b goes to a second window.
  await page.keyboard.press("Meta+n");
  await expect(page.locator(".ao-st")).toHaveCount(2);
  await page.evaluate((b) => (window as any).__terminescu.host.send({ type: "MoveTabToNewWindow", tabId: b }), b);
  await page.keyboard.press("Control+Meta+1");
  await expect.poll(() => tabIds(page)).toEqual([a]);
  const firstSide = (await page.locator(".ao-st").first().getAttribute("data-side"))!;

  const row = await center(page, ".ao-st >> nth=-1");
  await page.evaluate(({ b, row }) => (window as any).__terminescu.host.dragHover({ tabId: b, ...row }), { b, row });
  await expect(page.locator(".ao-st").last()).toHaveClass(/remote-hot/);
  await expect(page.locator(".side-end")).toBeVisible();
  // Leaving clears the light.
  await page.evaluate(() => (window as any).__terminescu.host.dragHover(null));
  await expect(page.locator(".remote-hot")).toHaveCount(0);
  await expect(page.locator(".side-end")).toBeHidden();

  // Dropped on the tab bar: into the active side tab, before tab a.
  const onTab = await center(page, `[data-tab="${a}"]`);
  await page.evaluate(({ b, onTab }) => (window as any).__terminescu.host.dragDrop({ tabId: b, ...onTab }), { b, onTab });
  await expect.poll(() => tabIds(page)).toEqual([b, a]);
  const ws = (await mock(page)).ws;
  expect(ws.windows, "the emptied second window closes").toHaveLength(1);
  expect(ws.windows[0].sideTabs.find((s: any) => s.id === firstSide).tabs.map((t: any) => t.id)).toEqual([b, a]);
});
