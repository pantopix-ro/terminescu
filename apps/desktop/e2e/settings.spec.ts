import { expect, test, type Page } from "@playwright/test";

// Task 6.9 on the mock host: the Settings screen holds Statuses, Commands, Variables,
// Extraction (read-only), Shell, Keys and Notifications (D25).

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await expect(page.locator(".term-pane")).toHaveCount(1);
}

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

async function openSettings(page: Page, section: string) {
  await page.keyboard.press("Meta+Comma");
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Settings ›");
  await dialog.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: section }).click();
  await expect(dialog).toContainText(`Settings › ${section}`);
  return dialog;
}

test("every section opens; extraction is read-only and the editor setting saves", async ({ page }) => {
  await start(page);
  const dialog = await openSettings(page, "Extraction");
  await expect(dialog.locator("[data-rule-name]")).toHaveCount(4);
  await expect(dialog.locator("[data-rule-name] input")).toHaveCount(0);
  await dialog.getByRole("button", { name: "Shell" }).click();
  await dialog.getByRole("checkbox", { name: /VISUAL/ }).check();
  await expect.poll(() => mock(page, "settings")).toMatchObject({ varMode: "saved", varTtlSecs: null, editor: true, zoomPercent: 100, sidebarWidth: 272, sidebarCollapsed: false });
  await dialog.getByRole("button", { name: "Keys" }).click();
  await expect(dialog.locator('[data-action="new_tab"] kbd')).toHaveText("⌘T");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
});

test("a command edited in Settings changes in the sidebar, and a global one cannot launch", async ({ page }) => {
  await start(page);
  const side = await page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].sideTabs[0].id as string);
  await mock(page, "createCommand", { sideTabId: side, name: "checks", body: "gh pr checks", target: "newTab", launcher: false });
  await mock(page, "createCommand", { sideTabId: null, name: "status", body: "git status", target: "focused", launcher: false });
  const dialog = await openSettings(page, "Commands");
  const row = dialog.locator('.cmd-edit[data-command="checks"]');
  // A side tab is named after its first tab's animal (task 8.30).
  const first = await page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].sideTabs[0].tabs[0].name);
  await expect(row.locator(".cmd-scope")).toHaveText(first);
  await expect(dialog.locator('.cmd-edit[data-command="status"]').getByLabel("launcher")).toBeDisabled();
  await row.getByLabel("checks command").fill("gh pr checks --watch");
  await row.getByRole("button", { name: "Save" }).click();
  await expect(row.getByRole("button", { name: "Save" })).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(page.locator('.cmd-section .cmd-row[data-command="checks"] .cmd-body')).toHaveText("gh pr checks --watch");
});

test("the default storage mode and TTL carry into Extract", async ({ page }) => {
  await start(page);
  const dialog = await openSettings(page, "Variables");
  await dialog.getByRole("radio", { name: "Session only" }).check();
  await dialog.getByLabel("Default TTL").fill("1h");
  await dialog.getByLabel("Default TTL").press("Enter");
  await expect.poll(() => mock(page, "settings")).toMatchObject({ varMode: "session", varTtlSecs: 3600, editor: false, zoomPercent: 100, sidebarWidth: 272, sidebarCollapsed: false });
  // An out-of-range TTL is refused and the field shows the saved one again.
  await dialog.getByLabel("Default TTL").fill("90d");
  await dialog.getByLabel("Default TTL").press("Enter");
  await expect(dialog.getByLabel("Default TTL")).toHaveValue("1h");
  await page.keyboard.press("Escape");

  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  await page.locator(".term-pane").first().click();
  await page.evaluate((id) => (window as any).__terminescu.terminals.get(id).selectLines(0, 0), pane);
  await page.keyboard.press("Meta+E");
  const extract = page.getByRole("dialog", { name: "Extract to variable" });
  await expect(extract.getByRole("radio", { name: "Session only" })).toHaveAttribute("aria-checked", "true");
  await expect(extract.getByLabel("Time to live")).toHaveValue("1h");
});

