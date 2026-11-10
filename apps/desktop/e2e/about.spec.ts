import { expect, test, type Page } from "@playwright/test";

// add-public-release tasks 4.2 and 4.3 on the mock host: About, Licenses and Privacy (D7, D8).

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".term-pane")).toHaveCount(1);
}

test("About shows the version, the maker and the not-affiliated line", async ({ page }) => {
  await start(page);
  await mock(page, "menu", "about");
  const about = page.getByRole("dialog", { name: "About Terminescu" });
  await expect(about).toContainText("Version 0.1.0 (dev)");
  await expect(about).toContainText("FSL-1.1-ALv2");
  await expect(about).toContainText("Works with Claude Code. Not affiliated with Anthropic.");
  await about.getByRole("button", { name: "PANTOPIX RO" }).click();
  expect(await page.evaluate(() => (window as any).__terminescu.host.openedUrls)).toContain("https://pantopix.ro");
  await about.getByRole("button", { name: "Licenses" }).click();
  const licenses = page.getByRole("dialog", { name: "Licenses" });
  await expect(licenses).toContainText("@xterm/xterm");
  await licenses.getByRole("textbox", { name: "Filter licenses" }).fill("codemirror");
  await expect(licenses).toContainText("@codemirror/");
  await expect(licenses).not.toContainText("@xterm/xterm");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("Help › Privacy names the update check and the baselines", async ({ page }) => {
  await start(page);
  await mock(page, "menu", "privacy");
  const privacy = page.getByRole("dialog", { name: "Privacy" });
  await expect(privacy).toContainText("no telemetry");
  await expect(privacy).toContainText("update check");
  await expect(privacy).toContainText("never written to disk");
  await expect(privacy).toContainText("never touch the repository");
});
