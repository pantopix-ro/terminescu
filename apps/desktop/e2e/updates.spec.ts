import { expect, test, type Page } from "@playwright/test";

// add-public-release task 6.1 on the mock host: Check for Updates and the restart prompt (D4).

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".term-pane")).toHaveCount(1);
}

test("Check for Updates says up to date when the feed has nothing newer", async ({ page }) => {
  await start(page);
  await mock(page, "menu", "check_updates");
  await expect(page.locator(".app-note")).toHaveText("Terminescu is up to date.");
  await expect(page.locator(".update-bar")).toHaveCount(0);
});

test("a found update waits for Restart to update; Later hides it", async ({ page }) => {
  await start(page);
  await mock(page, "nextUpdate", "0.2.1");
  await mock(page, "menu", "check_updates");
  const bar = page.locator(".update-bar");
  await expect(bar).toContainText("Terminescu 0.2.1 is ready");
  expect(await mock(page, "updateRestarts")).toBe(0);
  await bar.getByRole("button", { name: "Later" }).click();
  await expect(bar).toHaveCount(0);
  // The daily check finds it again, and this time the person restarts.
  await mock(page, "offerUpdate", "0.2.1");
  await bar.getByRole("button", { name: "Restart to update" }).click();
  await expect.poll(() => mock(page, "updateRestarts")).toBe(1);
});
