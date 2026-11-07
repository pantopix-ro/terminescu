import { expect, test, type Page } from "@playwright/test";

// Task 8.46 on the mock host: ⌘⇧F finds in every terminal of the window, history included, and
// a hit shows its tab, focuses its pane and selects the line.

const focusedPane = (page: Page) =>
  page.evaluate(() => document.activeElement?.closest<HTMLElement>(".term-pane")?.dataset.pane ?? null);
const write = (page: Page, pane: string, text: string) =>
  page.evaluate(
    ([id, t]) => new Promise<void>((done) => (window as any).__terminescu.terminals.get(id).write(t, done)),
    [pane, text],
  );

test("⌘⇧F finds in all panes and a hit shows the line in its tab", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  await expect.poll(() => focusedPane(page)).not.toBeNull();
  const first = await focusedPane(page);
  await write(page, first!, "error: deploy acme/api failed\r\n" + Array.from({ length: 100 }, (_, i) => `line ${i}\r\n`).join(""));
  await page.keyboard.press("Meta+T");
  await expect(page.getByRole("tab")).toHaveCount(2);
  await expect.poll(() => focusedPane(page)).not.toBe(first);
  const second = (await focusedPane(page))!;
  await write(page, second, "deploy web ok\r\n");

  await page.keyboard.press("Meta+Shift+F");
  const input = page.getByLabel("Find in all panes");
  await expect(input).toBeFocused();
  await input.fill("DEPLOY");
  const items = page.locator(".find-all [cmdk-item]");
  await expect(items).toHaveCount(2);
  await expect(page.locator(".find-all [cmdk-group-heading]")).toHaveCount(2);

  await items.filter({ hasText: "acme/api" }).click();
  await expect(page.locator(".find-all")).toHaveCount(0);
  await expect(page.getByRole("tab").first()).toHaveAttribute("aria-selected", "true");
  await expect
    .poll(() => page.evaluate((id) => (window as any).__terminescu.terminals.get(id).getSelection() as string, first!))
    .toMatch(/error: deploy acme\/api failed\s*$/);

  await page.keyboard.press("Meta+Shift+F");
  await page.getByLabel("Find in all panes").fill("nothing like this");
  await expect(page.locator(".find-all")).toContainText("No matches");
  await page.keyboard.press("Escape");
  await expect(page.locator(".find-all")).toHaveCount(0);
});
