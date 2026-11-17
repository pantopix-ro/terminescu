import { expect, test, type Page } from "@playwright/test";
import { addTab } from "./add-tab";

// Tasks 3.3, 3.4, 5.2, 5.3: focus picks what zooms. A pane scales its own text, a sidebar row
// or folder every pane of its tabs, window chrome the window.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
  await page.locator(".term-pane").first().click();
}

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());
const send = (page: Page, command: object) => page.evaluate((c) => (window as any).__terminescu.host.send(c), command);
const panesIn = (l: any): string[] => (l.type === "pane" ? [l.id] : l.children.flatMap(panesIn));
const leaves = (l: any): any[] => (l.type === "pane" ? [l] : l.children.flatMap(leaves));
const sides = async (page: Page) => (await workspace(page)).windows[0].sideTabs as any[];
const paneZoom = async (page: Page, pane: string) => {
  const found = (await sides(page)).flatMap((s) => s.tabs).flatMap((t) => leaves(t.layout)).find((l) => l.id === pane);
  return (found.zoom ?? 100) as number;
};
const fontOf = (page: Page, pane: string) => page.evaluate((id) => (window as any).__terminescu.terminals.get(id).options.fontSize as number, pane);
const note = (page: Page) => page.locator(".app-note");

test("zoom one pane: its font scales unrounded and the other pane stays", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+d");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  const [server, tests] = panesIn((await sides(page))[0].tabs[0].layout) as [string, string];
  const base = await fontOf(page, tests);
  await page.locator(`.ao-tp[data-pane="${tests}"] .term-pane`).click();
  await page.keyboard.press("Meta+Equal");
  await page.keyboard.press("Meta+Equal");
  await expect.poll(() => paneZoom(page, tests)).toBe(125);
  await expect(note(page)).toContainText("· 125%");
  await expect(note(page)).toContainText("Pane ");
  await expect.poll(() => fontOf(page, tests)).toBeCloseTo(base * 1.25, 5);
  expect(await paneZoom(page, server)).toBe(100);
  expect(await fontOf(page, server)).toBe(base);
  // ⌘0 puts the pane back.
  await page.keyboard.press("Meta+0");
  await expect.poll(() => paneZoom(page, tests)).toBe(100);
  await expect.poll(() => fontOf(page, tests)).toBe(base);
});

test("plain + reaches the terminal and zooms nothing", async ({ page }) => {
  await start(page);
  await page.keyboard.type("+");
  expect(await paneZoom(page, panesIn((await sides(page))[0].tabs[0].layout)[0]!)).toBe(100);
  expect((await workspace(page)).windows[0].zoomPercent).toBeUndefined();
});

test("zoom a folder: every pane of its tabs, and panes outside keep theirs", async ({ page }) => {
  await start(page);
  await addTab(page);
  await page.keyboard.press("Meta+d");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  await page.keyboard.press("Meta+Shift+KeyN");
  await expect(page.locator(".ao-st")).toHaveCount(2);
  const all = await sides(page);
  const folder = all[0];
  const folderPanes = folder.tabs.flatMap((t: any) => panesIn(t.layout)) as string[];
  expect(folderPanes).toHaveLength(3);
  const outside = panesIn(all[1].tabs[0].layout)[0]!;
  await page.locator(`.ao-st[data-side="${folder.id}"]`).focus();
  await page.keyboard.press("Meta+Equal");
  for (const p of folderPanes) await expect.poll(() => paneZoom(page, p)).toBe(110);
  await expect(note(page)).toContainText("· 3 panes · 110%");
  await expect(note(page)).toContainText("Folder ");
  expect(await paneZoom(page, outside)).toBe(100);
  await page.keyboard.press("Meta+Minus");
  await expect(note(page)).toContainText("· 3 panes · 100%");
});

test("folder panes line up on the selected pane's step", async ({ page }) => {
  await start(page);
  await addTab(page);
  await page.keyboard.press("Meta+d");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  const folder = (await sides(page))[0];
  const [a, b] = panesIn(folder.tabs[1].layout) as [string, string];
  await send(page, { type: "SetPaneZoom", paneIds: [a], zoom: 90 });
  await send(page, { type: "SetPaneZoom", paneIds: [b], zoom: 125 });
  // The selected pane is the focused one of the shown tab; make it the one at 90%.
  await page.locator(`.ao-tp[data-pane="${a}"] .term-pane`).click();
  await page.locator(`.ao-st[data-side="${folder.id}"]`).focus();
  await page.keyboard.press("Meta+Equal");
  await expect.poll(() => paneZoom(page, a)).toBe(100);
  await expect.poll(() => paneZoom(page, b)).toBe(100);
});

test("the pane zoom ends at 200% with a max notice", async ({ page }) => {
  await start(page);
  const pane = panesIn((await sides(page))[0].tabs[0].layout)[0]!;
  await send(page, { type: "SetPaneZoom", paneIds: [pane], zoom: 200 });
  await expect.poll(() => paneZoom(page, pane)).toBe(200);
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Equal");
  await expect(note(page)).toContainText("· 200% (max)");
});

test("zoom a window: only the focused window's zoom changes", async ({ page }) => {
  await start(page);
  // A second window in the workspace; this page shows the first.
  await page.keyboard.press("Meta+Shift+KeyN");
  await expect.poll(async () => (await sides(page)).length).toBe(2);
  await send(page, { type: "MoveSideTabToNewWindow", sideTabId: (await sides(page))[1].id });
  await expect.poll(async () => (await workspace(page)).windows.length).toBe(2);
  await page.locator(".app-traffic").click();
  await page.keyboard.press("Meta+Equal");
  await expect(note(page)).toHaveText("Window · 110%");
  await expect.poll(async () => (await workspace(page)).windows[0].zoomPercent).toBe(110);
  for (const w of (await workspace(page)).windows.slice(1)) expect(w.zoomPercent).toBeUndefined();
  expect(await page.evaluate(() => document.documentElement.style.zoom)).toBe("1.1");
  // The Settings default does not move.
  expect((await page.evaluate(() => (window as any).__terminescu.host.settings())).zoomPercent).toBe(100);
});
