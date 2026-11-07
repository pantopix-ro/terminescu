import { expect, test, type Page } from "@playwright/test";

// Task 8.36 on the mock host: Settings › Terminal sets every terminal's font and spacing and the
// tab text size, live; a split pane's header shows its whole name on hover.

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

const option = (page: Page, key: string) =>
  page.evaluate(
    (key) => {
      const terms = [...(window as any).__terminescu.terminals.values()];
      return terms[0].options[key];
    },
    key,
  );

async function openTerminalSettings(page: Page) {
  await page.keyboard.press("Meta+Comma");
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: "Terminal" }).click();
  await expect(dialog).toContainText("Settings › Terminal");
  return dialog;
}

test("font, size, line height, weight and tab text apply live and reset", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  // The defaults match Terminal.app's Basic profile.
  expect(await option(page, "lineHeight")).toBe(1);
  expect(await option(page, "fontSize")).toBe(11);
  expect(await option(page, "cursorStyle")).toBe("bar");
  expect(await option(page, "cursorBlink")).toBe(true);
  expect(await option(page, "drawBoldTextInBrightColors")).toBe(false);
  const rows = await page.evaluate(() => [...(window as any).__terminescu.terminals.values()][0].rows as number);

  const dialog = await openTerminalSettings(page);
  await dialog.getByRole("slider", { name: "Line height" }).fill("150");
  await expect.poll(() => option(page, "lineHeight")).toBe(1.5);
  await expect(dialog.locator(".set-out").first()).toHaveText("1.50");
  // Looser lines fit fewer rows, and the shell hears the new size.
  await expect.poll(() => page.evaluate(() => [...(window as any).__terminescu.terminals.values()][0].rows as number)).toBeLessThan(rows);

  await dialog.getByRole("spinbutton", { name: "Font size" }).fill("14");
  await expect.poll(() => option(page, "fontSize")).toBe(14);
  // A value out of range is not saved and comes back on blur.
  await dialog.getByRole("spinbutton", { name: "Font size" }).fill("40");
  await dialog.getByRole("spinbutton", { name: "Font size" }).blur();
  await expect(dialog.getByRole("spinbutton", { name: "Font size" })).toHaveValue("14");

  const font = dialog.getByRole("combobox", { name: "Terminal font" });
  await font.fill("Menlo");
  await font.press("Enter");
  await expect.poll(() => option(page, "fontFamily")).toMatch(/^"Menlo", ui-monospace/);
  await font.fill('Menlo"; x');
  await font.blur();
  await expect(font).toHaveValue("Menlo");

  await dialog.getByRole("combobox", { name: "Font weight" }).selectOption("500");
  await expect.poll(() => option(page, "fontWeight")).toBe(500);
  await expect.poll(() => option(page, "fontWeightBold")).toBe(800);

  await dialog.getByRole("spinbutton", { name: "Tab text size" }).fill("10");
  const tab = page.locator(".app-main .ao-tab[data-tab] .n").first();
  await expect.poll(() => tab.evaluate((el) => getComputedStyle(el).fontSize)).toBe("10px");

  expect(await mock(page, "settings")).toMatchObject({
    terminalFont: "Menlo",
    terminalFontSize: 14,
    terminalLineHeight: 150,
    terminalFontWeight: 500,
    tabFontSize: 10,
  });

  await dialog.getByRole("button", { name: "Reset to defaults" }).click();
  await expect.poll(() => option(page, "lineHeight")).toBe(1);
  await expect.poll(() => option(page, "fontSize")).toBe(11);
  await expect.poll(() => tab.evaluate((el) => getComputedStyle(el).fontSize)).toBe("12px");
});

test("a split pane's header names the pane in its tooltip", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+d");
  const heads = page.locator(".ao-tp-head");
  await expect(heads).toHaveCount(2);
  const name = (await heads.first().locator(".tp-name").textContent()) ?? "";
  expect(name.length).toBeGreaterThan(0);
  await expect(heads.first()).toHaveAttribute("title", `${name}\nDrag to the tab bar to make it a tab`);
});

test("Option+Enter sends Esc+Return, a newline in Claude Code", async ({ page }) => {
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  const pane = await page.evaluate(() => [...(window as any).__terminescu.terminals.keys()][0] as string);
  const inputs = () => page.evaluate((id) => (window as any).__terminescu.host.inputs.get(id) ?? "", pane);
  await page.keyboard.press("Alt+Enter");
  await expect.poll(inputs).toContain("\x1b\r");
  await page.keyboard.press("Enter");
  await expect.poll(inputs).toMatch(/\x1b\r\r$/);
});
