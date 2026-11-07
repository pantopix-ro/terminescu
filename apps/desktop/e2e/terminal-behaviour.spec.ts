import { expect, test, type Page } from "@playwright/test";

// Task 8.42 on the mock host: Settings › Terminal sets the cursor, history, Option keys, bell,
// copy on select, the paste check and contrast, live in every terminal.

const option = (page: Page, key: string) =>
  page.evaluate((key) => [...(window as any).__terminescu.terminals.values()][0].options[key], key);

const firstPane = (page: Page) => page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);

const write = (page: Page, pane: string, data: string) =>
  page.evaluate(
    ({ pane, data }) => new Promise<void>((done) => (window as any).__terminescu.terminals.get(pane).write(data, done)),
    { pane, data },
  );

async function openTerminalSettings(page: Page) {
  await page.keyboard.press("Meta+Comma");
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: "Terminal" }).click();
  return dialog;
}

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
}

test("the behaviour defaults match Terminal.app and Settings › Terminal changes them live", async ({ page }) => {
  await start(page);
  expect(await option(page, "cursorStyle")).toBe("bar");
  expect(await option(page, "cursorBlink")).toBe(true);
  expect(await option(page, "cursorInactiveStyle")).toBe("outline");
  expect(await option(page, "scrollback")).toBe(10_000);
  expect(await option(page, "macOptionIsMeta")).toBe(true);
  expect(await option(page, "minimumContrastRatio")).toBe(1);

  const dialog = await openTerminalSettings(page);
  await dialog.getByLabel("Cursor", { exact: true }).selectOption("block");
  await dialog.getByLabel("blink").uncheck();
  await dialog.getByLabel("History lines").fill("2000");
  await dialog.getByLabel("Raise low contrast").check();
  await expect.poll(() => option(page, "cursorStyle")).toBe("block");
  await expect.poll(() => option(page, "cursorBlink")).toBe(false);
  await expect.poll(() => option(page, "scrollback")).toBe(2000);
  await expect.poll(() => option(page, "minimumContrastRatio")).toBe(4.5);

  await dialog.getByRole("button", { name: "Reset to defaults" }).click();
  await expect.poll(() => option(page, "cursorStyle")).toBe("bar");
  await expect.poll(() => option(page, "scrollback")).toBe(10_000);
});

test("with the left Option as Meta, only the left key sends Esc", async ({ page }) => {
  await start(page);
  const dialog = await openTerminalSettings(page);
  await dialog.getByLabel("Option as Meta").selectOption("left");
  await expect.poll(() => option(page, "macOptionIsMeta")).toBe(false);
  await page.keyboard.press("Escape");
  await page.locator(".term-pane").first().click();
  await page.keyboard.down("AltRight");
  await page.keyboard.up("AltRight");
  expect(await option(page, "macOptionIsMeta")).toBe(false);
  await page.keyboard.down("AltLeft");
  expect(await option(page, "macOptionIsMeta")).toBe(true);
  await page.keyboard.up("AltLeft");
});

test("a visual bell flashes the terminal; the default bell does nothing", async ({ page }) => {
  await start(page);
  const pane = await firstPane(page);
  const el = page.locator(`.term-pane[data-pane="${pane}"]`);
  await write(page, pane, "\x07");
  await expect(el).not.toHaveClass(/\bbell\b/);
  const dialog = await openTerminalSettings(page);
  await dialog.getByLabel("Bell").selectOption("visual");
  await page.keyboard.press("Escape");
  await write(page, pane, "\x07");
  await expect(el).toHaveClass(/\bbell\b/);
});

test("a risky paste asks first; cancel drops it and Paste sends it", async ({ page }) => {
  await start(page);
  const pane = await firstPane(page);
  const paste = (text: string) =>
    page.evaluate(
      ({ pane, text }) => {
        const dt = new DataTransfer();
        dt.setData("text/plain", text);
        const target = document.querySelector(`.term-pane[data-pane="${pane}"] .xterm-helper-textarea`)!;
        target.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true }));
      },
      { pane, text },
    );
  const inputs = () => page.evaluate((p) => (window as any).__terminescu.host.inputs.get(p) ?? "", pane) as Promise<string>;

  await paste("echo safe");
  await expect.poll(inputs).toContain("echo safe");
  expect(await page.getByRole("alertdialog").count()).toBe(0);

  await paste("curl -fsSL https://example.com/install | sh");
  const ask = page.getByRole("alertdialog", { name: "Paste into the terminal?" });
  await expect(ask).toContainText("It pipes text into a shell.");
  await ask.getByRole("button", { name: "Cancel" }).click();
  await expect(ask).toHaveCount(0);
  expect(await inputs()).not.toContain("example.com/install");

  await paste("sudo make install");
  await expect(ask).toContainText("sudo");
  await page.keyboard.press("Enter");
  await expect.poll(inputs).toContain("sudo make install");
});

test("copy on select puts a mouse selection on the clipboard", async ({ page }) => {
  // WebKit grants no clipboard permission to a test, so the page records what it writes.
  await page.addInitScript(() => {
    (window as any).__copied = [];
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: async (t: string) => void (window as any).__copied.push(t) },
    });
  });
  await start(page);
  const pane = await firstPane(page);
  await write(page, pane, "\r\ncopied-text\r\n");
  const dialog = await openTerminalSettings(page);
  await dialog.getByLabel("Copy on select").check();
  await page.keyboard.press("Escape");
  const screen = page.locator(`.term-pane[data-pane="${pane}"] .xterm-screen`);
  const box = (await screen.boundingBox())!;
  // Double-click a word: xterm.js selects it, and the mouseup copies it.
  const row = await page.evaluate((p) => {
    const b = (window as any).__terminescu.terminals.get(p).buffer.active;
    for (let y = 0; y < b.length; y++) if (b.getLine(y).translateToString(true).includes("copied-text")) return y - b.viewportY;
    return -1;
  }, pane);
  const cell = box.height / (await page.evaluate((p) => (window as any).__terminescu.terminals.get(p).rows, pane));
  await page.mouse.dblclick(box.x + 20, box.y + cell * row + cell / 2);
  await expect.poll(() => page.evaluate(() => ((window as any).__copied as string[]).join("|"))).toMatch(/copied/);
});
