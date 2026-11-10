import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

// add-claude-insights on the mock host: the Claude half's decorations and calls are played as
// its Rust half would set and answer them; crates/plugin-claude/tests/insights.rs covers how it
// reads them from a transcript.

/** What crates/plugin-claude answers, as tests/contract.rs records it (polish-claude-insights D8). */
const contract = (name: string) =>
  JSON.parse(readFileSync(new URL(`../../../packages/plugin-claude/contract/${name}.json`, import.meta.url), "utf8"));

const host = (page: Page, method: string, ...args: unknown[]) =>
  page.evaluate(({ method, args }) => (window as any).__terminescu.host[method](...args), { method, args });

const first = (page: Page) =>
  page.evaluate(() => {
    const t = (window as any).__terminescu.host.workspace().windows[0].sideTabs[0].tabs[0];
    return { tab: t.id as string, pane: t.layout.id as string };
  });

const meter = (tab: string, text: string, tone: string, tip: string) => ({
  plugin: "claude",
  target: { kind: "tab", id: tab },
  slot: "meter",
  key: "context",
  text,
  tone,
  tip,
});

test("the meter turns amber at 80%", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  const { tab } = await first(page);
  await host(page, "decorate", meter(tab, "62%", "neutral", "124k of 200k tokens · claude-opus-5-5"));
  const item = page.locator(`.ao-tab[data-tab="${tab}"] .pl-meter[data-key="context"]`);
  await expect(item).toHaveText("62%");
  await expect(item).not.toHaveClass(/pl-warn/);
  await host(page, "decorate", meter(tab, "86%", "warn", "172k of 200k tokens · claude-opus-5-5"));
  await expect(item).toHaveText("86%");
  await expect(item).toHaveClass(/pl-warn/);
  await expect(item).toHaveAttribute("title", "172k of 200k tokens · claude-opus-5-5");
  // A one-tab row shows its tab's meter too.
  await expect(page.locator(`.ao-st[data-side] .pl-meter[data-key="context"]`).first()).toHaveText("86%");
});

test("the tab shows the model and mode chips, compaction and subagents", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  const { tab } = await first(page);
  const d = (slot: string, key: string, text: string) => ({ plugin: "claude", target: { kind: "tab", id: tab }, slot, key, text, tone: "neutral" });
  await host(page, "decorate", d("chip", "model", "opus-5-5"));
  await host(page, "decorate", d("chip", "mode", "plan"));
  await host(page, "decorate", d("subtitle", "compacting", "compacting"));
  await host(page, "decorate", d("badge", "subagents", "2 subagents"));
  const t = page.locator(`.ao-tab[data-tab="${tab}"]`);
  await expect(t.locator('.pl-chip[data-key="model"]')).toHaveText("opus-5-5");
  await expect(t.locator('.pl-chip[data-key="mode"]')).toHaveText("plan");
  await expect(t.locator('.pl-subtitle[data-key="compacting"]')).toHaveText("compacting");
  await expect(t.locator('.pl-badge[data-key="subagents"]')).toHaveText("2 subagents");
});

