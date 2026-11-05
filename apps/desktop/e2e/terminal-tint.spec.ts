import { expect, test, type Page } from "@playwright/test";

// Task 8.14 on the mock host: a status background tints the running terminal and its margin,
// live; a tab's own status wins over its side tab's.

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

const background = (page: Page, pane: string) =>
  page.evaluate((id) => (window as any).__terminescu.terminals.get(id).options.theme.background, pane);

const slot = (page: Page) => page.locator(".pane-slot").first().evaluate((el) => getComputedStyle(el).backgroundColor);

const look = (background: string | null) => ({ label: "Tag", accent: null, background });

test("a status background tints the terminal live, the tab's own status first", async ({ page }) => {
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  const side = (await workspace(page)).windows[0].sideTabs[0];
  const tab = side.tabs[0];
  const pane = tab.layout.id as string;
  const panel = await background(page, pane);

  await mock(page, "send", { type: "SetStatus", targetId: side.id, status: look("blue") });
  await expect.poll(() => background(page, pane)).toBe("#212f43");
  await expect.poll(() => slot(page)).toBe("rgb(33, 47, 67)");

  await mock(page, "send", { type: "SetStatus", targetId: tab.id, status: look("magenta") });
  await expect.poll(() => background(page, pane)).toBe("#2f2b42");

  await mock(page, "send", { type: "SetStatus", targetId: tab.id, status: null });
  await mock(page, "send", { type: "SetStatus", targetId: side.id, status: null });
  await expect.poll(() => background(page, pane)).toBe(panel);
  await expect.poll(() => slot(page)).not.toBe("rgb(33, 47, 67)");
});
