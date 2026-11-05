import { expect, test, type Page } from "@playwright/test";

// Task 8.7 on the mock host: right-click menus on side tabs, their tabs and saved commands.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
}

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

const send = (page: Page, command: unknown) => page.evaluate((c) => (window as any).__terminescu.host.send(c), command);

const sideRow = (page: Page, i = 0) => page.locator(".ao-side .ao-st").nth(i);

const item = (page: Page, name: string) => page.getByRole("menuitem", { name, exact: true });

test("rename a side tab in place; a blank name goes back to the generated one", async ({ page }) => {
  await start(page);
  const generated = (await sideRow(page).locator(".t").textContent())!;
  await sideRow(page).click({ button: "right" });
  await expect(page.getByRole("menu", { name: "Side tab" })).toBeVisible();
  await item(page, "Rename…").click();
  await page.getByLabel("Side tab name").fill("PROJ-482");
  await page.keyboard.press("Enter");
  await expect(sideRow(page).locator(".t")).toHaveText("PROJ-482");
  expect((await workspace(page)).windows[0].sideTabs[0].title).toBe("PROJ-482");

  // Esc keeps the name it had.
  await sideRow(page).click({ button: "right" });
  await item(page, "Rename…").click();
  await page.getByLabel("Side tab name").fill("something else");
  await page.keyboard.press("Escape");
  await expect(sideRow(page).locator(".t")).toHaveText("PROJ-482");

  await sideRow(page).click({ button: "right" });
  await item(page, "Rename…").click();
  await page.getByLabel("Side tab name").fill("   ");
  await page.keyboard.press("Enter");
  await expect(sideRow(page).locator(".t")).toHaveText(generated);
  expect((await workspace(page)).windows[0].sideTabs[0].title).toBeNull();
});

test("the keyboard opens the menu, walks it and runs an item; Esc closes it", async ({ page }) => {
  await start(page);
  await sideRow(page).focus();
  await page.keyboard.press("Shift+F10");
  const menu = page.getByRole("menu", { name: "Side tab" });
  await expect(menu).toBeVisible();
  await expect(item(page, "Rename…")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(menu).toBeHidden();
  await expect(sideRow(page)).toBeFocused();

  // Down past "Change icon…" to "New tab", Enter: the side tab gets a second tab.
  await page.keyboard.press("Shift+F10");
  await page.keyboard.press("ArrowDown");
  await expect(item(page, "Change icon…")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(item(page, "New tab")).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(menu).toBeHidden();
  expect((await workspace(page)).windows[0].sideTabs[0].tabs).toHaveLength(2);
});

test("merge into opens a submenu of the other side tabs", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Add a side tab" }).click();
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(2);
  const ws = await workspace(page);
  const [a, b] = ws.windows[0].sideTabs;
  await send(page, { type: "RenameSideTab", sideTabId: b.id, title: "PROJ-477" });
  await sideRow(page, 0).click({ button: "right" });
  await item(page, "Merge into").hover();
  await item(page, "PROJ-477").click();
  // No clash, so it merges straight in and offers undo.
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(1);
  await expect(page.locator(".toast")).toContainText("Merged");
  const after = await workspace(page);
  expect(after.windows[0].sideTabs[0].id).toBe(b.id);
  expect(after.windows[0].sideTabs[0].tabs.map((t: { id: string }) => t.id)).toContain(a.tabs[0].id);
});

test("move a side tab to a new window keeps its id; close asks first", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Add a side tab" }).click();
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(2);
  const second = (await workspace(page)).windows[0].sideTabs[1];

  await sideRow(page, 1).click({ button: "right" });
  await item(page, "Move to new window").click();
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(1);
  let ws = await workspace(page);
  expect(ws.windows).toHaveLength(2);
  expect(ws.windows[1].sideTabs[0]).toEqual(second);

  // Close side tab asks, and Cancel keeps it.
  await sideRow(page, 0).click({ button: "right" });
  await item(page, "Close side tab…").click();
  const dialog = page.getByRole("alertdialog", { name: "Close side tab" });
  await expect(dialog).toContainText("its context goes with it");
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(1);

  // Closing the other window's side tab through the host closes that window.
  await send(page, { type: "CloseSideTab", sideTabId: second.id });
  ws = await workspace(page);
  expect(ws.windows).toHaveLength(1);
});

test("a tab under its side tab renames, moves and closes from its menu", async ({ page }) => {
  await start(page);
  const side = (await workspace(page)).windows[0].sideTabs[0];
  await send(page, { type: "CreateTab", sideTabId: side.id });
  await send(page, { type: "CreateTab", sideTabId: side.id });
  const kids = page.locator(".ao-st-kids .ao-st-kid");
  await expect(kids).toHaveCount(3);

  await kids.nth(0).click({ button: "right" });
  await expect(page.getByRole("menu", { name: "Tab" })).toBeVisible();
  await item(page, "Rename…").click();
  await page.getByLabel("Tab name").fill("dev server");
  await page.keyboard.press("Enter");
  await expect(kids.nth(0)).toContainText("dev server");
  expect((await workspace(page)).windows[0].sideTabs[0].tabs[0].title).toBe("dev server");

  // Move to › New side tab, by keyboard from the submenu.
  await kids.nth(1).click({ button: "right" });
  await item(page, "Move to").focus();
  await page.keyboard.press("ArrowRight");
  await expect(item(page, "New side tab")).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(2);

  // The view followed the moved tab; back to the first side tab, now with two tabs.
  await sideRow(page, 0).click();
  await expect(kids).toHaveCount(2);
  await kids.nth(1).click({ button: "right" });
  await item(page, "Close tab").click();
  const ws = await workspace(page);
  expect(ws.windows[0].sideTabs.flatMap((s: { tabs: unknown[] }) => s.tabs)).toHaveLength(2);
});

test("a command edits, duplicates and deletes from its menu", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "New command" }).click();
  await page.getByLabel("Command name").fill("checks");
  await page.getByLabel("Command", { exact: true }).fill("gh pr checks 918");
  await page.locator(".cmd-form").getByRole("button", { name: "Save" }).click();
  const row = page.locator('.cmd-row[data-command="checks"]');
  await expect(row).toBeVisible();

  await row.click({ button: "right" });
  await item(page, "Edit…").click();
  const form = page.getByRole("form", { name: "Edit checks" });
  await expect(form.getByLabel("Command", { exact: true })).toHaveValue("gh pr checks 918");
  await form.getByLabel("Command", { exact: true }).fill("gh pr checks 919");
  await form.getByRole("button", { name: "Save" }).click();
  await expect(row.locator(".cmd-body")).toHaveText("gh pr checks 919");

  await row.click({ button: "right" });
  await item(page, "Duplicate").click();
  await expect(page.locator('.cmd-row[data-command="checks copy"]')).toBeVisible();

  await row.click({ button: "right" });
  await item(page, "Delete").click();
  await expect(row).toBeHidden();
  await expect(page.locator(".cmd-row")).toHaveCount(1);
});