test("the todo list shows Claude's plan", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?plugins=claude");
  const { pane } = await first(page);
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Backslash");
  const panel = page.locator('aside.ctx-pinned section[data-section="claude.todos"]');
  // A tab with no Claude session shows no plan section (polish-claude-insights D2).
  await expect(panel).toBeHidden();
  const section = panel.locator(".cl-todos");
  const some = contract("todos").some as { session: string; todos: { state: string }[] };
  const todos = (states: string[]) => ({ ...some, todos: some.todos.map((t, i) => ({ ...t, state: states[i] })) });
  await page.evaluate(({ pane, a }) => ((window as any).__fakeClaude = { [pane]: { todos: a } }), { pane, a: some });
  await host(page, "pluginEmit", "claude", "insights", { pane });
  await expect(panel.locator("h3")).toHaveText("Claude's plan");
  await expect(section.locator(".cl-todo")).toHaveCount(4);
  await expect(section.locator(".cl-todo").first()).toHaveAttribute("data-state", "inProgress");
  await expect(section.locator(".cl-todo").first()).toHaveText(/Reading src\/retry\.ts/);
  await page.evaluate(({ pane, a }) => ((window as any).__fakeClaude = { [pane]: { todos: a } }), { pane, a: todos(["completed", "inProgress", "pending", "pending"]) });
  await host(page, "pluginEmit", "claude", "insights", { pane });
  await expect(section.locator(".cl-todo").first()).toHaveAttribute("data-state", "completed");
  await expect(section.locator(".cl-todo").nth(1)).toHaveAttribute("data-state", "inProgress");
  // The session ends: the section goes again.
  await page.evaluate(({ pane, a }) => ((window as any).__fakeClaude = { [pane]: { todos: a } }), { pane, a: contract("todos").none });
  await host(page, "pluginEmit", "claude", "insights", { pane });
  await expect(panel).toBeHidden();
});

test("resume types the command and no Enter", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?plugins=claude");
  const row = await page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].sideTabs[0].id as string);
  const now = Math.floor(Date.now() / 1000);
  await page.evaluate(
    ({ row, now }) =>
      ((window as any).__fakeClaude = {
        [row]: {
          history: [
            { id: "sess-new", cwd: "/Users/owner/src/shop-api", started: now - 60, prompt: "Add backoff to the retry" },
            { id: "sess-old", cwd: "/Users/owner/src/shop-api/web", started: now - 86400, ended: now - 80000, prompt: "Fix PROJ-482" },
          ],
        },
      }),
    { row, now },
  );
  // From the row's edit dialog: the sessions newest first, then a new tab with the line typed.
  await page.locator(`.ao-st[data-side="${row}"]`).click({ button: "right" });
  await page.locator(".edit-actions").getByRole("button", { name: /Resume…/ }).click();
  const dialog = page.getByRole("alertdialog", { name: "Resume a Claude session" });
  await expect(dialog).toBeVisible();
  const items = dialog.getByRole("radio");
  await expect(items).toHaveCount(2);
  await expect(items.first()).toContainText("Add backoff to the retry");
  await items.nth(1).click();
  await dialog.getByRole("button", { name: "Resume", exact: true }).click();
  await expect(dialog).toBeHidden();
  const tabs = await page.evaluate((row) => (window as any).__terminescu.host.workspace().windows[0].sideTabs.find((s: any) => s.id === row).tabs, row);
  expect(tabs).toHaveLength(2);
  const pane = tabs[1].layout.id;
  await expect
    .poll(() => page.evaluate((pane) => (window as any).__terminescu.host.typed.get(pane), pane))
    .toBe("claude --resume sess-old");
  // The palette has it too, with its ⌘ chord.
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+KeyP");
  await expect(page.locator(".palette")).toBeVisible();
  await page.keyboard.type("resume");
  await expect(page.locator(".palette-item", { hasText: "Resume…" })).toContainText("⌥⌘R");
});

test("a missing CLAUDE.md opens unsaved", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?plugins=claude&top-tabs");
  const row = await page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].sideTabs[0].id as string);
  await page.evaluate((row) => ((window as any).__fakeClaude = { [row]: { root: "/Users/owner/src/shop-api" } }), row);
  const files = () => page.evaluate(() => Object.fromEntries((window as any).__terminescu.host.files)) as Promise<Record<string, string>>;
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Alt+KeyC");
  const editor = page.locator(".text-pane .cm-content");
  await expect(editor).toBeVisible();
  await expect(page.locator(".app-main .ao-tab.active")).toContainText("CLAUDE.md");
  await expect(editor).toHaveText("");
  expect((await files())["/Users/owner/src/shop-api/CLAUDE.md"]).toBeUndefined();
  await editor.click();
  await page.keyboard.type("# shop-api");
  expect((await files())["/Users/owner/src/shop-api/CLAUDE.md"], "nothing is written before ⌘S").toBeUndefined();
  await page.keyboard.press("Meta+KeyS");
  await expect.poll(async () => (await files())["/Users/owner/src/shop-api/CLAUDE.md"]).toBe("# shop-api");
  expect(await page.evaluate(() => (window as any).__terminescu.host.dialogs), "saved at its path, no dialog").toEqual([]);
  // The user file and the transcript folder are in the palette with their chords.
  await page.keyboard.press("Meta+KeyP");
  await page.keyboard.type("CLAUDE.md");
  await expect(page.locator(".palette-item", { hasText: "Open User CLAUDE.md" })).toContainText("⌥⇧⌘C");
  await page.keyboard.press("Escape");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Alt+Shift+KeyR");
  await expect(page.locator(".ao-error, [role=alert]").first()).toContainText("no transcript yet");
});

