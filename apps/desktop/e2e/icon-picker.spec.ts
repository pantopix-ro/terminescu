import { expect, test, type Page } from "@playwright/test";

// Task 8.16 on the mock host: a double click on a side tab's or a tab's icon opens the icon
// list; a pick changes the icon and "Default icon" brings the old one back.

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
}

test("a double click on a side tab's icon picks another and resets it", async ({ page }) => {
  await start(page);
  const icon = page.locator(".ao-st > .ao-i").first();
  await expect(icon).toHaveClass(/ao-i-term/);
  await icon.dblclick();
  const picker = page.getByRole("dialog", { name: /^Icon for / });
  await expect(picker.locator(".icon-cell")).toHaveCount(20);
  await expect(picker.getByRole("button", { name: "Default icon" })).toBeDisabled();
  await picker.getByRole("button", { name: "Pin" }).click();
  await expect(picker).toHaveCount(0);
  await expect(icon).toHaveClass(/ao-i-pin/);
  expect((await workspace(page)).windows[0].sideTabs[0].icon).toBe("pin");

  await icon.dblclick();
  await expect(picker.getByRole("button", { name: "Pin" })).toHaveAttribute("aria-pressed", "true");
  await picker.getByRole("button", { name: "Default icon" }).click();
  await expect(icon).toHaveClass(/ao-i-term/);
  expect((await workspace(page)).windows[0].sideTabs[0].icon).toBeUndefined();
});

test("a tab's icon shows in the group and the tab bar; arrows and Enter pick, Esc closes", async ({ page }) => {
  await start(page);
  const side = (await workspace(page)).windows[0].sideTabs[0];
  await mock(page, "send", { type: "CreateTab", sideTabId: side.id });
  const kid = page.locator(".ao-st-kid").first().locator(".ao-i").first();
  await kid.dblclick();
  const picker = page.getByRole("dialog", { name: /^Icon for / });
  await page.keyboard.press("Escape");
  await expect(picker).toHaveCount(0);

  await kid.dblclick();
  // Terminal is first and focused; two rows down (five icons a row) is Link.
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(kid).toHaveClass(/ao-i-link/);
  await expect(page.locator(".ao-tab").first().locator(".ao-i-link")).toHaveCount(1);
});

test("the right-click menu opens the icon list too", async ({ page }) => {
  await start(page);
  await page.locator(".ao-st").first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Change icon…" }).click();
  await page.getByRole("dialog", { name: /^Icon for / }).getByRole("button", { name: "Document" }).click();
  await expect(page.locator(".ao-st > .ao-i").first()).toHaveClass(/ao-i-doc/);
});
