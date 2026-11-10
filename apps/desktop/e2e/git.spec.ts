import { expect, test, type Page } from "@playwright/test";

// add-git-plugin group 3 on the mock host: a fake git Rust half answers `changes` from
// `window.__fakeGit` and the test plays its chips through `decorate`.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?plugins=git");
  await expect(page.locator(".term-pane")).toHaveCount(1);
  return page.evaluate(() => {
    const ws = (window as any).__terminescu.host.workspace();
    const side = ws.windows[0].sideTabs[0];
    const tab = side.tabs[0];
    return { side: side.id as string, tab: tab.id as string, pane: tab.layout.id as string };
  });
}

const host = (page: Page, run: string, ...args: unknown[]) =>
  page.evaluate(({ run, args }) => (window as any).__terminescu.host[run](...args), { run, args });

const branch = (tab: string, text: string, tone = "neutral", tip?: string) => ({
  plugin: "git",
  target: { kind: "tab", id: tab },
  slot: "chip",
  key: "branch",
  text,
  tone,
  ...(tip ? { tip } : {}),
});

const change = (path: string, c: Record<string, unknown> = {}) => ({
  path,
  staged: false,
  unstaged: true,
  untracked: false,
  conflicted: false,
  added: 12,
  removed: 3,
  binary: false,
  ...c,
});

const answer = (page: Page, pane: string, changes: unknown[], total = changes.length) =>
  page.evaluate(({ pane, changes, total }) => ((window as any).__fakeGit = { [pane]: { changes: { root: "/code/shop-api", total, changes } } }), { pane, changes, total });

test("the branch chip shows on the row with its ahead mark, and turns red on a conflict", async ({ page }) => {
  const { tab } = await start(page);
  await host(page, "decorate", branch(tab, "feature/PROJ-482-retry ↑2"));
  const chip = page.locator('.app-side [data-plugin="git"][data-key="branch"]');
  await expect(chip).toHaveText("feature/PROJ-482-retry ↑2");
  await expect(chip).toHaveClass(/pl-neutral/);
  await host(page, "decorate", branch(tab, "feature/PROJ-482-retry •", "bad", "Rebase stopped on a conflict"));
  await expect(chip).toHaveClass(/pl-bad/);
  await expect(chip).toHaveAttribute("title", "Rebase stopped on a conflict");
  // A long name shortens in the middle; the tip keeps it whole.
  const long = "feature/PROJ-4821-retry-failed-payments-after-timeout";
  await host(page, "decorate", branch(tab, `${long} ↑1`));
  await expect(chip).toHaveText(/^feature\/PROJ-482.*….*after-timeout ↑1$/);
  await expect(chip).toHaveAttribute("title", long);
  await host(page, "setPluginEnabled", "git", false);
  await expect(chip).toHaveCount(0);
});

test("the Changes section lists the shown tab's changes and reads them again on new facts", async ({ page }) => {
  const { pane } = await start(page);
  await answer(page, pane, [change("src/retry.ts"), change("src/new.ts", { untracked: true, unstaged: false, added: 4, removed: 0 }), change("logo.png", { binary: true, added: null, removed: null })]);
  await page.keyboard.press("Meta+Backslash");
  const section = page.locator('aside.ctx-pinned section[data-section="git.changes"]');
  await expect(section.locator("h3")).toHaveText("Changes");
  const items = section.locator(".git-change");
  await expect(items).toHaveCount(3);
  const retry = section.locator('.git-change[data-path="src/retry.ts"]');
  await expect(retry.locator(".git-name")).toHaveText("retry.ts");
  await expect(retry.locator(".git-lines")).toHaveText("+12 −3");
  await expect(retry.locator(".git-mark")).toHaveText("M");
  await expect(section.locator('.git-change[data-path="src/new.ts"] .git-mark')).toHaveText("U");
  await expect(section.locator('.git-change[data-path="logo.png"]')).toContainText("binary");
  // The Rust half read the pane's facts again: the list follows.
  await answer(page, pane, [change("src/retry.ts", { staged: true, unstaged: false })], 812);
  await host(page, "pluginEmit", "git", "facts", { pane });
  await expect(items).toHaveCount(1);
  await expect(retry.locator(".git-mark")).toHaveText("S");
  await expect(section.locator(".git-total")).toHaveText("The first 1 of 812 changed files.");
  // Another pane's facts leave it alone.
  await answer(page, pane, []);
  await host(page, "pluginEmit", "git", "facts", { pane: "elsewhere" });
  await expect(items).toHaveCount(1);
  await host(page, "pluginEmit", "git", "facts", { pane });
  await expect(section.locator(".ctx-empty")).toHaveText("No changes.");
});

