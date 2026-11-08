import { expect, test, type Page } from "@playwright/test";

// Tasks 8.30 and 8.60 on the mock host: a tab is named "Terminal", a row with two tabs or more
// takes an animal or bird name, and a live subtitle follows its focused terminal: the path,
// then the program or `claude` and Claude's title.

const host = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });
const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
}

test("tabs are named Terminal, and only a row with two tabs or more takes an animal name", async ({ page }) => {
  await start(page);
  for (let i = 0; i < 2; i++) await page.getByRole("button", { name: "New tab", exact: true }).click();
  await page.getByRole("button", { name: "Add a side tab" }).click();
  const sides: { id: string; name: string; tabs: { id: string }[] }[] = (await workspace(page)).windows[0].sideTabs;
  expect(sides.map((s) => s.tabs.length)).toEqual([3, 1]);
  expect(sides[0]!.name).toMatch(/^[A-Z][a-z]+$/);
  expect(sides[0]!.name).not.toBe(sides[1]!.name);
  // The group shows its animal; the single row shows its tab's name.
  await expect(page.locator(".ao-st .t")).toHaveText([sides[0]!.name, "Terminal"]);
  const tab = sides[1]!.tabs[0]!.id;
  await expect(page.locator(`.ao-tab[data-tab="${tab}"] .n`)).toHaveText("Terminal");

  await host(page, "send", { type: "RenameTab", tabId: tab, title: "api" });
  await expect(page.locator(`.ao-tab[data-tab="${tab}"] .n`)).toHaveText("api");
  await expect(page.locator(".ao-st .t").nth(1)).toHaveText("api");
  // Clearing the name brings "Terminal" back.
  await host(page, "send", { type: "RenameTab", tabId: tab, title: null });
  await expect(page.locator(`.ao-tab[data-tab="${tab}"] .n`)).toHaveText("Terminal");

  // A group left with one tab shows that tab's name again.
  for (const t of sides[0]!.tabs.slice(1)) await host(page, "send", { type: "CloseTab", tabId: t.id });
  await expect(page.locator(".ao-st .t").first()).toHaveText("Terminal");
});

test("the subtitle keeps the path and follows the program and Claude's title", async ({ page }) => {
  await start(page);
  const tab = (await workspace(page)).windows[0].sideTabs[0].tabs[0];
  const pane = tab.layout.id;
  const sub = page.locator(`.ao-tab[data-tab="${tab.id}"] .tab-sub`);
  await expect(sub).toHaveText("~");
  // A one-tab row shows it under its name too.
  await expect(page.locator(".ao-st").first().locator(".st-sub")).toHaveText("~");

  await host(page, "setCwd", pane, "/Users/owner/work/api");
  await expect(sub).toHaveText("~/work/api");
  await host(page, "runCommand", pane, "TOKEN=tok-example npm run dev", null);
  await expect(sub).toHaveText("~/work/api · npm");
  await host(page, "runCommand", pane, "npm run dev", 0);
  await expect(sub).toHaveText("~/work/api");

  await host(page, "runCommand", pane, "claude", null);
  await host(page, "setAgent", pane, { session: "s-1", state: "running", lastMessage: null });
  await expect(sub).toHaveText("~/work/api · claude");
  // Claude's spinner never shows, and its default title adds nothing.
  await host(page, "setTitle", pane, "✳ Claude Code");
  await expect(sub).toHaveText("~/work/api · claude");
  await host(page, "setTitle", pane, "◐ Fix PROJ-482");
  await expect(sub).toHaveText("~/work/api · claude · Fix PROJ-482");
  // Hovering the tab shows the sidebar's tooltip under it: the whole name, then the subtitle (task 8.48).
  await page.locator(`.ao-tab[data-tab="${tab.id}"]`).hover();
  const tip = page.getByRole("tooltip");
  await expect(tip.locator(".px-tip-name")).toHaveText("Terminal");
  await expect(tip.locator(".px-tip-sub")).toHaveText("~/work/api · claude · Fix PROJ-482");
  const [t, b] = await Promise.all([tip.boundingBox(), page.locator(`.ao-tab[data-tab="${tab.id}"]`).boundingBox()]);
  expect(t!.y).toBeGreaterThan(b!.y + b!.height - 1);
  await page.mouse.move(600, 500);
  await expect(tip).toHaveCount(0);

  // Claude ends: the title goes with its command.
  await host(page, "setAgent", pane, null);
  await host(page, "runCommand", pane, "claude", 0);
  await expect(sub).toHaveText("~/work/api");
});

test("a split tab's subtitle follows its focused terminal, also in the group and the shell list", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "New tab", exact: true }).click();
  const made = (await workspace(page)).windows[0].sideTabs[0].tabs[1];
  await host(page, "send", { type: "SplitPane", paneId: made.layout.id, dir: "row" });
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  const side = (await workspace(page)).windows[0].sideTabs[0];
  const tab = side.tabs[1];
  const [left, right] = tab.layout.children.map((c: { id: string }) => c.id);
  await host(page, "setCwd", left, "/Users/owner/work/web");
  await host(page, "setCwd", right, "/Users/owner/work/api");
  const sub = page.locator(`.ao-tab[data-tab="${tab.id}"] .tab-sub`);
  await page.locator(`.app-pane .ao-tp[data-pane="${right}"]`).click();
  await expect(sub).toHaveText("~/work/api");
  await page.locator(`.app-pane .ao-tp[data-pane="${left}"]`).click();
  await expect(sub).toHaveText("~/work/web");
  // The open group lists the same subtitle beside the tab's name.
  await expect(page.locator(`.ao-st-kid[data-kid="${tab.id}"] .kid-sub`)).toHaveText("~/work/web");
  await expect(page.locator(`.ao-st-kid[data-kid="${tab.id}"] .kid-n`)).toHaveText("Terminal");
});
