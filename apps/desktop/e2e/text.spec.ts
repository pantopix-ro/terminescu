import { expect, test, type Page } from "@playwright/test";

// Group 7 on the mock host: text panes open with ⌥⌘T, save with ⌘S, and show a bar when the
// file changes on disk under unsaved edits (D27, D28).


async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
}

const files = (page: Page) =>
  page.evaluate(() => Object.fromEntries((window as any).__terminescu.host.files)) as Promise<Record<string, string>>;

test("⌥⌘T opens an Untitled text tab; ⌘S asks for a path and writes the text", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Alt+KeyT");
  const tab = page.locator(".app-main .ao-tab").nth(1);
  await expect(tab).toContainText("Untitled");
  const editor = page.locator(".text-pane .cm-content");
  await expect(editor).toBeFocused();
  await page.keyboard.type("# Notes for PROJ-482");
  await expect(tab.locator(".tab-dirty")).toBeVisible();
  await page.evaluate(() => (window as any).__terminescu.host.dialogAnswers.push("/Users/owner/acme-api/notes.md"));
  await page.keyboard.press("Meta+KeyS");
  await expect(tab).toContainText("notes.md");
  await expect(tab.locator(".tab-dirty")).toHaveCount(0);
  expect((await files(page))["/Users/owner/acme-api/notes.md"]).toBe("# Notes for PROJ-482");
  expect(await page.evaluate(() => (window as any).__terminescu.host.dialogs)).toEqual(["save"]);
  await expect(page.locator(".text-status")).toContainText("Markdown");
});

test("a change on disk reloads a clean buffer and puts a bar over a dirty one", async ({ page }) => {
  await start(page);
  const side = await page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].sideTabs[0].id as string);
  const path = "/Users/owner/acme-api/README.md";
  await page.evaluate(
    ({ side, path }) => {
      const h = (window as any).__terminescu.host;
      h.files.set(path, "first\r\nsecond\r\n");
      return h.openFile(side, path);
    },
    { side, path },
  );
  await page.locator(".app-main .ao-tab").nth(1).click();
  const editor = page.locator(".text-pane .cm-content");
  await expect(editor).toContainText("second");
  await expect(page.locator(".text-status")).toContainText("CRLF");
  await page.evaluate(({ path }) => (window as any).__terminescu.host.writeFile(path, "first\r\nchanged\r\n"), { path });
  await expect(editor).toContainText("changed");
  await editor.click();
  await page.keyboard.press("End");
  await page.keyboard.type(" mine");
  // Wait for the edit to reach the host before the disk changes again.
  await expect(page.locator(".app-main .ao-tab").nth(1).locator(".tab-dirty")).toBeVisible();
  await page.evaluate(({ path }) => (window as any).__terminescu.host.writeFile(path, "theirs\r\n"), { path });
  const bar = page.getByRole("alert").filter({ hasText: "changed on disk" });
  await expect(bar).toBeVisible();
  await bar.getByRole("button", { name: "Reload" }).click();
  await expect(bar).toHaveCount(0);
  await expect(editor).toContainText("theirs");
  await expect(editor).not.toContainText("mine");
});

async function dirtyText(page: Page, text: string) {
  await page.keyboard.press("Meta+Alt+KeyT");
  await expect(page.locator(".text-pane .cm-content")).toBeFocused();
  await page.keyboard.type(text);
  await expect(page.locator(".app-main .ao-tab .tab-dirty")).toBeVisible();
}

test("closing a dirty text tab asks: Cancel keeps it, Don't save drops it", async ({ page }) => {
  await start(page);
  await dirtyText(page, "draft for acme/api");
  await page.keyboard.press("Meta+KeyW");
  const dialog = page.getByRole("alertdialog", { name: "Unsaved changes" });
  await expect(dialog).toContainText("Save changes to Untitled?");
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(2);
  await page.locator(".app-main .ao-tab").nth(1).getByRole("button", { name: "Close tab" }).click();
  await dialog.getByRole("button", { name: "Don't save" }).click();
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(1);
  expect(await files(page)).toEqual({});
});

test("Save on close writes the file first; a cancelled Save dialog keeps the tab", async ({ page }) => {
  await start(page);
  await dirtyText(page, "release notes");
  await page.keyboard.press("Meta+KeyW");
  const dialog = page.getByRole("alertdialog", { name: "Unsaved changes" });
  // The Save dialog is cancelled: nothing closes.
  await page.evaluate(() => (window as any).__terminescu.host.dialogAnswers.push(null));
  await dialog.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(2);
  await page.locator(".text-pane .cm-content").click();
  await page.keyboard.press("Meta+KeyW");
  await page.evaluate(() => (window as any).__terminescu.host.dialogAnswers.push("/Users/owner/acme-api/NOTES.md"));
  await dialog.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(1);
  expect((await files(page))["/Users/owner/acme-api/NOTES.md"]).toBe("release notes");
});

test("a clean text tab closes without asking", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Alt+KeyT");
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(2);
  await page.keyboard.press("Meta+KeyW");
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(1);
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
});