test("outside a repository the Changes section says so", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Backslash");
  const section = page.locator('aside.ctx-pinned section[data-section="git.changes"]');
  await expect(section.locator(".git-off")).toHaveText(/not in a git repository/);
});

const files = (page: Page) =>
  page.evaluate(() => Object.fromEntries((window as any).__terminescu.host.files)) as Promise<Record<string, string>>;

test("a diff tab edits and saves, and switches between side by side and unified", async ({ page }) => {
  const { pane } = await start(page);
  const root = "/Users/owner/shop-api";
  await page.evaluate(
    ({ pane, root, retry }) => {
      (window as any).__fakeGit = {
        [pane]: { changes: { root, total: 1, changes: [retry] } },
        [root]: { base: { "src/retry.ts": "const tries = 3;\nexport { tries };\n" } },
      };
      (window as any).__terminescu.host.files.set(`${root}/src/retry.ts`, "const tries = 5;\nexport { tries };\n");
    },
    { pane, root, retry: change("src/retry.ts", { added: 1, removed: 1 }) },
  );
  await page.keyboard.press("Meta+Backslash");
  await page.locator('aside.ctx-pinned .git-change[data-path="src/retry.ts"] .git-path').click();
  // The base on the left, read-only; the working copy on the right.
  const sides = page.locator(".cm-mergeView .cm-editor");
  await expect(sides).toHaveCount(2);
  await expect(sides.nth(0)).toContainText("const tries = 3;");
  await expect(sides.nth(1)).toContainText("const tries = 5;");
  await expect(page.locator(".diff-note")).toHaveText("Changes since HEAD.");
  await expect(page.locator(".cm-mergeView .cm-changedLine").first()).toBeVisible();
  await expect(sides.nth(0).locator(".cm-content")).toHaveAttribute("contenteditable", "false");
  // The working side is the text tab's own document: typing and ⌘S write the file.
  await sides.nth(1).locator(".cm-line").first().click();
  await page.keyboard.press("Meta+ArrowUp");
  await page.keyboard.type("// retry on timeout\n");
  await page.keyboard.press("Meta+KeyS");
  await expect.poll(async () => (await files(page))[`${root}/src/retry.ts`]).toBe("// retry on timeout\nconst tries = 5;\nexport { tries };\n");
  // Unified: one editor, the removed line above the one that replaced it.
  await page.locator(".diff-mode").click();
  await expect(page.locator(".cm-mergeView")).toHaveCount(0);
  await expect(page.locator(".text-pane .cm-deletedChunk")).toContainText("const tries = 3;");
  await expect(page.locator(".text-pane .cm-content")).toContainText("// retry on timeout");
  // ⌥⌘U switches back.
  await page.locator(".text-pane .cm-content").click();
  await page.keyboard.press("Meta+Alt+KeyU");
  await expect(page.locator(".cm-mergeView .cm-editor")).toHaveCount(2);
});

