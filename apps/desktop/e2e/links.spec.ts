import { expect, test, type Page } from "@playwright/test";

// Task 8.49 on the mock host: a plain click on a terminal link copies it and says so; a right
// click opens a pixel menu with the apps for its kind; ⌘-click still opens it.

async function start(page: Page) {
  await page.addInitScript(() => {
    (window as any).__copied = [];
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: async (t: string) => void (window as any).__copied.push(t) },
    });
  });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  await page.evaluate(
    (id) =>
      new Promise<void>((done) =>
        (window as any).__terminescu.terminals
          .get(id)
          .write("\x1b[2J\x1b[Hhttps://example.com/docs\r\n./notes/todo.md\r\nops@example.com\r\n", done),
      ),
    pane,
  );
  return pane;
}

/** The window point of a terminal cell's middle, row and column from 0. */
const cell = (page: Page, pane: string, row: number, col: number) =>
  page.evaluate(
    ({ id, row, col }) => {
      const t = (window as any).__terminescu.terminals.get(id);
      const r = (t.element as HTMLElement).querySelector(".xterm-screen")!.getBoundingClientRect();
      return { x: r.left + ((col + 0.5) * r.width) / t.cols, y: r.top + ((row + 0.5) * r.height) / t.rows };
    },
    { id: pane, row, col },
  );

async function at(page: Page, pane: string, row: number) {
  const p = await cell(page, pane, row, 4);
  await page.mouse.move(p.x - 20, p.y);
  await page.mouse.move(p.x, p.y);
  return p;
}

const copied = (page: Page) => page.evaluate(() => (window as any).__copied as string[]);
const opened = (page: Page) => page.evaluate(() => [...(window as any).__terminescu.host.openedUrls] as string[]);

test("a click copies a link and shows a pixel message; ⌘-click opens it", async ({ page }) => {
  const pane = await start(page);
  const p = await at(page, pane, 0);
  await page.mouse.click(p.x, p.y);
  await expect(page.locator(".link-tip")).toHaveText("Copied link");
  expect(await copied(page)).toEqual(["https://example.com/docs"]);
  expect(await opened(page)).toEqual([]);
  await expect(page.locator(".link-tip")).toHaveCount(0, { timeout: 4000 });

  await page.keyboard.down("Meta");
  await page.mouse.click(p.x, p.y);
  await page.keyboard.up("Meta");
  await expect.poll(() => opened(page)).toEqual(["https://example.com/docs"]);

  const e = await at(page, pane, 2);
  await page.mouse.click(e.x, e.y);
  await expect(page.locator(".link-tip")).toHaveText("Copied address");
  expect((await copied(page)).at(-1)).toBe("ops@example.com");
});

test("a right click shows the apps for the link's kind", async ({ page }) => {
  const pane = await start(page);
  let p = await at(page, pane, 0);
  await page.mouse.click(p.x, p.y, { button: "right" });
  const menu = page.getByRole("menu", { name: "Link" });
  await expect(menu.getByRole("menuitem")).toHaveText(["Open in Browser", "Copy Link"]);
  await menu.getByRole("menuitem", { name: "Open in Browser" }).click();
  await expect.poll(() => opened(page)).toEqual(["https://example.com/docs"]);

  p = await at(page, pane, 1);
  await page.mouse.click(p.x, p.y, { button: "right" });
  const paths = page.getByRole("menu", { name: "Path" });
  await expect(paths.getByRole("menuitem")).toHaveText(["Open in Text Tab", "Reveal in Finder", "Open with Default App", "Copy Path"]);
  await paths.getByRole("menuitem", { name: "Reveal in Finder" }).click();
  await expect.poll(() => opened(page)).toContain("reveal:./notes/todo.md");

  p = await at(page, pane, 2);
  await page.mouse.click(p.x, p.y, { button: "right" });
  await page.getByRole("menu", { name: "Email address" }).getByRole("menuitem", { name: "Compose in Mail" }).click();
  await expect.poll(() => opened(page)).toContain("mailto:ops@example.com");

  // Off a link, a right click opens no link menu.
  const blank = await cell(page, pane, 6, 30);
  await page.mouse.move(blank.x, blank.y);
  await page.mouse.click(blank.x, blank.y, { button: "right" });
  await expect(page.getByRole("menu")).toHaveCount(0);
});

test("a double click or a drag inside a link selects text and copies no link", async ({ page }) => {
  const pane = await start(page);
  const p = await at(page, pane, 0);
  await page.mouse.dblclick(p.x, p.y);
  await page.waitForTimeout(500);
  await expect(page.locator(".link-tip")).toHaveCount(0);
  const q = await cell(page, pane, 0, 14);
  await at(page, pane, 0);
  await page.mouse.down();
  await page.mouse.move(q.x, q.y, { steps: 5 });
  await page.mouse.up();
  await page.waitForTimeout(300);
  await expect(page.locator(".link-tip")).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).__copied as string[])).not.toContain("https://example.com/docs");
  // A plain click still copies it.
  await page.mouse.click(p.x, p.y);
  await expect(page.locator(".link-tip")).toHaveText("Copied link");
});
