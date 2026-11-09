import { expect, type Page } from "@playwright/test";

/**
 * Adds a tab to the selected row and selects it, as ⌘T did before task 8.79. ⌘T now opens a
 * row of its own beside a one-tab row, so tests that need a folder build it here.
 */
export async function addTab(page: Page) {
  const side = await page.locator(".ao-st.active").first().getAttribute("data-side");
  const before = await page.evaluate(
    (side) => (window as any).__terminescu.host.workspace().windows.flatMap((w: any) => w.sideTabs).find((s: any) => s.id === side).tabs.map((t: any) => t.id),
    side,
  );
  await page.evaluate((side) => (window as any).__terminescu.host.send({ type: "CreateTab", sideTabId: side }), side);
  const added = page.locator(`[data-kid]:not(${before.map((id: string) => `[data-kid="${id}"]`).join(",")}), .ao-tabs [data-tab]:not(${before.map((id: string) => `[data-tab="${id}"]`).join(",")})`).first();
  await expect(added).toBeVisible();
  await added.click();
}
