import { expect, test, type Page } from "@playwright/test";
import { addTab } from "./add-tab";

// Task 8.23 on the mock host: the sidebar's edge drags its width, a button on hover collapses
// it to the rows' icons, and pixel tooltips show a hovered icon's name and status, or a name the
// row cuts short.

const settings = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.settings());

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
}

const sideWidth = (page: Page) => page.locator(".app-side").evaluate((el) => Math.round(el.getBoundingClientRect().width));

test("dragging the sidebar's edge sets its width; a double click resets it", async ({ page }) => {
  await start(page);
  expect(await sideWidth(page)).toBe(272);
  const edge = page.getByRole("separator", { name: "Sidebar width" });
  const box = (await edge.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, 400);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 60, 400, { steps: 5 });
  await page.mouse.up();
  await expect.poll(() => sideWidth(page)).toBe(332);
  await expect.poll(async () => (await settings(page)).sidebarWidth).toBe(332);

  // Past the end of the range, the width stops at it.
  await page.mouse.move(box.x + 60 + box.width / 2, 400);
  await page.mouse.down();
  await page.mouse.move(1000, 400, { steps: 5 });
  await page.mouse.up();
  await expect.poll(async () => (await settings(page)).sidebarWidth).toBe(480);

  await edge.dblclick();
  await expect.poll(() => sideWidth(page)).toBe(272);
});

test("the collapse button shows on hover and leaves only icons with a tooltip", async ({ page }) => {
  await start(page);
  const toggle = page.getByRole("button", { name: "Collapse sidebar" });
  await page.mouse.move(1000, 400);
  await expect(toggle).toHaveCSS("opacity", "0");
  await page.locator(".ao-st").first().hover();
  await expect(toggle).toHaveCSS("opacity", "1");
  await toggle.click();
  await expect.poll(async () => (await settings(page)).sidebarCollapsed).toBe(true);
  expect(await sideWidth(page)).toBe(80);
  const row = page.locator(".ao-st").first();
  await expect(row.locator(".t")).toBeHidden();
  await expect(row.locator(".ao-i").first()).toBeVisible();

  await row.hover();
  // Collapsed, every row's shell list carries its name and status.
  const tip = page.getByRole("group", { name: /^Shells in / });
  await expect(tip.locator(".px-tip-name")).toHaveText("zsh");
  await expect(tip.locator(".px-tip-status")).toHaveText("Idle");
  await page.mouse.move(1000, 400);
  await expect(tip).toHaveCount(0);

  await row.hover();
  await page.getByRole("button", { name: "Expand sidebar" }).click();
  await expect(row.locator(".t")).toBeVisible();
  expect(await sideWidth(page)).toBe(272);
});

test("a name the row cuts short shows whole in a tooltip; one that fits does not", async ({ page }) => {
  await start(page);
  const name = page.locator(".ao-st .t").first();
  await name.hover();
  await expect(page.getByRole("tooltip")).toHaveCount(0);

  const long = "PROJ-482 review of the acme/api rate limiter rollout";
  await name.dblclick();
  await page.getByRole("textbox", { name: "Tab name" }).fill(long);
  await page.keyboard.press("Enter");
  await expect(name).toHaveText(long);
  await page.mouse.move(1000, 400);
  await name.hover();
  const tip = page.getByRole("tooltip");
  await expect(tip).toHaveText(long);
  // The tooltip starts where the name does, so the text reads on.
  const [t, n] = [(await tip.boundingBox())!, (await name.boundingBox())!];
  expect(Math.abs(t.x + 6 - n.x)).toBeLessThan(2);
});

// Task 8.28: a bigger icon, the status under the name, no tab count, and a row's shells beside it.
test("a row puts its status under the name and lists its shells on hover, one click away", async ({ page }) => {
  await start(page);
  await addTab(page);
  await expect(page.locator(".ao-tab")).toHaveCount(2);
  const first = page.locator(".ao-st").first();
  await expect(first).not.toContainText(/\d tabs?\b/);
  const [icon, name, chip] = await first.evaluate((el) =>
    [el.querySelector(":scope > .ao-i")!, el.querySelector(".t")!, el.querySelector(".m .chip-btn")!].map((n) => n.getBoundingClientRect().toJSON()),
  );
  expect(icon.width).toBe(24);
  expect(chip.top).toBeGreaterThanOrEqual(name.bottom - 1);

  await page.keyboard.press("Meta+Shift+KeyN");
  await expect(page.locator(".ao-st")).toHaveCount(2);
  await expect(page.locator(".ao-st").nth(1)).toHaveClass(/active/);

  await first.hover();
  const pick = page.getByRole("group", { name: /^Shells in / });
  await expect(pick.locator(".px-pick-item")).toHaveCount(2);
  // It sits beside the row, over the panes.
  const [p, r] = [(await pick.boundingBox())!, (await first.boundingBox())!];
  expect(p.x).toBeGreaterThanOrEqual(r.x + r.width);

  const second = pick.locator(".px-pick-item").nth(1);
  const id = await second.getAttribute("data-pick");
  await second.click();
  await expect(pick).toHaveCount(0);
  await expect(first).toHaveClass(/active/);
  await expect(page.locator(`.ao-st-kid[data-kid="${id}"]`)).toHaveClass(/active/);

  await page.mouse.move(1000, 400);
  await page.locator(".ao-st").nth(1).hover();
  await page.waitForTimeout(400);
  // A row with one shell needs no list while the sidebar is open.
  await expect(pick).toHaveCount(0);
});

