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
  // Collapsed, every row's shell list carries its name and status.
  const tip = page.getByRole("group", { name: /^Shells in / });
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

// Task 8.28: a bigger icon, the status under the name, no tab count, and a row's shells beside it.
test("a row puts its status under the name and lists its shells on hover, one click away", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+t");
  await expect(page.locator(".ao-tab")).toHaveCount(2);
  const first = page.locator(".ao-st").first();
  await expect(first).not.toContainText(/\d tabs?\b/);
  const [icon, name, chip] = await first.evaluate((el) =>
    [el.querySelector(":scope > .ao-i")!, el.querySelector(".t")!, el.querySelector(".m .chip-btn")!].map((n) => n.getBoundingClientRect().toJSON()),
  );
  expect(icon.width).toBe(24);
  expect(chip.top).toBeGreaterThanOrEqual(name.bottom - 1);

  await page.getByRole("button", { name: "Add a side tab" }).click();
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
