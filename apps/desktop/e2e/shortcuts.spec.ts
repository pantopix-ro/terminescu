import { expect, test, type Page } from "@playwright/test";

// The macOS terminal and editor shortcuts people expect, on the mock host: ⌘K clears, ⌘P opens
// the palette, ⌘A selects, and the editing chords reach the shell as Terminal.app sends them.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  return page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
}
const inputs = (page: Page, pane: string) =>
  page.evaluate((p) => (window as any).__terminescu.host.inputs.get(p) ?? "", pane) as Promise<string>;

test("⌥←, ⌥→, ⌘←, ⌘→ and ⌘⌫ send the shell's word and line keys", async ({ page }) => {
  const pane = await start(page);
  const before = (await inputs(page, pane)).length;
  for (const k of ["Alt+ArrowLeft", "Alt+ArrowRight", "Meta+ArrowLeft", "Meta+ArrowRight", "Meta+Backspace"]) await page.keyboard.press(k);
  await expect.poll(async () => (await inputs(page, pane)).slice(before)).toBe("\x1bb\x1bf\x01\x05\x15");
});

test("⌘A selects the terminal's whole buffer, ⌘K clears it and ⌘P opens the palette", async ({ page }) => {
  const pane = await start(page);
  await page.evaluate(
    (id) => new Promise<void>((done) => (window as any).__terminescu.terminals.get(id).write("first line\r\nsecond line\r\n", done)),
    pane,
  );
  await page.keyboard.press("Meta+A");
  const selection = () => page.evaluate((id) => (window as any).__terminescu.terminals.get(id).getSelection() as string, pane);
  await expect.poll(selection).toContain("first line");
  expect(await selection()).toContain("second line");
  await page.keyboard.press("Meta+K");
  await expect.poll(() => page.evaluate((id) => (window as any).__terminescu.host.clears.length as number, pane)).toBeGreaterThan(0);
  await page.keyboard.press("Meta+P");
  await expect(page.locator(".palette")).toBeVisible();
  // ⌘A in the palette's field selects its text, not the terminal's.
  await page.keyboard.type("split");
  await page.keyboard.press("Meta+A");
  expect(await page.evaluate(() => {
    const el = document.activeElement as HTMLInputElement;
    return [el.selectionStart, el.selectionEnd, el.value.length];
  })).toEqual([0, 5, 5]);
});

test("in a text pane ⌘A selects the document and ⌘⇧K deletes a line", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Alt+T");
  const editor = page.locator(".text-pane .cm-content");
  await expect(editor).toBeVisible();
  await editor.click();
  await page.keyboard.type("one\ntwo\nthree");
  await page.keyboard.press("Meta+A");
  await expect.poll(() => page.evaluate(() => window.getSelection()?.toString() ?? "")).toContain("one");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Meta+Shift+K");
  await expect(editor).toHaveText(/onetwo/);
});
