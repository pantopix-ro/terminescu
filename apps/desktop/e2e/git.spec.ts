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
  await expect(page.locator(".git-diff-note")).toHaveText("Changes since HEAD.");
  await expect(page.locator(".cm-mergeView .cm-changedLine").first()).toBeVisible();
  await expect(sides.nth(0).locator(".cm-content")).toHaveAttribute("contenteditable", "false");
  // The working side is the text tab's own document: typing and ⌘S write the file.
  await sides.nth(1).locator(".cm-line").first().click();
  await page.keyboard.press("Meta+ArrowUp");
  await page.keyboard.type("// retry on timeout\n");
  await page.keyboard.press("Meta+KeyS");
  await expect.poll(async () => (await files(page))[`${root}/src/retry.ts`]).toBe("// retry on timeout\nconst tries = 5;\nexport { tries };\n");
  // Unified: one editor, the removed line above the one that replaced it.
  await page.locator(".git-diff-mode").click();
  await expect(page.locator(".cm-mergeView")).toHaveCount(0);
  await expect(page.locator(".text-pane .cm-deletedChunk")).toContainText("const tries = 3;");
  await expect(page.locator(".text-pane .cm-content")).toContainText("// retry on timeout");
  // ⌥⌘U switches back.
  await page.locator(".text-pane .cm-content").click();
  await page.keyboard.press("Meta+Alt+KeyU");
  await expect(page.locator(".cm-mergeView .cm-editor")).toHaveCount(2);
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
  await expect(page.locator(".git-diff-note")).toHaveText("New file: not in HEAD.");
  await expect(page.locator(".cm-mergeView .cm-editor").nth(1)).toContainText("export const added = true;");
});