// harden-plugin-host D6: the document keeps its diff view; a plain open shows it as text.
test("a diff tab's document keeps its view until a plain open", async ({ page }) => {
  const { pane } = await start(page);
  const root = "/Users/owner/acme-api";
  await page.evaluate(
    ({ pane, root, retry }) => {
      (window as any).__fakeGit = {
        [pane]: { changes: { root, total: 1, changes: [retry] } },
        [root]: { base: { "src/retry.ts": "const tries = 3;\n" } },
      };
      (window as any).__terminescu.host.files.set(`${root}/src/retry.ts`, "const tries = 5;\n");
    },
    { pane, root, retry: change("src/retry.ts", { added: 1, removed: 1 }) },
  );
  await page.keyboard.press("Meta+Backslash");
  await page.locator('aside.ctx-pinned .git-change[data-path="src/retry.ts"] .git-path').click();
  await expect(page.locator(".cm-mergeView .cm-editor")).toHaveCount(2);
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+KeyO");
  await page.keyboard.type("retry");
  await page.keyboard.press("Enter");
  await expect(page.locator(".cm-mergeView")).toHaveCount(0);
  await expect(page.locator(".text-pane .cm-content")).toContainText("const tries = 5;");
});

test("a new file's diff tab compares with nothing", async ({ page }) => {
  const { pane } = await start(page);
  const root = "/Users/owner/shop-api";
  await page.evaluate(
    ({ pane, root, added }) => {
      (window as any).__fakeGit = { [pane]: { changes: { root, total: 1, changes: [added] } }, [root]: { base: {} } };
      (window as any).__terminescu.host.files.set(`${root}/src/new.ts`, "export const added = true;\n");
    },
    { pane, root, added: change("src/new.ts", { untracked: true, unstaged: false, added: 1, removed: 0 }) },
  );
  await page.keyboard.press("Meta+Backslash");
  await page.locator('aside.ctx-pinned .git-change[data-path="src/new.ts"] .git-path').click();
  await expect(page.locator(".diff-note")).toHaveText("New file: not in HEAD.");
  await expect(page.locator(".cm-mergeView .cm-editor").nth(1)).toContainText("export const added = true;");
});

// polish-git-plugin D7: a rename compares with its old path; a deleted file shows read-only.
test("a renamed file diffs against its old path, and a deleted one opens read-only", async ({ page }) => {
  const { pane } = await start(page);
  const root = "/Users/owner/shop-api";
  await page.evaluate(
    ({ pane, root, renamed, gone }) => {
      (window as any).__fakeGit = {
        [pane]: { changes: { root, total: 2, changes: [renamed, gone] } },
        [root]: { base: { "src/old.ts": "export const name = 'old';\n", "src/gone.ts": "export const gone = 1;\n" } },
      };
      (window as any).__terminescu.host.files.set(`${root}/src/new.ts`, "export const name = 'new';\n");
    },
    { pane, root, renamed: change("src/new.ts", { from: "src/old.ts", staged: true }), gone: change("src/gone.ts", { deleted: true, added: 0, removed: 1 }) },
  );
  await page.keyboard.press("Meta+Backslash");
  await page.locator('aside.ctx-pinned .git-change[data-path="src/new.ts"] .git-path').click();
  const sides = page.locator(".cm-mergeView .cm-editor");
  await expect(sides.nth(0)).toContainText("export const name = 'old';");
  await expect(page.locator(".diff-note")).toHaveText("Changes since HEAD.");
  await page.locator('aside.ctx-pinned .git-change[data-path="src/gone.ts"] .git-path').click();
  const pane2 = page.locator(".text-pane").filter({ hasText: "Deleted:" });
  await expect(pane2.locator(".diff-note")).toHaveText("Deleted: the file is gone from the working tree.");
  await expect(pane2.locator(".cm-mergeView .cm-editor").nth(0)).toContainText("export const gone = 1;");
  await expect(pane2.locator(".cm-mergeView .cm-editor").nth(1).locator(".cm-content")).toHaveAttribute("aria-readonly", "true");
  await expect(pane2.getByText("Not on disk yet.")).toHaveCount(0);
  await expect(pane2.locator(".text-status")).toContainText("Read-only");
});

