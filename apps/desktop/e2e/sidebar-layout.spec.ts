import { expect, test, type Page } from "@playwright/test";

// Task 8.23 on the mock host: the sidebar's edge drags its width, a button on hover collapses
// it to the rows' icons, and pixel tooltips show a hovered icon's name and status, or a name the
// row cuts short.

const settings = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.settings());

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
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
  const tip = page.getByRole("tooltip");
  await expect(tip).toContainText("shell");
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
  await page.getByRole("textbox", { name: "Side tab name" }).fill(long);
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
