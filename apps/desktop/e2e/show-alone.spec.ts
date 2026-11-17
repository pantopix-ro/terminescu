import { expect, test, type Page } from "@playwright/test";
import { addTab } from "./add-tab";

// Tasks 2.1 to 2.3: Show Alone (⌥⌘M) leaves the focused pane as the only thing in the window.
// The tab's other panes become tabs, or rows, with their shells running.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
  await page.locator(".term-pane").first().click();
}

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());
const send = (page: Page, command: object) => page.evaluate((c) => (window as any).__terminescu.host.send(c), command);
const sides = async (page: Page) => (await workspace(page)).windows[0].sideTabs as any[];
const panesIn = (l: any): string[] => (l.type === "pane" ? [l.id] : l.children.flatMap(panesIn));
const sideIds = (page: Page) => page.locator(".ao-st").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.side!));

/** Splits the selected tab twice and returns its three panes. */
async function threePanes(page: Page) {
  await page.keyboard.press("Meta+d");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  await page.keyboard.press("Meta+d");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(3);
  const s = await sides(page);
  return panesIn(s.find((x) => x.tabs.length)!.tabs[0].layout);
}

test("the menu, palette and head all offer Show Alone", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+d");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  await expect(page.locator(".ao-tp-head button[aria-label='Show alone']")).toHaveCount(2);
  await page.keyboard.press("Meta+p");
  await page.keyboard.type("show alone");
  await expect(page.locator("[role=option], .ao-palette li, .palette li").first()).toContainText("Show Alone");
  await page.keyboard.press("Escape");
  await page.locator(".ao-tp").first().click({ button: "right" });
  await expect(page.getByText("Show Alone")).toBeVisible();
});

test("a one-tab row splits into rows, the row keeps one tab", async ({ page }) => {
  await start(page);
  const [a, b, c] = await threePanes(page);
  const rowsBefore = (await sides(page)).length;
  await page.locator(`.ao-tp[data-pane="${a}"]`).click();
  await page.keyboard.press("Meta+Alt+m");
  await expect.poll(async () => (await sides(page)).length).toBe(rowsBefore + 2);
  const s = await sides(page);
  expect(s[0].tabs).toHaveLength(1);
  expect(panesIn(s[0].tabs[0].layout)).toEqual([a]);
  expect(s[1].tabs).toHaveLength(1);
  expect(panesIn(s[1].tabs[0].layout)).toEqual([b]);
  expect(panesIn(s[2].tabs[0].layout)).toEqual([c]);
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
  await expect(page.locator(".app-pane .ao-tp")).toHaveAttribute("data-pane", a!);
});

test("a split tab in a folder, beside another area, moves panes to tabs after it", async ({ page }) => {
  await start(page);
  await addTab(page);
  const side = (await sides(page))[0];
  expect(side.tabs).toHaveLength(2);
  await page.keyboard.press("Meta+d");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  const tab = (await sides(page))[0].tabs[1];
  const [server, tests] = panesIn(tab.layout);
  await page.locator(`.ao-tp[data-pane="${server}"]`).click();
  await page.keyboard.press("Meta+Alt+m");
  await expect.poll(async () => (await sides(page))[0].tabs.length).toBe(3);
  const tabs = (await sides(page))[0].tabs;
  expect(panesIn(tabs[1].layout)).toEqual([server]);
  expect(panesIn(tabs[2].layout)).toEqual([tests]);
  expect((await sides(page)).length).toBe(1);
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
});

test("hides the other areas of the view but keeps their rows", async ({ page }) => {
  await start(page);
  const [first] = await sideIds(page);
  await page.keyboard.press("Meta+Shift+KeyN");
  await expect.poll(async () => (await sideIds(page)).length).toBe(2);
  const second = (await sideIds(page))[1]!;
  const from = (await page.locator(`[data-side="${first}"]`).boundingBox())!;
  const to = (await page.locator(".app-pane").boundingBox())!;
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2 + 10, from.y + from.height / 2, { steps: 3 });
  await page.mouse.move(to.x + to.width * 0.97, to.y + to.height / 2, { steps: 8 });
  await page.mouse.up();
  await expect(page.locator(".view-head")).toHaveCount(2);
  await page.locator(".view-head button[aria-label='Show alone']").first().click();
  await expect(page.locator(".view-head")).toHaveCount(0);
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
  expect(await sideIds(page)).toContain(second);
  expect(await sideIds(page)).toContain(first);
});

test("already alone says so, and ⌘⇧M still toggles a maximize", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Alt+m");
  await expect(page.locator(".app-note")).toHaveText("Already alone");
  await page.keyboard.press("Meta+d");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  const tabs = async () => (await sides(page)).flatMap((s) => s.tabs).length;
  const before = await tabs();
  await page.keyboard.press("Meta+Shift+m");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(1);
  await page.keyboard.press("Meta+Shift+m");
  await expect(page.locator(".app-pane .ao-tp")).toHaveCount(2);
  expect(await tabs()).toBe(before);
  void send;
});