async function seed(page: Page, paths: string[]) {
  await page.evaluate((paths) => {
    const h = (window as any).__terminescu.host;
    for (const p of paths) h.files.set(p, `# ${p.split("/").pop()}\n`);
  }, paths);
}

test("⌘O finds a file under the pane's folder; ⌥↵ opens it beside; ⌘O again opens the dialog", async ({ page }) => {
  await start(page);
  await seed(page, ["/Users/owner/acme-api/README.md", "/Users/owner/acme-api/src/audit_export.rs", "/Users/owner/acme-api/src/main.rs"]);
  await page.keyboard.press("Meta+KeyO");
  const finder = page.locator(".palette");
  await expect(finder.getByRole("option", { name: "src/main.rs" })).toBeVisible();
  await page.keyboard.type("audit");
  await expect(finder.getByRole("option")).toHaveText(["src/audit_export.rs", "Browse…⌘O"]);
  await page.keyboard.press("Enter");
  await expect(page.locator(".app-main .ao-tab").nth(1)).toContainText("audit_export.rs");
  await expect(page.locator(".text-pane .cm-content")).toBeFocused();

  // Back in the terminal tab: ⌥↵ splits beside the terminal.
  await page.locator(".app-main .ao-tab").first().click();
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+KeyO");
  await page.keyboard.type("readme");
  await page.keyboard.press("Alt+Enter");
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(2);
  await expect(page.locator(".app-main .ao-tab").first()).toContainText("README.md");
  await expect(page.locator(".ao-tp")).toHaveCount(2);

  await page.keyboard.press("Meta+KeyO");
  await expect(page.locator(".palette")).toBeVisible();
  await page.evaluate(() => (window as any).__terminescu.host.dialogAnswers.push("/Users/owner/acme-api/src/main.rs"));
  await page.keyboard.press("Meta+KeyO");
  await expect(page.locator(".palette")).toHaveCount(0);
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(3);
  expect(await page.evaluate(() => (window as any).__terminescu.host.dialogs)).toEqual(["open"]);
});

test("+ ▾ › Text opens an Untitled tab and a Finder drop opens the file", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Choose tab kind" }).click();
  await page.getByRole("menuitem", { name: /Text/ }).click();
  await expect(page.locator(".app-main .ao-tab").nth(1)).toContainText("Untitled");
  await seed(page, ["/Users/owner/acme-api/CHANGELOG.md"]);
  await page.evaluate(() => (window as any).__terminescu.host.dropFiles(["/Users/owner/acme-api/CHANGELOG.md"]));
  await expect(page.locator(".app-main .ao-tab").nth(2)).toContainText("CHANGELOG.md");
});

test("an editor request splits beside the caller, marks the tab and returns on ⌘↵", async ({ page }) => {
  await start(page);
  const path = "/Users/owner/acme-api/.git/COMMIT_EDITMSG";
  await page.evaluate((path) => (window as any).__terminescu.host.files.set(path, "\n# Please enter the commit message\n"), path);
  const pane = await page.locator(".ao-tp").first().getAttribute("data-pane");
  await page.evaluate(
    ({ pane, path }) => {
      const w = window as any;
      w.__edited = "waiting";
      void w.__terminescu.host.editWait(pane, path).then(() => (w.__edited = "returned"));
    },
    { pane, path },
  );
  await expect(page.locator(".ao-tp")).toHaveCount(2);
  await expect(page.locator(".app-main .ao-tab").first()).toHaveClass(/wait/);
  const editor = page.locator(".text-pane .cm-content");
  await expect(editor).toBeFocused();
  await page.keyboard.press("Meta+ArrowUp");
  await page.keyboard.type("Add the audit export");
  await page.keyboard.press("Meta+Enter");
  await expect(page.locator(".ao-tp")).toHaveCount(1);
  await expect.poll(() => page.evaluate(() => (window as any).__edited)).toBe("returned");
  expect((await files(page))[path]).toMatch(/^Add the audit export\n/);
  await expect(page.locator(".app-main .ao-tab").first()).not.toHaveClass(/wait/);
});

test("closing an editor request's pane ends the wait", async ({ page }) => {
  await start(page);
  const path = "/Users/owner/acme-api/.git/COMMIT_EDITMSG";
  await page.evaluate((path) => (window as any).__terminescu.host.files.set(path, ""), path);
  const pane = await page.locator(".ao-tp").first().getAttribute("data-pane");
  await page.evaluate(
    ({ pane, path }) => {
      const w = window as any;
      void w.__terminescu.host.editWait(pane, path).then(() => (w.__edited = "ended"));
    },
    { pane, path },
  );
  await expect(page.locator(".text-pane .cm-content")).toBeFocused();
  await page.keyboard.press("Meta+KeyW");
  await expect(page.locator(".ao-tp")).toHaveCount(1);
  await expect.poll(() => page.evaluate(() => (window as any).__edited)).toBe("ended");
});
