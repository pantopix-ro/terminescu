import { expect, test, type Page } from "@playwright/test";

// add-public-release task 5.1 on the mock host: confirm, remove the helpers, then quit (D9).

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".term-pane")).toHaveCount(1);
}

test("Cancel removes nothing", async ({ page }) => {
  await start(page);
  await mock(page, "menu", "uninstall");
  await page.getByRole("dialog", { name: "Uninstall Terminescu Helpers" }).getByRole("button", { name: "Cancel" }).click();
  expect(await page.evaluate(() => (window as any).__terminescu.host.uninstalls)).toEqual([]);
  expect(await mock(page, "claudeHook")).toBe(true);
});

test("Uninstall keeps the data unless asked, then says how to finish and quits", async ({ page }) => {
  await start(page);
  await mock(page, "menu", "uninstall");
  const sheet = page.getByRole("dialog", { name: "Uninstall Terminescu Helpers" });
  await expect(sheet.getByRole("checkbox", { name: /Also delete my Terminescu data/ })).not.toBeChecked();
  await sheet.getByRole("button", { name: "Uninstall" }).click();
  const done = page.getByRole("dialog", { name: "Helpers removed" });
  await expect(done).toContainText("Drag Terminescu from Applications to the Trash");
  expect(await page.evaluate(() => (window as any).__terminescu.host.uninstalls)).toEqual([false]);
  expect(await mock(page, "claudeHook")).toBe(false);
  await done.getByRole("button", { name: "Quit" }).click();
  await expect.poll(() => mock(page, "quits")).toBe(1);
});