// polish-git-plugin D4: text taken in place keeps the file's own line breaks.
test("a CRLF file's diff tab keeps its line breaks in both layouts", async ({ page }) => {
  const { pane } = await start(page);
  const root = "/Users/owner/shop-api";
  await page.evaluate(
    ({ pane, root, c }) => {
      (window as any).__fakeGit = { [pane]: { changes: { root, total: 1, changes: [c] } }, [root]: { base: { "win.txt": "one\r\ntwo\r\n" } } };
      (window as any).__terminescu.host.files.set(`${root}/win.txt`, "one\r\n2\r\n");
    },
    { pane, root, c: change("win.txt", { added: 1, removed: 1 }) },
  );
  await page.keyboard.press("Meta+Backslash");
  await page.locator('aside.ctx-pinned .git-change[data-path="win.txt"] .git-path').click();
  await expect(page.locator(".cm-mergeView .cm-editor")).toHaveCount(2);
  await expect(page.locator(".text-status")).toContainText("CRLF");
  await page.locator(".diff-mode").click();
  await expect(page.locator(".cm-mergeView")).toHaveCount(0);
  await expect(page.locator(".text-status")).toContainText("CRLF");
  await page.locator(".text-pane .cm-line").first().click();
  await page.keyboard.press("End");
  await page.keyboard.type("!");
  await page.keyboard.press("Meta+KeyS");
  await expect.poll(async () => (await files(page))[`${root}/win.txt`]).toBe("one!\r\n2\r\n");
  await page.locator(".diff-mode").click();
});

// polish-git-plugin D3, D4: a new base for this repository goes into the open editor in place.
test("a diff tab keeps its scroll place when HEAD moves, and ignores other repositories", async ({ page }) => {
  const { pane } = await start(page);
  const root = "/Users/owner/shop-api";
  const lines = (tag: string) => Array.from({ length: 300 }, (_, i) => `line ${i} ${i === 120 ? tag : ""}`).join("\n") + "\n";
  await page.evaluate(
    ({ pane, root, base, work, retry }) => {
      (window as any).__fakeGit = {
        [pane]: { changes: { root, total: 1, changes: [retry] } },
        [root]: { base: { "src/long.ts": base } },
      };
      (window as any).__terminescu.host.files.set(`${root}/src/long.ts`, work);
    },
    { pane, root, base: lines("base"), work: lines("work"), retry: change("src/long.ts", { added: 1, removed: 1 }) },
  );
  await page.keyboard.press("Meta+Backslash");
  await page.locator('aside.ctx-pinned .git-change[data-path="src/long.ts"] .git-path').click();
  const merge = page.locator(".cm-mergeView");
  await expect(merge.locator(".cm-editor")).toHaveCount(2);
  await merge.evaluate((el) => (el.scrollTop = 2000));
  const top = () => merge.evaluate((el) => el.scrollTop);
  await expect.poll(top).toBeGreaterThan(1500);
  const before = await top();
  const setBase = (tag: string) =>
    page.evaluate(({ root, base }) => ((window as any).__fakeGit[root].base["src/long.ts"] = base), { root, base: lines(tag) });
  // Another repository's facts leave the tab alone.
  await setBase("other");
  await host(page, "pluginEmit", "git", "facts", { pane: "elsewhere", root: "/Users/owner/other" });
  await page.waitForTimeout(200);
  await expect(merge.locator(".cm-editor").nth(0)).not.toContainText("line 120 other");
  // This repository's facts bring the new base, in place.
  await setBase("commit");
  await host(page, "pluginEmit", "git", "facts", { pane, root });
  await expect(merge.locator(".cm-editor").nth(0)).toContainText("line 120 commit");
  expect(Math.abs((await top()) - before)).toBeLessThan(5);
  // A layout switch and back puts the scroll back.
  await page.locator(".diff-mode").click();
  await expect(merge).toHaveCount(0);
  await page.locator(".diff-mode").click();
  await expect(merge.locator(".cm-editor")).toHaveCount(2);
  await expect.poll(top).toBeGreaterThan(before - 50);
});

