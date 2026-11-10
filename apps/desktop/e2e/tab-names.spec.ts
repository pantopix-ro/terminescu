import { expect, test, type Page } from "@playwright/test";

// Tasks 8.30, 8.60 and 8.62 on the mock host: a tab is named after what runs in its terminal, as
// Terminal.app names one (`zsh`, `npm`, `claude`), a row with two tabs or more takes an animal or bird name, and a live subtitle follows its focused terminal: the path,
// then the program or `claude` and Claude's title.

const host = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });
const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
}

test("tabs are named after their terminal, and only a row with two tabs or more takes an animal name", async ({ page }) => {
  await start(page);
  for (let i = 0; i < 2; i++) await page.getByRole("button", { name: "New tab", exact: true }).click();
  await page.keyboard.press("Meta+Shift+KeyN");
  const sides: { id: string; name: string; tabs: { id: string }[] }[] = (await workspace(page)).windows[0].sideTabs;
  expect(sides.map((s) => s.tabs.length)).toEqual([3, 1]);
  expect(sides[0]!.name).toMatch(/^[A-Z][a-z]+$/);
  expect(sides[0]!.name).not.toBe(sides[1]!.name);
  // The folder shows its animal; the single row shows its tab's name.
  await expect(page.locator(".ao-st .t")).toHaveText([sides[0]!.name, "zsh"]);
  const tab = sides[1]!.tabs[0]!.id;
  await expect(page.locator(`.ao-tab[data-tab="${tab}"] .n`)).toHaveText("zsh");

  await host(page, "send", { type: "RenameTab", tabId: tab, title: "api" });
  await expect(page.locator(`.ao-tab[data-tab="${tab}"] .n`)).toHaveText("api");
  await expect(page.locator(".ao-st .t").nth(1)).toHaveText("api");
  // Clearing the name brings the shell's name back.
  await host(page, "send", { type: "RenameTab", tabId: tab, title: null });
  await expect(page.locator(`.ao-tab[data-tab="${tab}"] .n`)).toHaveText("zsh");

  // A renamed folder still shows its animal in the list its row shows on hover.
  await host(page, "send", { type: "RenameSideTab", sideTabId: sides[0]!.id, title: "work" });
  await page.locator(".ao-st").first().hover();
  await expect(page.getByRole("group", { name: "Shells in work" }).locator(".px-tip-alias")).toHaveText(sides[0]!.name);
  await page.mouse.move(900, 500);
  await host(page, "send", { type: "RenameSideTab", sideTabId: sides[0]!.id, title: null });

  // The name follows what runs: the command, then Claude, then the shell again.
  const pane = (await workspace(page)).windows[0].sideTabs[1].tabs[0].layout.id as string;
  await host(page, "runCommand", pane, "npm run dev", null);
  await expect(page.locator(`.ao-tab[data-tab="${tab}"] .n`)).toHaveText("npm");
  await host(page, "runCommand", pane, "npm run dev", 0);
  await host(page, "runCommand", pane, "claude", null);
  await host(page, "setAgent", pane, { session: "s-1", state: "running", lastMessage: null });
  await expect(page.locator(`.ao-tab[data-tab="${tab}"] .n`)).toHaveText("claude");
  await host(page, "setAgent", pane, null);
  await host(page, "runCommand", pane, "claude", 0);
  await expect(page.locator(`.ao-tab[data-tab="${tab}"] .n`)).toHaveText("zsh");

  // A folder left with one tab shows that tab's name again.
  for (const t of sides[0]!.tabs.slice(1)) await host(page, "send", { type: "CloseTab", tabId: t.id });
  await expect(page.locator(".ao-st .t").first()).toHaveText("zsh");
});

test("the name follows the program and the subtitle keeps the path and Claude's title", async ({ page }) => {
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
  // The tab is named "npm" (task 8.62), so the subtitle leaves it out.
  await expect(page.locator(`.ao-tab[data-tab="${tab.id}"] .n`)).toHaveText("npm");
  await expect(sub).toHaveText("~/work/api");
  await host(page, "runCommand", pane, "npm run dev", 0);
  await expect(sub).toHaveText("~/work/api");

  await host(page, "runCommand", pane, "claude", null);
  await host(page, "setAgent", pane, { session: "s-1", state: "running", lastMessage: null });
  await expect(sub).toHaveText("~/work/api");
  // Claude's spinner never shows, and its default title adds nothing.
  await host(page, "setTitle", pane, "✳ Claude Code");
  await expect(sub).toHaveText("~/work/api");
  await host(page, "setTitle", pane, "◐ Fix PROJ-482");
  await expect(sub).toHaveText("~/work/api · Fix PROJ-482");
  // Hovering the tab shows the sidebar's tooltip under it: the whole name, then the subtitle (task 8.48).
  await page.locator(`.ao-tab[data-tab="${tab.id}"]`).hover();
  const tip = page.getByRole("tooltip");
  await expect(tip.locator(".px-tip-name")).toHaveText("claude");
  await expect(tip.locator(".px-tip-sub")).toHaveText("~/work/api · Fix PROJ-482");
  const [t, b] = await Promise.all([tip.boundingBox(), page.locator(`.ao-tab[data-tab="${tab.id}"]`).boundingBox()]);
  expect(t!.y).toBeGreaterThan(b!.y + b!.height - 1);
  await page.mouse.move(600, 500);
  await expect(tip).toHaveCount(0);

  // Claude ends: the title goes with its command.
  await host(page, "setAgent", pane, null);
  await host(page, "runCommand", pane, "claude", 0);
  await expect(sub).toHaveText("~/work/api");
});

test("a split tab's subtitle follows its focused terminal, also in the folder and the shell list", async ({ page }) => {
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
  // The open folder lists the same subtitle beside the tab's name.
  await expect(page.locator(`.ao-st-kid[data-kid="${tab.id}"] .kid-sub`)).toHaveText("~/work/web");
  await expect(page.locator(`.ao-st-kid[data-kid="${tab.id}"] .kid-n`)).toHaveText("zsh");
});