test("a session's changes open as diffs", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?plugins=claude");
  const { pane } = await first(page);
  const { changes, baseText } = contract("changes");
  const root: string = changes.base;
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Backslash");
  const section = page.locator('aside.ctx-pinned section[data-section="claude.changes"]');
  await expect(section).toBeHidden();
  const large = { path: `${root}/data/dump.sql`, added: null, removed: null, created: false, deleted: false, none: "too large" };
  await page.evaluate(
    ({ pane, root, changes, base }) => {
      (window as any).__fakeClaude = { [pane]: { changes } };
      (window as any).__fakeClaudeBases = { [`${root}/src/retry.ts`]: base };
      (window as any).__terminescu.host.files.set(`${root}/src/retry.ts`, "const tries = 5;\nexport {};\n");
    },
    { pane, root, changes: { ...changes, files: [...changes.files, large] }, base: baseText.changed },
  );
  await host(page, "pluginEmit", "claude", "changed", { pane });
  await expect(section.locator("h3")).toHaveText("What Claude changed");
  const rows = section.locator(".cl-change");
  await expect(rows).toHaveCount(4);
  await expect(section.locator(".cl-untracked")).toHaveCount(0);
  const retry = section.locator(`.cl-change[data-path="${root}/src/retry.ts"]`);
  await expect(retry.locator(".cl-name")).toHaveText("retry.ts");
  await expect(retry.locator(".cl-dir")).toHaveText("src/");
  await expect(retry.locator(".cl-lines")).toHaveText("+2 −1");
  await expect(section.locator(`.cl-change[data-path="${root}/src/backoff.ts"] .cl-change-mark`)).toHaveText("A");
  await expect(section.locator(`.cl-change[data-path="${root}/src/old.ts"] .cl-change-mark`)).toHaveText("D");
  await expect(section.locator(`.cl-change[data-path="${root}/data/dump.sql"]`)).toContainText("Too large to compare");
  // The diff: the file before the session on the left, the working copy on the right.
  await retry.locator(".cl-path").click();
  const sides = page.locator(".cm-mergeView .cm-editor");
  await expect(sides).toHaveCount(2);
  await expect(sides.nth(0)).toContainText("const tries = 3;");
  await expect(sides.nth(1)).toContainText("const tries = 5;");
  await expect(page.locator(".diff-note")).toHaveText("Changes since the session started.");
  // The layout chord is the app's: it works with the git plugin off.
  await expect(page.locator(".diff-mode")).toHaveAttribute("title", "Switch the diff layout (⌥⌘U)");
  await sides.nth(1).locator(".cm-content").click();
  await page.keyboard.press("Meta+Alt+KeyU");
  await expect(page.locator(".cm-mergeView")).toHaveCount(0);
  await page.keyboard.press("Meta+Alt+KeyU");
  await expect(sides).toHaveCount(2);
  // Outside git the list says what it cannot see.
  await page.evaluate(({ pane, root }) => ((window as any).__fakeClaude[pane].changes = { session: "sess-1", tree: false, base: root, files: [] }), { pane, root });
  await host(page, "pluginEmit", "claude", "changed", { pane });
  await expect(section.locator(".cl-untracked")).toContainText("Bash edits are not tracked");
});