const titles = async (page: Page) =>
  (await page.evaluate(() => (window as any).__terminescu.host.workspace())).windows[0].sideTabs.map((s: { title: string | null }) => s.title);

const done = (page: Page) => page.evaluate(() => (window as any).__fakeGitDone);

test("a worktree row opens below the selected one", async ({ page }) => {
  const { pane, side } = await start(page);
  await page.evaluate((pane) => ((window as any).__fakeGit = { [pane]: { changes: { root: "/code/shop-api", total: 0, changes: [] }, worktrees: ["PROJ-491-tax"] } }), pane);
  // A second row, then back to the first: the worktree row goes between them.
  await page.getByRole("button", { name: "New terminal" }).click();
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(2);
  await page.locator(`.ao-st[data-side="${side}"]`).click();
  await page.getByRole("button", { name: "Choose side tab kind" }).click();
  await page.getByRole("menuitem", { name: /New Row from Worktree/ }).click();
  const dialog = page.getByRole("alertdialog", { name: "New row from worktree" });
  await expect(dialog).toContainText("Goes to /code/shop-api-<branch>, from main");
  await dialog.getByRole("textbox", { name: "Branch" }).fill("PROJ-490-refunds");
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
  await expect.poll(() => titles(page)).toEqual([null, "PROJ-490-refunds", null]);
  expect((await done(page)).added).toEqual([{ branch: "PROJ-490-refunds", base: null }]);
  // ⌥⇧⌘N opens the worktrees no row shows, below the selected row.
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Alt+Shift+KeyN");
  await expect.poll(() => titles(page)).toEqual([null, "PROJ-491-tax", "PROJ-490-refunds", null]);
  // Escape asks nothing more and makes nothing.
  await page.keyboard.press("Meta+Alt+KeyN");
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  expect((await done(page)).added).toHaveLength(1);
});

test("closing a worktree row with changes asks twice", async ({ page }) => {
  const { pane } = await start(page);
  await page.evaluate((pane) => ((window as any).__fakeGit = { [pane]: { changes: { root: "/code/shop-api", total: 0, changes: [] } } }), pane);
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Alt+KeyN");
  await page.getByRole("textbox", { name: "Branch" }).fill("PROJ-490-refunds");
  await page.keyboard.press("Enter");
  await expect.poll(() => titles(page)).toEqual([null, "PROJ-490-refunds"]);
  const row = Object.keys((await done(page)).marks)[0]!;
  await page.evaluate((row) => ((window as any).__fakeGit[row] = { changed: 2 }), row);

  const close = async () => {
    await page.locator(`.ao-st[data-side="${row}"]`).click({ button: "right" });
    await page.getByRole("dialog", { name: "Edit" }).getByRole("button", { name: "Close side tab…" }).click();
    await page.getByRole("alertdialog", { name: "Close side tab" }).getByRole("button", { name: "Close side tab" }).click();
  };
  const first = page.getByRole("alertdialog", { name: "Close worktree row" });
  const second = page.getByRole("alertdialog", { name: "Remove worktree with changes" });

  // Remove, then Cancel on the second question: the row, the worktree and the branch stay.
  await close();
  await expect(first).toContainText("/code/shop-api-PROJ-490-refunds");
  await first.getByRole("button", { name: "Remove worktree" }).click();
  await expect(second).toContainText("2 changed files in the worktree would be lost, and so would ignored files such as .env.");
  await second.getByRole("button", { name: "Cancel" }).click();
  await expect(second).toBeHidden();
  expect(await titles(page)).toEqual([null, "PROJ-490-refunds"]);
  expect((await done(page)).removed).toEqual([]);

  // Remove anyway: the worktree goes, then the row.
  await close();
  await first.getByRole("button", { name: "Remove worktree" }).click();
  await second.getByRole("button", { name: "Remove anyway" }).click();
  await expect.poll(() => titles(page)).toEqual([null]);
  expect((await done(page)).removed).toEqual([{ row, force: true }]);
});

