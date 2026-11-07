import { expect, test } from "@playwright/test";

// Task 8.39 on the mock host: ⌘⇧M shows the focused split pane alone and brings the split
// back; a split or a focus move restores it too.

const shown = (page: import("@playwright/test").Page) => page.locator(".app-pane .ao-tp");

test("⌘⇧M maximizes the focused pane and restores the split", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+D");
  await expect(shown(page)).toHaveCount(2);
  const focused = await page.locator(".app-pane .ao-tp.focused").getAttribute("data-pane");

  await page.keyboard.press("Meta+Shift+M");
  await expect(shown(page)).toHaveCount(1);
  await expect(shown(page)).toHaveAttribute("data-pane", focused!);
  // The terminal keeps the keys and fills the pane.
  await expect(page.locator(`.term-pane[data-pane="${focused}"] .xterm-helper-textarea`)).toBeFocused();
  await expect(shown(page).getByRole("button", { name: "Restore panes", exact: true })).toBeVisible();

  await page.keyboard.press("Meta+Shift+M");
  await expect(shown(page)).toHaveCount(2);

  // The header button does the same; a focus move brings the split back.
  await page.locator(`.ao-tp[data-pane="${focused}"]`).getByRole("button", { name: "Maximize pane", exact: true }).click();
  await expect(shown(page)).toHaveCount(1);
  await page.keyboard.press("Meta+Alt+ArrowLeft");
  await expect(shown(page)).toHaveCount(2);
});
