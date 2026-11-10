import { expect, test, type Page } from "@playwright/test";

// Tasks 8.7 and 8.34 on the mock host: a right-click opens the edit dialog of a side tab or
// tab (name, icon, colour, status, actions); saved commands keep their menu.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
}

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

const send = (page: Page, command: unknown) => page.evaluate((c) => (window as any).__terminescu.host.send(c), command);

const sideRow = (page: Page, i = 0) => page.locator(".ao-side .ao-st").nth(i);

const item = (page: Page, name: string) => page.getByRole("menuitem", { name, exact: true });

const edit = (page: Page) => page.getByRole("dialog", { name: "Edit" });

test("right-click renames a side tab in its edit dialog; Esc keeps the name, blank goes back", async ({ page }) => {
  await start(page);
  const generated = (await sideRow(page).locator(".t").textContent())!;
  await sideRow(page).click({ button: "right" });
  await expect(edit(page)).toBeVisible();
  await expect(page.getByRole("menu")).toHaveCount(0);
  await page.getByLabel("Side tab name").fill("PROJ-482");
  await page.keyboard.press("Enter");
  await expect(edit(page)).toBeHidden();
  await expect(sideRow(page).locator(".t")).toHaveText("PROJ-482");
  expect((await workspace(page)).windows[0].sideTabs[0].title).toBe("PROJ-482");

  // Esc keeps the name it had.
  await sideRow(page).click({ button: "right" });
  await expect(page.getByLabel("Side tab name")).toHaveValue("PROJ-482");
  await page.getByLabel("Side tab name").fill("something else");
  await page.keyboard.press("Escape");
  await expect(edit(page)).toBeHidden();
  await expect(sideRow(page).locator(".t")).toHaveText("PROJ-482");

  await sideRow(page).click({ button: "right" });
  await page.getByLabel("Side tab name").fill("   ");
  await page.keyboard.press("Enter");
  await expect(sideRow(page).locator(".t")).toHaveText(generated);
  expect((await workspace(page)).windows[0].sideTabs[0].title).toBeNull();
});

test("the keyboard opens the edit dialog; its actions run; Esc closes it", async ({ page }) => {
  await start(page);
  await sideRow(page).focus();
  await page.keyboard.press("Shift+F10");
  await expect(edit(page)).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(edit(page)).toBeHidden();

  await sideRow(page).focus();
  await page.keyboard.press("Shift+F10");
  await edit(page).getByRole("button", { name: "New tab", exact: true }).click();
  await expect(edit(page)).toBeHidden();
  await expect.poll(async () => (await workspace(page)).windows[0].sideTabs[0].tabs.length).toBe(2);
});

test("the row colour comes from the neons and the tab bar opens a tab's dialog", async ({ page }) => {
  await start(page);
  await sideRow(page).click({ button: "right" });
  const colours = edit(page).getByRole("radiogroup", { name: "Row colour" });
  await colours.getByRole("radio", { name: "lime" }).click();
  await expect(colours.getByRole("radio", { name: "lime" })).toHaveAttribute("aria-checked", "true");
  await expect.poll(async () => (await workspace(page)).windows[0].sideTabs[0].accent).toBe("lime");
  await page.keyboard.press("Escape");

  const tab = page.locator(".ao-tabs [role=tab]").first();
  await tab.click({ button: "right" });
  await expect(edit(page)).toContainText("tab ·");
  await edit(page).getByRole("radiogroup", { name: "Tab colour" }).getByRole("radio", { name: "violet" }).click();
  await expect.poll(async () => (await workspace(page)).windows[0].sideTabs[0].tabs[0].accent).toBe("violet");
  await expect(tab).toHaveAttribute("style", /--neon-violet/);
});

test("merge into is a drop-down of the other side tabs", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "New terminal" }).click();
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(2);
  const ws = await workspace(page);
  const [a, b] = ws.windows[0].sideTabs;
  await send(page, { type: "RenameSideTab", sideTabId: b.id, title: "PROJ-477" });
  await sideRow(page, 0).click({ button: "right" });
  await edit(page).getByLabel("Merge into").selectOption({ label: "PROJ-477" });
  // No clash, so it merges straight in and offers undo.
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(1);
  await expect(page.locator(".toast")).toContainText("Merged");
  const after = await workspace(page);
  expect(after.windows[0].sideTabs[0].id).toBe(b.id);
  expect(after.windows[0].sideTabs[0].tabs.map((t: { id: string }) => t.id)).toContain(a.tabs[0].id);
});

test("move a side tab to a new window keeps its id; close asks first", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "New terminal" }).click();
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(2);
  const second = (await workspace(page)).windows[0].sideTabs[1];

  await sideRow(page, 1).click({ button: "right" });
  await edit(page).getByRole("button", { name: "Move to new window" }).click();
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(1);
  let ws = await workspace(page);
  expect(ws.windows).toHaveLength(2);
  expect(ws.windows[1].sideTabs[0]).toEqual(second);

  // Close side tab asks, and Cancel keeps it.
  await sideRow(page, 0).click({ button: "right" });
  await edit(page).getByRole("button", { name: "Close side tab…" }).click();
  const dialog = page.getByRole("alertdialog", { name: "Close side tab" });
  await expect(dialog).toContainText("its context goes with it");
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(1);

  // Closing the other window's side tab through the host closes that window.
  await send(page, { type: "CloseSideTab", sideTabId: second.id });
  ws = await workspace(page);
  expect(ws.windows).toHaveLength(1);
});

test("a tab under its side tab renames, moves and closes from its dialog", async ({ page }) => {
  await start(page);
  const side = (await workspace(page)).windows[0].sideTabs[0];
  await send(page, { type: "CreateTab", sideTabId: side.id });
  await send(page, { type: "CreateTab", sideTabId: side.id });
  const kids = page.locator(".ao-st-kids .ao-st-kid");
  await expect(kids).toHaveCount(3);

  await kids.nth(0).click({ button: "right" });
  await expect(edit(page)).toBeVisible();
  await page.getByLabel("Tab name").fill("dev server");
  await page.keyboard.press("Enter");
  await expect(kids.nth(0)).toContainText("dev server");
  expect((await workspace(page)).windows[0].sideTabs[0].tabs[0].title).toBe("dev server");

  await kids.nth(1).click({ button: "right" });
  await edit(page).getByLabel("Move to").selectOption({ label: "New side tab" });
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(2);

  // The view followed the moved tab; back to the first side tab, now with two tabs.
  await sideRow(page, 0).click();
  await expect(kids).toHaveCount(2);
  await kids.nth(1).click({ button: "right" });
  await edit(page).getByRole("button", { name: "Close tab" }).click();
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

test("a deleted command comes back with Undo", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "New command" }).click();
  await page.getByLabel("Command name").fill("checks");
  await page.getByLabel("Command", { exact: true }).fill("gh pr checks 918");
  await page.locator(".cmd-form").getByRole("button", { name: "Save" }).click();
  const row = page.locator('.cmd-row[data-command="checks"]');
  await row.click({ button: "right" });
  await item(page, "Delete").click();
  await expect(row).toBeHidden();
  await expect(page.locator(".toast")).toContainText("Deleted checks.");
  await page.locator(".toast").getByRole("button", { name: "Undo" }).click();
  await expect(row).toBeVisible();
  await expect(page.locator(".toast")).toHaveCount(0);
});
