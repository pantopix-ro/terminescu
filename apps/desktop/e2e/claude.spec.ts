import { expect, test, type Page } from "@playwright/test";

// add-claude-insights on the mock host: the Claude half's decorations and calls are played as
// its Rust half would set and answer them; crates/plugin-claude/tests/insights.rs covers how it
// reads them from a transcript.

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
  await expect(panel.locator("h3")).toHaveText("Claude's plan");
  await expect(panel.locator(".ctx-empty")).toHaveText("No Claude session in this tab.");
  const section = panel.locator(".cl-todos");
  const todos = (states: string[]) => ({
    session: "sess-1",
    todos: ["Read src/retry.ts", "Add exponential backoff", "Cover it with a test", "Run the tests"].map((content, i) => ({
      content,
      state: states[i],
      activeForm: i === 0 ? "Reading src/retry.ts" : undefined,
    })),
  });
  await page.evaluate(({ pane, a }) => ((window as any).__fakeClaude = { [pane]: { todos: a } }), { pane, a: todos(["inProgress", "pending", "pending", "pending"]) });
  await host(page, "pluginEmit", "claude", "insights", { pane });
  await expect(section.locator(".cl-todo")).toHaveCount(4);
  await expect(section.locator(".cl-todo").first()).toHaveAttribute("data-state", "inProgress");
  await expect(section.locator(".cl-todo").first()).toHaveText(/Reading src\/retry\.ts/);
  await page.evaluate(({ pane, a }) => ((window as any).__fakeClaude = { [pane]: { todos: a } }), { pane, a: todos(["completed", "inProgress", "pending", "pending"]) });
  await host(page, "pluginEmit", "claude", "insights", { pane });
  await expect(section.locator(".cl-todo").first()).toHaveAttribute("data-state", "completed");
  await expect(section.locator(".cl-todo").nth(1)).toHaveAttribute("data-state", "inProgress");
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
