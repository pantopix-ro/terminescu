import { expect, test } from "@playwright/test";

// A modal dialog keeps the keyboard: Tab cycles inside it, focus taken elsewhere comes back,
// and closing it returns focus to the terminal it was opened from.

test("Tab stays inside Settings, and Escape gives the terminal its focus back", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Comma");
  const dialog = page.getByRole("dialog");
  await expect(dialog).toHaveAttribute("aria-modal", "true");

  const inside = () => page.evaluate(() => !!document.activeElement?.closest('[role="dialog"]'));
  for (let i = 0; i < 60; i++) {
    await page.keyboard.press(i % 7 === 0 ? "Shift+Tab" : "Tab");
    expect(await inside()).toBe(true);
  }
  // A script focusing the terminal behind is pulled back into the dialog.
  await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane .xterm-helper-textarea")!.focus());
  expect(await inside()).toBe(true);

  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => !!document.activeElement?.closest(".term-pane"))).toBe(true);
});

test("an alert dialog wraps from its last button to its first", async ({ page }) => {
  await page.goto("/");
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  await page.locator(".term-pane").first().click();
  await page.evaluate((id) => (window as any).__terminescu.host.setAgent(id, { session: "sess-1", state: "running", lastMessage: null }), pane);
  await page.keyboard.press("Meta+W");
  const ask = page.getByRole("alertdialog", { name: "Close a running program" });
  await expect(ask.getByRole("button", { name: "Close", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(ask.getByRole("button", { name: "Cancel" })).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(ask.getByRole("button", { name: "Close", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(ask).toHaveCount(0);
  await expect(page.locator(".term-pane")).toHaveCount(1);
});