// Task 8.57 and 8.58: tabs show only in the sidebar until Settings turns the top bar on, and a
// folder stays open or closed as the person leaves it.
test("tabs live in the sidebar by default and folders stay as left", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await addTab(page);
  await expect(page.locator(".ao-tabs .ao-tab")).toHaveCount(0);
  await expect(page.locator(".ao-st-kid")).toHaveCount(2);

  // Another row keeps this folder open; the chevron closes it. A click on the folder selects it
  // and opens it, and a click on the selected folder closes and opens it (task 8.61).
  await page.keyboard.press("Meta+Shift+KeyN");
  await expect(page.locator(".ao-st")).toHaveCount(2);
  await expect(page.locator(".ao-st-kid")).toHaveCount(2);
  await page.getByLabel("Collapse side tab").click();
  await expect(page.locator(".ao-st-kid")).toHaveCount(0);
  const folder = page.locator(".ao-st").first();
  await folder.click();
  await expect(folder).toHaveClass(/active/);
  await expect(page.locator(".ao-st-kid")).toHaveCount(2);
  await folder.click();
  await expect(page.locator(".ao-st-kid")).toHaveCount(0);
  await folder.click();
  await expect(page.locator(".ao-st-kid")).toHaveCount(2);

  // Settings › Terminal › Show tabs in the top bar brings them back.
  await page.evaluate(async () => {
    const h = (window as any).__terminescu.host;
    await h.setSettings({ ...(await h.settings()), topTabs: true });
  });
  await expect(page.locator(".ao-tabs .ao-tab")).toHaveCount(2);
});

// Task 8.59: with no tabs on top there is no top bar; the sidebar's + ▾ makes a text side tab,
// a folder's small + adds a tab to it, and the sidebar toggle sits on the sidebar's line.
test("the sidebar makes text side tabs and folder tabs, and the toggle sits on its line", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await expect(page.locator(".ao-tabs")).toHaveCount(0);
  const pane = await page.locator(".app-pane").boundingBox();
  expect(pane!.y).toBeLessThan(2);

  // + ▾ › Text: a new row holding only an untitled text.
  await page.locator(".side-fill").hover();
  await page.getByRole("button", { name: "Choose side tab kind" }).click();
  await page.getByRole("menuitem", { name: /Text/ }).click();
  await expect(page.locator(".ao-st")).toHaveCount(2);
  await expect(page.locator(".ao-st").last()).toHaveClass(/active/);
  await expect(page.locator(".app-pane .text-pane")).toHaveCount(1);
  await expect(page.locator(".app-pane .term-pane:not(:has(.text-pane))")).toHaveCount(0);
  await expect.poll(async () => (await page.evaluate(() => (window as any).__terminescu.host.workspace())).windows[0].sideTabs[1].tabs.length).toBe(1);

  // The first row becomes a folder; its small + adds a third tab there.
  await page.locator(".ao-st").first().click();
  await addTab(page);
  await expect(page.locator(".ao-st-kid")).toHaveCount(2);
  await page.locator(".ao-st-kids").hover();
  await page.getByRole("button", { name: /^New tab in / }).click();
  await expect(page.locator(".ao-st-kid")).toHaveCount(3);
  await expect(page.locator(".ao-st-kid").last()).toHaveClass(/active/);

  // A row under the mouse gets the amber line; the others do not.
  await page.locator(".ao-st").last().hover();
  await expect.poll(() => page.locator(".ao-st").last().evaluate((el) => getComputedStyle(el, "::before").boxShadow)).toContain("2px");
  expect(await page.locator(".ao-st").first().evaluate((el) => getComputedStyle(el, "::before").content)).toBe("none");
  await page.locator(".ao-st-kid").first().hover();
  await expect.poll(() => page.locator(".ao-st-kid").first().evaluate((el) => getComputedStyle(el, "::before").boxShadow)).toContain("2px");

  // The toggle shows on hover, centred on the sidebar's right line, by the wordmark.
  await page.locator(".app-side").hover();
  const side = (await page.locator(".app-side").boundingBox())!;
  const toggle = (await page.getByRole("button", { name: "Collapse sidebar" }).boundingBox())!;
  expect(Math.abs(toggle.x + toggle.width / 2 - (side.x + side.width - 1))).toBeLessThanOrEqual(1);
  expect(toggle.y).toBeLessThan(34);
});

test("a one-tab row is one line, as tall as a folder's tab, with a subtitle beside its name", async ({ page }) => {
  // Task 8.77.
  await start(page);
  await page.keyboard.press("Meta+Shift+KeyN");
  await page.getByRole("button", { name: "New tab", exact: true }).click();
  await expect(page.locator(".ao-st-kid")).toHaveCount(2);
  const single = page.locator(".ao-st.single");
  await expect(single).toHaveCount(1);
  const height = (l: ReturnType<Page["locator"]>) => l.evaluate((e) => Math.round(e.getBoundingClientRect().height));
  expect(await height(single)).toBe(await height(page.locator(".ao-st-kid").first()));
  await expect(single.locator(".st-sub")).toHaveText("~");
  // A folder's header keeps its two lines and its squares.
  const folder = page.locator(".ao-st:not(.single)");
  expect(await height(folder)).toBeGreaterThan(await height(single));
  await expect(folder.locator(".ao-cell")).toHaveCount(2);
});
