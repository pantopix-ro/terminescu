import { expect, test, type Page } from "@playwright/test";

// Task 2.5: 20 panes over 2 windows; every visible pane renders. Each page is one window on
// its own mock Host.

interface PaneState {
  tabId: string;
  renderer: string | undefined;
  hidden: boolean;
  firstLine: string;
  cols: number;
  rows: number;
}

async function panes(page: Page): Promise<PaneState[]> {
  return page.evaluate(() => {
    const { terminals } = (window as any).__terminescu as { terminals: Map<string, any> };
    return [...document.querySelectorAll<HTMLElement>(".term-pane")].map((el) => {
      const term = terminals.get(el.dataset.tab!);
      return {
        tabId: el.dataset.tab!,
        renderer: el.dataset.renderer,
        hidden: el.hidden,
        firstLine: term?.buffer.active.getLine(0)?.translateToString(true) ?? "",
        cols: term?.cols ?? 0,
        rows: term?.rows ?? 0,
      };
    });
  });
}

/** Opens a window with two side tabs of five tabs each. */
async function openWindow(page: Page) {
  await page.goto("/");
  for (let i = 0; i < 4; i++) await page.getByLabel("New tab").click();
  await page.getByLabel("New side tab").click();
  for (let i = 0; i < 4; i++) await page.getByLabel("New tab").click();
  await expect(page.locator(".term-pane")).toHaveCount(10);
}

async function checkVisible(page: Page) {
  await expect
    .poll(async () => {
      const visible = (await panes(page)).filter((p) => !p.hidden);
      return visible.length === 1 && visible[0]!.firstLine.startsWith(`mock ${visible[0]!.tabId} $`);
    })
    .toBe(true);
  const all = await panes(page);
  const shown = all.find((p) => !p.hidden)!;
  expect(shown.renderer, "the visible pane takes WebGL").toBe("webgl");
  expect(all.filter((p) => p.hidden && p.renderer === "webgl"), "hidden panes give WebGL back").toEqual([]);
  const box = await page.locator(`.term-pane[data-tab="${shown.tabId}"] canvas`).first().boundingBox();
  expect(box?.width ?? 0).toBeGreaterThan(100);
}

test("20 panes over 2 windows: every visible pane renders", async ({ browser }) => {
  const windows = await Promise.all([browser.newPage(), browser.newPage()]);
  const errors: string[] = [];
  for (const page of windows) page.on("pageerror", (e) => errors.push(String(e)));
  await Promise.all(windows.map(openWindow));

  for (const page of windows) {
    for (const side of await page.locator(".ao-st").all()) {
      await side.click();
      for (const tab of await page.getByRole("tab").all()) {
        await tab.click();
        await checkVisible(page);
      }
    }
  }
  expect(errors).toEqual([]);
});

test("a pane takes keys and follows the window size", async ({ page }) => {
  await page.setViewportSize({ width: 1200, height: 760 });
  await page.goto("/");
  await expect.poll(async () => (await panes(page))[0]?.firstLine ?? "").toMatch(/^mock \S+ \$ $/);
  const pane = page.locator(".term-pane").first();
  await pane.click();
  await page.keyboard.type("size");
  await page.keyboard.press("Enter");
  const [first] = await panes(page);
  await expect.poll(() => line(page, 1)).toBe(`${first!.cols}x${first!.rows}`);

  await page.setViewportSize({ width: 900, height: 600 });
  await expect.poll(async () => (await panes(page))[0]!.cols).toBeLessThan(first!.cols);
  await page.keyboard.type("size");
  await page.keyboard.press("Enter");
  const [after] = await panes(page);
  await expect.poll(() => line(page, 3)).toBe(`${after!.cols}x${after!.rows}`);
});

test("a ⌘-click on a link opens it; a plain click does not", async ({ page }) => {
  await page.goto("/");
  await expect.poll(async () => (await panes(page))[0]?.firstLine ?? "").toMatch(/^mock /);
  await page.locator(".term-pane").first().click();
  await page.keyboard.type("https://example.com/docs");
  await page.keyboard.press("Enter");
  await expect.poll(() => line(page, 1)).toContain("https://example.com/docs");
  const cell = await cellOf(page, 1, "https://");
  await page.mouse.move(cell.x, cell.y);
  await page.mouse.click(cell.x, cell.y);
  await page.keyboard.down("Meta");
  await page.mouse.move(cell.x + 2, cell.y);
  await page.mouse.click(cell.x + 2, cell.y);
  await page.keyboard.up("Meta");
  await expect
    .poll(() => page.evaluate(() => (window as any).__terminescu.host.openedUrls as string[]))
    .toEqual(["https://example.com/docs"]);
});

function line(page: Page, y: number): Promise<string> {
  return page.evaluate((y) => {
    const term = [...(window as any).__terminescu.terminals.values()][0];
    return term.buffer.active.getLine(y)?.translateToString(true) ?? "";
  }, y);
}

/** The page position of the first cell of `needle` on row `y` of the first pane. */
async function cellOf(page: Page, y: number, needle: string) {
  return page.evaluate(
    ({ y, needle }) => {
      const term = [...(window as any).__terminescu.terminals.values()][0];
      const x = term.buffer.active.getLine(y).translateToString(true).indexOf(needle);
      const screen = document.querySelector(".term-pane .xterm-screen")!.getBoundingClientRect();
      const cw = screen.width / term.cols;
      const ch = screen.height / term.rows;
      return { x: screen.left + (x + 2) * cw, y: screen.top + (y + 0.5) * ch };
    },
    { y, needle },
  );
}
