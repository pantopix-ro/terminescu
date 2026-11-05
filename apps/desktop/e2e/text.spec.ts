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
