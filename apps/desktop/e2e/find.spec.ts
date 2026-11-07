import { expect, test, type Page } from "@playwright/test";

// Task 8.38 on the mock host: ⌘F opens a find bar over the focused terminal, ⌘G and ⇧↵ step
// through the matches, Esc closes it; in a text pane ⌘F stays the editor's own search.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  // Output straight into xterm.js, so the matches do not depend on the mock shell's echo.
  await page.evaluate(
    (id) =>
      new Promise<void>((done) =>
        (window as any).__terminescu.terminals.get(id).write("deploy acme/api\r\nDeploy web\r\nredeploy acme/api\r\n", done),
      ),
    pane,
  );
  return pane;
}

const selection = (page: Page, pane: string) =>
  page.evaluate((id) => (window as any).__terminescu.terminals.get(id).getSelection() as string, pane);

test("⌘F finds in the terminal, ⌘G steps, Esc closes and gives the keys back", async ({ page }) => {
  const pane = await start(page);
  await page.keyboard.press("Meta+F");
  const bar = page.getByRole("search");
  await expect(bar).toBeVisible();
  const input = bar.getByLabel("Find in terminal");
  await expect(input).toBeFocused();
  await input.fill("deploy");
  await expect(bar.locator(".find-count")).toHaveText(/of 3$/);
  await expect.poll(() => selection(page, pane)).toMatch(/^deploy$/i);

  await bar.getByRole("button", { name: "Match case" }).click();
  await expect(bar.locator(".find-count")).toHaveText(/of 2$/);
  await bar.getByRole("button", { name: "Whole word" }).click();
  await expect(bar.locator(".find-count")).toHaveText("1 of 1");

  await bar.getByRole("button", { name: "Whole word" }).click();
  await input.press("Enter");
  const before = await bar.locator(".find-count").textContent();
  await page.keyboard.press("Meta+G");
  await expect(bar.locator(".find-count")).not.toHaveText(before!);
  await page.keyboard.press("Meta+Shift+G");
  await expect(bar.locator(".find-count")).toHaveText(before!);

  await input.fill("zzz");
  await expect(bar.locator(".find-count")).toHaveText("No matches");
  await bar.getByRole("button", { name: "Regular expression" }).click();
  await input.fill("(");
  await expect(bar.locator(".find-count")).toHaveText("Bad pattern");

  await input.press("Escape");
  await expect(bar).toHaveCount(0);
  await expect(page.locator(".term-pane .xterm-helper-textarea").first()).toBeFocused();
});

test("⌘F in a text pane opens the editor's search, not the terminal's", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Alt+T");
  const editor = page.locator(".text-pane .cm-content");
  await expect(editor).toBeVisible();
  await editor.click();
  await page.keyboard.press("Meta+F");
  await expect(page.locator(".text-pane .cm-search")).toBeVisible();
  await expect(page.getByRole("search")).toHaveCount(0);
});
