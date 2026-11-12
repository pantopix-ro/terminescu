import { expect, test, type Page } from "@playwright/test";

// add-public-release task 4.4 on the mock host: both feedback routes carry the versions, and
// the log is revealed only with the box ticked (D7).

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });
const opened = (page: Page) => page.evaluate(() => [...(window as any).__terminescu.host.openedUrls] as string[]);

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".term-pane")).toHaveCount(1);
}

test("Send Feedback opens a prefixed email with no log by default", async ({ page }) => {
  await start(page);
  await mock(page, "menu", "feedback_email");
  const sheet = page.getByRole("dialog", { name: "Send Feedback" });
  await expect(sheet.getByRole("checkbox", { name: /Include app.log/ })).not.toBeChecked();
  await sheet.getByRole("button", { name: "Open email" }).click();
  await expect(sheet).toHaveCount(0);
  const urls = await opened(page);
  expect(urls).toHaveLength(1);
  expect(urls[0]).toMatch(/^mailto:salut@pantopix\.com\?subject=%5BTerminescu%5D%20Feedback&body=/);
  expect(decodeURIComponent(urls[0] ?? "")).toContain("macOS 15.0");
  expect(decodeURIComponent(urls[0] ?? "")).not.toContain("app.log");
  expect(urls).not.toContain("reveal:app.log");
});

test("Report an Issue opens the repo's new issue and reveals the log when ticked", async ({ page }) => {
  await start(page);
  await mock(page, "menu", "feedback_issue");
  const sheet = page.getByRole("dialog", { name: "Report an Issue" });
  await sheet.getByRole("checkbox", { name: /Include app.log/ }).check();
  await sheet.getByRole("button", { name: "Open issue" }).click();
  const urls = await opened(page);
  expect(urls[0]).toMatch(/^https:\/\/github\.com\/pantopix-ro\/terminescu\/issues\/new\?body=/);
  expect(decodeURIComponent(urls[0] ?? "")).toContain("Terminescu 0.1.0 (dev)");
  expect(urls).toContain("reveal:app.log");
});
