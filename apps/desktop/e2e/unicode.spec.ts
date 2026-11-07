import { expect, test } from "@playwright/test";

// Task 8.41: terminals measure characters with the Unicode 11 table, as the daemon's grid does.

test("a terminal uses Unicode 11 widths", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".term-pane").first()).toBeVisible();
  const version = await page.evaluate(() => {
    const term = [...(window as any).__terminescu.terminals.values()][0];
    return term.unicode.activeVersion as string;
  });
  expect(version).toBe("11");
});
