import { expect, test, type Page } from "@playwright/test";

// Task 8.45 on the mock host: OSC 133 marks from the shell integration; ⌘↑ and ⌘↓ jump between
// prompts, ⌘⇧O selects the last command's output, and a text pane keeps ⌘↑/⌘↓ for its editor.

const A = "\x1b]133;A;src=terminescu\x07";
const C = "\x1b]133;C;src=terminescu\x07";
const D = (code: number) => `\x1b]133;D;${code};src=terminescu\x07`;

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  let text = "";
  for (const [i, code] of [0, 2, 0].entries()) {
    text += `${A}$ make step${i}\r\n${C}` + Array.from({ length: 60 }, (_, n) => `step${i} line ${n}\r\n`).join("") + D(code);
  }
  text += `${A}$ `;
  await page.evaluate(
    ([id, t]) => new Promise<void>((done) => (window as any).__terminescu.terminals.get(id).write(t, done)),
    [pane, text],
  );
  return pane;
}

const top = (page: Page, pane: string) =>
  page.evaluate((id) => {
    const b = (window as any).__terminescu.terminals.get(id).buffer.active;
    return b.getLine(b.viewportY).translateToString(true) as string;
  }, pane);

test("⌘↑ and ⌘↓ jump between prompts, ⌘⇧O selects the output", async ({ page }) => {
  const pane = await start(page);
  await page.keyboard.press("Meta+ArrowUp");
  await expect.poll(() => top(page, pane)).toBe("$ make step2");
  await page.keyboard.press("Meta+ArrowUp");
  await expect.poll(() => top(page, pane)).toBe("$ make step1");

  await page.keyboard.press("Meta+Shift+O");
  const selection = await page.evaluate((id) => (window as any).__terminescu.terminals.get(id).getSelection() as string, pane);
  expect(selection.split("\n")[0]).toBe("step1 line 0");
  expect(selection.trimEnd().split("\n").at(-1)).toBe("step1 line 59");

  await page.keyboard.press("Meta+ArrowDown");
  await expect.poll(() => top(page, pane)).toBe("$ make step2");
  // The terminal keeps the keys.
  await expect(page.locator(`.term-pane[data-pane="${pane}"] .xterm-helper-textarea`)).toBeFocused();
});

test("⌘↓ in a text pane moves the editor's cursor", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Alt+T");
  const editor = page.locator(".text-pane .cm-content");
  await editor.click();
  await page.keyboard.type("one\ntwo\nthree");
  await page.keyboard.press("Meta+ArrowUp");
  await page.keyboard.type("0 ");
  await page.keyboard.press("Meta+ArrowDown");
  await page.keyboard.type(" 4");
  await expect(editor).toHaveText(/^0 one.*three 4$/s);
});
