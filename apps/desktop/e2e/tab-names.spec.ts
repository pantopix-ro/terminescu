import { expect, test, type Page } from "@playwright/test";

// Task 8.30 on the mock host: every tab gets an animal or bird name, and a live subtitle
// follows its focused terminal: the path, then the program or `claude` and Claude's title.

const host = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });
const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
}

test("new tabs take different animal names, and a rename overrides one", async ({ page }) => {
  await start(page);
  for (let i = 0; i < 2; i++) await page.getByRole("button", { name: "New tab", exact: true }).click();
  await page.getByRole("button", { name: "Add a side tab" }).click();
  const sides = (await workspace(page)).windows[0].sideTabs;
  const tabs: { id: string; name: string }[] = sides.flatMap((s: { tabs: unknown[] }) => s.tabs);
  expect(tabs).toHaveLength(4);
  expect(new Set(tabs.map((t) => t.name)).size).toBe(4);
  expect(tabs.every((t) => /^[A-Z][a-z]+$/.test(t.name))).toBe(true);
  // The row takes its first tab's name; the second row is the active one.
  await expect(page.locator(".ao-st .t")).toHaveText([tabs[0]!.name, tabs[3]!.name]);
  await expect(page.locator(`.ao-tab[data-tab="${tabs[3]!.id}"] .n`)).toHaveText(tabs[3]!.name);

  await host(page, "send", { type: "RenameTab", tabId: tabs[3]!.id, title: "api" });
  await expect(page.locator(`.ao-tab[data-tab="${tabs[3]!.id}"] .n`)).toHaveText("api");
  // Clearing the name brings the animal back.
  await host(page, "send", { type: "RenameTab", tabId: tabs[3]!.id, title: null });
  await expect(page.locator(`.ao-tab[data-tab="${tabs[3]!.id}"] .n`)).toHaveText(tabs[3]!.name);
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
  await expect(tip.locator(".px-tip-name")).toHaveText(tab.name);
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
  await expect(page.locator(`.ao-st-kid[data-kid="${tab.id}"] .kid-n`)).toHaveText(tab.name);
});
