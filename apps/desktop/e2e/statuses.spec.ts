import { expect, test, type Page } from "@playwright/test";

// Group 5 on the mock host: manual statuses, sort, the amber marker over a manual status,
// rules and CLI statuses. The mock plays the core's part, as in agents.spec.ts.

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

async function open(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  const pane = await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane");
  await page.locator(".xterm").first().click();
  return pane!;
}

test("a manual status shows on the side tab until back to auto", async ({ page }) => {
  await open(page);
  const side = page.locator(".ao-st").first();
  await expect(side.locator(".ao-stat")).toHaveText("Idle");

  await page.keyboard.press("Meta+Shift+S");
  const editor = page.getByRole("dialog");
  await expect(editor).toBeVisible();
  await editor.getByLabel("Status label").fill("Review");
  await editor.getByRole("radio", { name: "Magenta", exact: true }).click();
  await editor.getByRole("radio", { name: "Magenta tint" }).click();
  await editor.getByLabel("Status label").press("Enter");
  await expect(editor).toBeHidden();
  await expect(side.locator(".ao-stat.manual")).toHaveText("Review");
  // The row's neon washes over the status background (task 8.25).
  await expect(side).toHaveAttribute("style", /--st-bg: color-mix\(in srgb, var\(--neon-\w+\) 12%, var\(--tint-magenta\)\)/);

  await page.keyboard.press("Meta+Shift+S");
  await expect(editor.getByLabel("Status label")).toHaveValue("Review");
  await editor.getByRole("button", { name: "back to auto" }).click();
  await editor.locator(".ao-frame-foot").getByRole("button", { name: /Done/ }).click();
  await expect(side.locator(".ao-stat")).toHaveText("Idle");
  await expect(side.locator(".ao-stat.manual")).toHaveCount(0);
});

test("a waiting session shows amber over a manual status", async ({ page }) => {
  const pane = await open(page);
  const side = page.locator(".ao-st").first();
  await page.keyboard.press("Meta+Shift+S");
  await page.getByLabel("Status label").fill("Blocked");
  await page.getByLabel("Status label").press("Enter");
  await expect(side.locator(".ao-stat")).toHaveText("Blocked");

  await mock(page, "setAgent", pane, { session: "sess-0b5c", state: "waiting", lastMessage: null });
  await expect(side).toHaveClass(/\bneed\b/);
  await expect(side.locator(".ao-stat.manual")).toHaveText("Blocked");
  await mock(page, "setAgent", pane, { session: "sess-0b5c", state: "running", lastMessage: null });
  await expect(side).not.toHaveClass(/\bneed\b/);
});

// The sort lives in the menu of the empty space under the rows.
async function sortBy(page: Page, mode: string) {
  await page.locator(".side-fill").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Sort side tabs" }).click();
  await page.getByRole("menuitem", { name: new RegExp(`^${mode}`) }).click();
}

test("sorting by background groups tints, by name inside each", async ({ page }) => {
  await open(page);
  const windowId = await page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].id);
  const rows = [
    ["zeta", "red"],
    ["alpha", "blue"],
    ["mid", null],
    ["beta", "red"],
  ] as const;
  for (const [name, bg] of rows) {
    await mock(page, "send", { type: "CreateSideTab", windowId });
    const ws = await page.evaluate(() => (window as any).__terminescu.host.workspace());
    const side = ws.windows[0].sideTabs.at(-1);
    await mock(page, "send", { type: "RenameTab", tabId: side.tabs[0].id, title: name });
    if (bg) await mock(page, "send", { type: "SetStatus", targetId: side.id, status: { label: "Tag", accent: null, background: bg } });
  }
  const names = page.locator(".ao-st .t");
  await expect(names).toHaveText(["shell", "zeta", "alpha", "mid", "beta"]);
  await sortBy(page, "background");
  await expect(names).toHaveText(["alpha", "beta", "zeta", "mid", "shell"]);
  await sortBy(page, "name");
  await expect(names).toHaveText(["alpha", "beta", "mid", "shell", "zeta"]);
  // The choice survives a reload.
  await page.reload();
  await page.locator(".side-fill").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Sort side tabs" }).click();
  await expect(page.getByRole("menuitem", { name: "name ✓" })).toBeVisible();
});

test("an exit-code rule and terminescu status set dynamic statuses", async ({ page }) => {
  const pane = await open(page);
  const side = page.locator(".ao-st").first();
  const saved = await mock(page, "saveStatus", null, { label: "Tests red", accent: "red", background: null });
  await mock(page, "addRule", (saved as { id: string }).id, { kind: "exitCode", pattern: "pnpm test*" });

  await mock(page, "runCommand", pane, "pnpm test --filter api", 1);
  await expect(side.locator(".ao-stat.manual")).toHaveText("Tests red");
  // The next run of the command clears it; the auto status shows again.
  await mock(page, "runCommand", pane, "pnpm test", null);
  await expect(side.locator(".ao-stat")).toHaveText("Running");
  await mock(page, "runCommand", pane, "pnpm test", 0);
  await expect(side.locator(".ao-stat")).toHaveText("Done");

  await mock(page, "setCliStatus", pane, { label: "Migrating", accent: "cyan", background: null });
  await expect(side.locator(".ao-stat.manual")).toHaveText("Migrating");
  await mock(page, "setCliStatus", pane, null);
  await expect(side.locator(".ao-stat")).toHaveText("Done");
});

test("Settings adds a global status the editor offers, and deleting it goes back to auto", async ({ page }) => {
  await open(page);
  const side = page.locator(".ao-st").first();
  await page.keyboard.press("Meta+Comma");
  const settings = page.getByRole("dialog");
  await expect(settings.locator("[data-builtin]")).toHaveCount(5);
  await settings.getByLabel("New global status").fill("Parked");
  await settings.getByRole("button", { name: "Add", exact: true }).click();
  await expect(settings.locator('[data-global="Parked"]')).toBeVisible();
  await settings.locator(".ao-frame-foot").getByRole("button", { name: "Done" }).click();

  await page.keyboard.press("Meta+Shift+S");
  await page.getByRole("dialog").locator(".preset", { hasText: "Parked" }).click();
  await page.locator(".status-editor .ao-frame-foot").getByRole("button", { name: /Done/ }).click();
  await expect(side.locator(".ao-stat.manual")).toHaveText("Parked");

  await page.keyboard.press("Meta+Comma");
  await page.getByRole("button", { name: "Delete Parked" }).click();
  await page.locator(".ao-frame-foot").getByRole("button", { name: "Done" }).click();
  await expect(side.locator(".ao-stat")).toHaveText("Idle");
});