test("closing a worktree row keeps the worktree by default", async ({ page }) => {
  const { pane } = await start(page);
  await page.evaluate((pane) => ((window as any).__fakeGit = { [pane]: { changes: { root: "/code/shop-api", total: 0, changes: [] } } }), pane);
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Alt+KeyN");
  // The dialog names the folder the worktree goes to and the base (polish-git-plugin D8).
  await expect(page.getByRole("alertdialog", { name: "New row from worktree" })).toContainText("Goes to /code/shop-api-<branch>, from main");
  await page.getByRole("textbox", { name: "Branch" }).fill("PROJ-490-refunds");
  await page.keyboard.press("Enter");
  await expect.poll(() => titles(page)).toEqual([null, "PROJ-490-refunds"]);
  const row = Object.keys((await done(page)).marks)[0]!;
  await page.locator(`.ao-st[data-side="${row}"]`).click({ button: "right" });
  await page.getByRole("dialog", { name: "Edit" }).getByRole("button", { name: "Close side tab…" }).click();
  await page.getByRole("alertdialog", { name: "Close side tab" }).getByRole("button", { name: "Close side tab" }).click();
  await expect(page.getByRole("alertdialog", { name: "Close worktree row" })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect.poll(() => titles(page)).toEqual([null]);
  expect((await done(page)).removed).toEqual([]);
  // The close asked already, so its event raises no notice.
  const win = await page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].id);
  await host(page, "pluginEmit", "git", "orphan", { row, window: win, worktree: "/code/shop-api-PROJ-490-refunds", repo: "/code/shop-api", branch: "PROJ-490-refunds" });
  await page.waitForTimeout(200);
  await expect(page.getByRole("alertdialog", { name: "Worktree kept" })).toHaveCount(0);
});

// polish-git-plugin D6: a row that closed without asking, as on a shell exit.
test("a worktree row closed without asking offers to remove its worktree", async ({ page }) => {
  await start(page);
  const win = await page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].id);
  const orphan = { row: "gone-row", window: win, worktree: "/code/shop-api-PROJ-7-tax", repo: "/code/shop-api", branch: "PROJ-7-tax" };
  // Another window's row asks there, not here.
  await host(page, "pluginEmit", "git", "orphan", { ...orphan, window: "another" });
  await page.waitForTimeout(200);
  await expect(page.getByRole("alertdialog", { name: "Worktree kept" })).toHaveCount(0);
  await host(page, "pluginEmit", "git", "orphan", orphan);
  const notice = page.getByRole("alertdialog", { name: "Worktree kept" });
  await expect(notice).toContainText("Worktree kept at /code/shop-api-PROJ-7-tax. Remove it?");
  await notice.getByRole("button", { name: "Remove worktree" }).click();
  await expect.poll(async () => (await done(page)).removedAt).toEqual([{ worktree: "/code/shop-api-PROJ-7-tax", force: false }]);
});

const prs = [
  { number: 918, title: "Retry failed payments", url: "https://github.com/acme/shop-api/pull/918", branch: "feature/PROJ-482-retry", checks: "failing" },
  { number: 921, title: "Refund partial orders", url: "https://github.com/acme/shop-api/pull/921", branch: "PROJ-490-refunds", checks: "passing" },
];

const actionIds = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.pluginActions().map((a: { id: string }) => a.id));

test("without gh no pull request item shows in the menus, the palette or the + menu", async ({ page }) => {
  await start(page);
  await expect.poll(() => actionIds(page)).toContain("git.newWorktreeRow");
  expect(await actionIds(page)).not.toContain("git.openPullRequest");
  expect(await actionIds(page)).not.toContain("git.checkOutPullRequest");
  await page.getByRole("button", { name: "Choose side tab kind" }).click();
  await expect(page.getByRole("menuitem", { name: /New Row from Worktree/ })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /Pull Request/ })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+KeyP");
  await expect(page.locator(".palette")).toBeVisible();
  await page.keyboard.type("row");
  await expect(page.locator(".palette-item", { hasText: "New Row from Worktree" })).toBeVisible();
  await expect(page.locator(".palette-item", { hasText: "Pull Request" })).toHaveCount(0);
});

