import { expect, test, type Page } from "@playwright/test";

// Task 8.54 on the mock host: a tab's icon blinks in the tab bar and the sidebar while a command
// or Claude works, and blinks and bounces while Claude waits for an answer.

const host = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });
const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

test("icons blink while a command runs and bounce while Claude waits", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  const side = (await workspace(page)).windows[0].sideTabs[0];
  const tab = side.tabs[0];
  const pane = tab.layout.id;
  const tabIcon = page.locator(`.ao-tab[data-tab="${tab.id}"] .ao-i`).first();
  const rowIcon = page.locator(`.ao-st[data-side="${side.id}"] > .ao-i`);

  await expect(tabIcon).not.toHaveClass(/act-/);
  await host(page, "runCommand", pane, "mvn verify", null);
  await expect(tabIcon).toHaveClass(/act-busy/);
  await expect(rowIcon).toHaveClass(/act-busy/);
  await expect(tabIcon).toHaveCSS("animation-name", "act-blink");

  // The command ends: the icons stop.
  await host(page, "runCommand", pane, "mvn verify", 0);
  await expect(tabIcon).not.toHaveClass(/act-/);
  await expect(rowIcon).not.toHaveClass(/act-/);

  // Claude at its prompt is still; working blinks; a question blinks and bounces.
  await host(page, "runCommand", pane, "claude", null);
  await expect(tabIcon).not.toHaveClass(/act-/);
  await host(page, "setAgent", pane, { session: "s-1", state: "idle", lastMessage: null });
  await expect(tabIcon).not.toHaveClass(/act-/);
  await host(page, "setAgent", pane, { session: "s-1", state: "running", lastMessage: null });
  await expect(tabIcon).toHaveClass(/act-busy/);
  await host(page, "setAgent", pane, { session: "s-1", state: "waiting", lastMessage: null });
  await expect(tabIcon).toHaveClass(/act-wait/);
  await expect(rowIcon).toHaveClass(/act-wait/);
  // Claude's ✳ comes before the tab name while it waits, and goes when it works again.
  const ask = page.locator(`.ao-tab[data-tab="${tab.id}"] .ask-mark`);
  await expect(ask).toHaveText("✳");
  await expect(page.locator(`.ao-tab[data-tab="${tab.id}"] .n`)).not.toContainText("✳");
  await expect(tabIcon).toHaveCSS("animation-name", "act-blink, act-bounce");
  await host(page, "setAgent", pane, { session: "s-1", state: "running", lastMessage: null });
  await expect(ask).toHaveCount(0);
});

test("reduced motion keeps the icons still", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  const tab = (await workspace(page)).windows[0].sideTabs[0].tabs[0];
  await host(page, "runCommand", tab.layout.id, "npm test", null);
  const icon = page.locator(`.ao-tab[data-tab="${tab.id}"] .ao-i`).first();
  await expect(icon).toHaveClass(/act-busy/);
  await expect(icon).toHaveCSS("animation-name", "none");
});

test("a group row keeps its icon still while its shells blink", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await page.getByRole("button", { name: "New tab", exact: true }).click();
  await expect(page.locator(".ao-st-kid")).toHaveCount(2);
  const side = (await workspace(page)).windows[0].sideTabs[0];
  const tab = side.tabs[0];
  await host(page, "runCommand", tab.layout.id, "npm run dev", null);
  await expect(page.locator(`.ao-st-kid[data-kid="${tab.id}"] .ao-i`).first()).toHaveClass(/act-busy/);
  await expect(page.locator(`.ao-tab[data-tab="${tab.id}"] .ao-i`).first()).toHaveClass(/act-busy/);
  await expect(page.locator(`.ao-st[data-side="${side.id}"] > .ao-i`)).not.toHaveClass(/act-/);
});