test("notification and chime toggles save the attention settings", async ({ page }) => {
  await start(page);
  const dialog = await openSettings(page, "Notifications");
  await expect(dialog.getByRole("checkbox", { name: "macOS notification" }).first()).toBeChecked();
  await dialog.getByRole("checkbox", { name: "Chime" }).check();
  await expect.poll(() => mock(page, "attentionSettings")).toEqual({ notify: true, chime: true, commandNotify: true, commandSecs: 30 });
  // Task 8.40: the long command notification and its threshold.
  await dialog.getByLabel("Long command seconds").fill("90");
  await dialog.getByRole("checkbox", { name: "macOS notification" }).nth(1).uncheck();
  await expect.poll(() => mock(page, "attentionSettings")).toEqual({ notify: true, chime: true, commandNotify: false, commandSecs: 90 });
});

test("Keys: rebind New Side Tab to ⌘J, refuse ⌘T as a clash, reset", async ({ page }) => {
  await start(page);
  const dialog = await openSettings(page, "Keys");
  const chord = dialog.getByRole("button", { name: "Shortcut for New Side Tab" });
  await chord.click();
  await expect(chord).toHaveText("press a chord");
  // While recording, ⌘T is the chord being tried, not a new tab.
  await page.keyboard.press("Meta+T");
  await expect(dialog.getByRole("alert")).toHaveText("⌘T is already New Tab.");
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(1);
  await page.keyboard.press("Meta+J");
  await expect(chord).toHaveText("⌘J");
  await expect.poll(() => mock(page, "keyBindings")).toEqual({ new_side_tab: "Cmd+J" });
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);

  const sides = page.locator(".ao-st");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+N");
  await expect(sides).toHaveCount(1);
  await page.keyboard.press("Meta+J");
  await expect(sides).toHaveCount(2);

  const again = await openSettings(page, "Keys");
  await again.getByRole("button", { name: "Reset New Side Tab" }).click();
  await expect(again.getByRole("button", { name: "Shortcut for New Side Tab" })).toHaveText("⌘N");
  await expect.poll(() => mock(page, "keyBindings")).toEqual({});
});

// Task 8.53: the terminal's editing chords are editable; ⇧↵ as the new line chord sends Esc CR.
test("Keys: set ⇧↵ as the terminal's new line, refuse ⌘P, turn ⌘⌫ off", async ({ page }) => {
  await start(page);
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  const inputs = () => page.evaluate((p) => ((window as any).__terminescu.host.inputs.get(p) ?? "") as string, pane);
  const dialog = await openSettings(page, "Keys");
  const newline = dialog.getByRole("button", { name: "Shortcut for New line without running (Claude)" });
  await expect(newline).toHaveText("off");
  await newline.click();
  await page.keyboard.press("Meta+P");
  await expect(dialog.getByRole("alert")).toHaveText(/^⌘P is already Command Palette/);
  await page.keyboard.press("Shift+Enter");
  await expect(newline).toHaveText("⇧↵");
  await dialog.getByRole("button", { name: "Turn off Delete to line start" }).click();
  await expect(dialog.getByRole("button", { name: "Shortcut for Delete to line start" })).toHaveText("off");
  await expect.poll(async () => (await mock(page, "settings")).terminalKeys).toMatchObject({ newline: "Shift+Enter", deleteLine: "" });
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);

  await page.locator(".term-pane").first().click();
  const before = (await inputs()).length;
  await page.keyboard.press("Meta+Backspace");
  await page.keyboard.press("Shift+Enter");
  await expect.poll(async () => (await inputs()).slice(before)).toBe("\x1b\r");

  const again = await openSettings(page, "Keys");
  await again.getByRole("button", { name: "Reset Delete to line start" }).click();
  await expect(again.getByRole("button", { name: "Shortcut for Delete to line start" })).toHaveText("⌘⌫");
});
