import { expect, test, type Page } from "@playwright/test";

// Task 8.55 on the mock host: a right click anywhere opens the app's pixel menu, never the
// system's. A terminal's and an editor's menus hold macOS's Edit items, sent down the responder
// chain to the element right-clicked, beside the app's own pane actions.

const edits = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.editCommands) as Promise<{ action: string; focus: string }[]>;
const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
}

test("a terminal's right click opens the pixel menu; Paste goes to that terminal", async ({ page }) => {
  await start(page);
  await page.locator(".term-pane").first().click({ button: "right", position: { x: 120, y: 80 } });
  const menu = page.getByRole("menu", { name: "Terminal" });
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("menuitem")).toHaveText([/Copy/, /Paste/, /Select All/, /Find…/, /Clear to Start/, /Split Right/, /Split Down/, /Maximize Pane/, /Move Pane to New Tab/, /Close Pane/]);
  await expect(menu.getByRole("menuitem", { name: /Paste/ })).toContainText("⌘V");
  await menu.getByRole("menuitem", { name: /Paste/ }).click();
  await expect(menu).toBeHidden();
  await expect.poll(() => edits(page)).toEqual([{ action: "paste", focus: "xterm-helper-textarea" }]);
});

test("Split Right from the menu splits the terminal", async ({ page }) => {
  await start(page);
  await page.locator(".term-pane").first().click({ button: "right" });
  await page.getByRole("menu", { name: "Terminal" }).getByRole("menuitem", { name: /Split Right/ }).click();
  await expect.poll(async () => (await workspace(page)).windows[0].sideTabs[0].tabs[0].layout.type).toBe("split");
});

test("a text pane's menu has Undo and Redo, and Escape gives the editor its focus back", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Alt+KeyT");
  const editor = page.locator(".text-pane .cm-content");
  await expect(editor).toBeFocused();
  await page.keyboard.type("draft");
  await editor.click({ button: "right" });
  const menu = page.getByRole("menu", { name: "Text" });
  await expect(menu.getByRole("menuitem")).toHaveText([/Undo/, /Redo/, /Cut/, /Copy/, /Paste/, /Select All/, /Find…/]);
  await page.keyboard.press("Escape");
  await expect(menu).toBeHidden();
  await expect(editor).toBeFocused();
  await editor.click({ button: "right" });
  await menu.getByRole("menuitem", { name: /Undo/ }).click();
  await expect.poll(() => edits(page)).toEqual([{ action: "undo", focus: expect.stringContaining("cm-content") }]);
});

test("a field gets Cut, Copy and Paste, the sidebar keeps its menu, bare chrome shows none", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+KeyP");
  const field = page.locator('[role="dialog"] input, .palette input').first();
  await field.click({ button: "right" });
  await expect(page.getByRole("menu", { name: "Field" }).getByRole("menuitem")).toHaveText([/Cut/, /Copy/, /Paste/, /Select All/]);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");

  // The sidebar keeps its own menu.
  await page.locator(".side-fill").click({ button: "right" });
  await expect(page.getByRole("menu", { name: "Side tabs" })).toBeVisible();
  await page.keyboard.press("Escape");

  // The tab bar's empty strip has nothing to offer: no menu, and the system's stays away.
  const shown = await page.evaluate(() => {
    const e = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 5, clientY: 5 });
    document.querySelector(".app-main")!.dispatchEvent(e);
    return e.defaultPrevented;
  });
  expect(shown).toBe(true);
  await expect(page.getByRole("menu")).toHaveCount(0);
});

test("selected text outside the panes gets Copy", async ({ page }) => {
  await start(page);
  await page.evaluate(() => {
    // WebKit's test browser has no clipboard permission: record the write instead.
    Object.defineProperty(navigator, "clipboard", { value: { writeText: async (t: string) => void ((window as any).copied = t) } });
    const p = document.createElement("p");
    p.id = "probe";
    p.textContent = "ticket summary";
    // The chrome is unselectable; text a person can select, as a transcript's, opts back in.
    p.style.webkitUserSelect = "text";
    document.body.append(p);
    // A mouse selection takes the focus off the terminal; WebKit keeps it in the textarea otherwise.
    (document.activeElement as HTMLElement).blur();
    getSelection()!.selectAllChildren(p);
  });
  // The probe sits under the app's full-window layout, so the event goes to it directly.
  await page.evaluate(() => document.getElementById("probe")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 300, clientY: 300 })));
  const menu = page.getByRole("menu", { name: "Selection" });
  await expect(menu.getByRole("menuitem")).toHaveText([/Copy/]);
  await menu.getByRole("menuitem").click();
  await expect.poll(() => page.evaluate(() => (window as any).copied)).toBe("ticket summary");
});