test("with gh the pull request items open the branch's pull request and check one out as a row", async ({ page }) => {
  await page.addInitScript((prs) => ((window as any).__fakeGitHub = { prs }), prs);
  const { pane } = await start(page);
  await page.evaluate(
    ({ pane, pr }) => ((window as any).__fakeGit = { [pane]: { changes: { root: "/code/shop-api", total: 0, changes: [] }, pr } }),
    { pane, pr: prs[0] },
  );
  await expect.poll(() => actionIds(page)).toContain("git.openPullRequest");
  // ⌥⌘G opens the branch's pull request in the browser.
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Alt+KeyG");
  await expect.poll(() => page.evaluate(() => (window as any).__terminescu.host.openedUrls)).toContain(prs[0]!.url);
  // ⌥⇧⌘G lists the open pull requests, newest first; the picked one opens as a row.
  await page.keyboard.press("Meta+Alt+Shift+KeyG");
  const dialog = page.getByRole("alertdialog", { name: "Check out pull request" });
  const items = dialog.getByRole("radio");
  await expect(items).toHaveCount(2);
  await expect(items.first()).toContainText("#921 Refund partial orders");
  await expect(items.first()).toHaveAttribute("aria-checked", "true");
  await page.keyboard.press("ArrowDown");
  await expect(items.nth(1)).toHaveAttribute("aria-checked", "true");
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
  await expect.poll(() => titles(page)).toEqual([null, "#918 feature/PROJ-482-retry"]);
  expect((await done(page)).checkedOut).toEqual([918]);
  // gh logs out: the items go.
  await page.evaluate(() => (window as any).__terminescu.host.pluginEmit("git", "github", { on: false }));
  await expect.poll(() => actionIds(page)).not.toContain("git.openPullRequest");
  expect(await actionIds(page)).not.toContain("git.checkOutPullRequest");
});

test("closing a worktree row's last tab asks too; Escape keeps it, a locked worktree keeps it", async ({ page }) => {
  const { pane } = await start(page);
  await page.evaluate((pane) => ((window as any).__fakeGit = { [pane]: { changes: { root: "/code/shop-api", total: 0, changes: [] } } }), pane);
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Alt+KeyN");
  await page.getByRole("textbox", { name: "Branch" }).fill("PROJ-490-refunds");
  await page.keyboard.press("Enter");
  await expect.poll(() => titles(page)).toEqual([null, "PROJ-490-refunds"]);
  const row = Object.keys((await done(page)).marks)[0]!;
  const first = page.getByRole("alertdialog", { name: "Close worktree row" });

  // ⌘W on the row's only tab ends the row, so the row's question comes first.
  await page.locator(`.ao-st[data-side="${row}"]`).click();
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+KeyW");
  await expect(first).toBeVisible();
  // A second ⌘W while it asks does not ask again.
  await page.keyboard.press("Meta+KeyW");
  await expect(first).toHaveCount(1);
  await page.keyboard.press("Escape");
  await expect(first).toBeHidden();
  expect(await titles(page)).toEqual([null, "PROJ-490-refunds"]);

  // Remove anyway on a worktree git cannot remove: the row stays and says why.
  await page.evaluate((row) => ((window as any).__fakeGit[row] = { changed: 1, stuck: true }), row);
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+KeyW");
  await first.getByRole("button", { name: "Remove worktree" }).click();
  await page.getByRole("alertdialog", { name: "Remove worktree with changes" }).getByRole("button", { name: "Remove anyway" }).click();
  await expect(page.getByRole("alert")).toContainText("git could not remove the worktree");
  expect(await titles(page)).toEqual([null, "PROJ-490-refunds"]);
});
