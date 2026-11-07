import { expect, test, type Page } from "@playwright/test";

// Task 8.44 on the mock host: ⌘⇧K clears to the prompt line, ⌥⌘K drops the history and keeps
// the screen; both ask the daemon to clear its copy.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  const lines = Array.from({ length: 200 }, (_, i) => `build line ${i}`).join("\r\n") + "\r\n";
  await page.evaluate(
    ([id, text]) => new Promise<void>((done) => (window as any).__terminescu.terminals.get(id).write(text, done)),
    [pane, lines],
  );
  return pane;
}

const buffer = (page: Page, pane: string) =>
  page.evaluate((id) => {
    const t = (window as any).__terminescu.terminals.get(id);
    const b = t.buffer.active;
    return { length: b.length, rows: t.rows, base: b.baseY, top: b.getLine(0)?.translateToString(true) as string };
  }, pane);
const clears = (page: Page) => page.evaluate(() => [...(window as any).__terminescu.host.clears] as string[]);

test("⌥⌘K drops the history and keeps the screen", async ({ page }) => {
  const pane = await start(page);
  expect((await buffer(page, pane)).base).toBeGreaterThan(0);
  await page.keyboard.press("Meta+Alt+K");
  await expect.poll(async () => (await buffer(page, pane)).base).toBe(0);
  // The screen keeps its last lines.
  expect((await buffer(page, pane)).top).toMatch(/^build line \d+$/);
  expect(await clears(page)).toEqual([`${pane} history`]);
});

test("⌘⇧K clears to the prompt line", async ({ page }) => {
  const pane = await start(page);
  await page.keyboard.press("Meta+Shift+K");
  await expect.poll(async () => (await buffer(page, pane)).base).toBe(0);
  expect((await buffer(page, pane)).top).not.toMatch(/^build line/);
  expect(await clears(page)).toEqual([`${pane} to start`]);
});
