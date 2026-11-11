import { expect, test, type Page } from "@playwright/test";

// add-public-release task 3.3 on the mock host: `?first-run` starts it as a fresh Mac, with no
// hook in Claude Code's settings and the first-run screen not yet shown (D5, D6).

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

async function start(page: Page, url: string) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto(url);
  await expect(page.locator(".term-pane")).toHaveCount(1);
}

test("the first run asks once; Not now adds no hook", async ({ page }) => {
  await start(page, "/?first-run");
  const sheet = page.getByRole("dialog", { name: "Welcome to Terminescu" });
  await expect(sheet).toContainText("shell daemon");
  await expect(sheet.getByRole("checkbox", { name: /Claude Code hook/ })).toBeChecked();
  await sheet.getByRole("button", { name: "Privacy" }).click();
  await expect(page.getByRole("dialog", { name: "Privacy" })).toContainText("update check");
  await page.keyboard.press("Escape");
  await sheet.getByRole("button", { name: "Not now" }).click();
  await expect(sheet).toHaveCount(0);
  expect(await mock(page, "claudeHook")).toBe(false);
  await expect.poll(() => mock(page, "settings")).toMatchObject({ hook: "off", firstRunDone: true });
  // Later settings changes do not bring it back.
  const now = await mock(page, "settings");
  await mock(page, "setSettings", { ...now, zoomPercent: 110 });
  await expect.poll(() => mock(page, "settings")).toMatchObject({ zoomPercent: 110 });
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("the hook tick survives a look at Privacy, and a click outside keeps the sheet", async ({ page }) => {
  await start(page, "/?first-run");
  const sheet = page.getByRole("dialog", { name: "Welcome to Terminescu" });
  const tick = sheet.getByRole("checkbox", { name: /Claude Code hook/ });
  await tick.uncheck();
  await sheet.getByRole("button", { name: "Privacy" }).click();
  await page.getByRole("dialog", { name: "Privacy" }).press("Escape");
  await expect(tick).not.toBeChecked();
  await page.mouse.click(5, 790);
  await expect(sheet).toBeVisible();
  await sheet.getByRole("button", { name: "Continue" }).click();
  await expect(sheet).toHaveCount(0);
  await expect.poll(() => mock(page, "settings")).toMatchObject({ hook: "off", firstRunDone: true });
  expect(await mock(page, "claudeHook")).toBe(false);
});

test("Continue adds the hook, and Settings › Shell removes and adds it", async ({ page }) => {
  await start(page, "/?first-run");
  await page.getByRole("dialog", { name: "Welcome to Terminescu" }).getByRole("button", { name: "Continue" }).click();
  await expect.poll(() => mock(page, "claudeHook")).toBe(true);
  await page.keyboard.press("Meta+Comma");
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: "Shell" }).click();
  const toggle = dialog.getByRole("checkbox", { name: /Claude Code hook/ });
  await expect(toggle).toBeChecked();
  await toggle.uncheck();
  await expect.poll(() => mock(page, "claudeHook")).toBe(false);
  await toggle.check();
  await expect.poll(() => mock(page, "claudeHook")).toBe(true);
});

test("an install with settings already stored shows no first run", async ({ page }) => {
  await start(page, "/");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await mock(page, "claudeHook")).toBe(true);
});

test("with the hook off, a Claude pane says so and turns it on", async ({ page }) => {
  await start(page, "/?first-run");
  await page.getByRole("dialog", { name: "Welcome to Terminescu" }).getByRole("button", { name: "Not now" }).click();
  const pane = await page.locator(".ao-tp[data-pane]").first().getAttribute("data-pane");
  await mock(page, "runCommand", pane, "claude", null);
  const bar = page.locator(".hook-off-bar");
  await expect(bar).toContainText("Claude hook is off");
  await bar.getByRole("button", { name: "Turn on" }).click();
  await expect.poll(() => mock(page, "claudeHook")).toBe(true);
  await expect(bar).toHaveCount(0);
});
