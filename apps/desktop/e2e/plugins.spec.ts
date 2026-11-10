import { expect, test, type Page } from "@playwright/test";

// add-plugin-architecture 2.3 on the mock host: a fake plugin fills every slot (chips, a panel
// section, actions with a chord clash, a pane view) and the app shows each only while it is on.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?plugins=fake");
  await expect(page.locator(".term-pane")).toHaveCount(1);
  return page.evaluate(() => {
    const ws = (window as any).__terminescu.host.workspace();
    const side = ws.windows[0].sideTabs[0];
    return { side: side.id as string, tab: side.tabs[0].id as string, window: ws.windows[0].id as string };
  });
}

const host = (page: Page, run: string, ...args: unknown[]) =>
  page.evaluate(({ run, args }) => (window as any).__terminescu.host[run](...args), { run, args });
const field = (page: Page, name: string) => page.evaluate((n) => (window as any).__terminescu.host[n], name);

const deco = (plugin: string, kind: string, id: string, key: string, text: string) => ({ plugin, target: { kind, id }, slot: "chip", key, text, tone: "neutral" });

test("a plugin's chips show on its row and tab, and go when it is switched off", async ({ page }) => {
  const { side, tab } = await start(page);
  await host(page, "decorate", deco("fake", "sideTab", side, "row", "row-chip"));
  await host(page, "decorate", deco("fake", "tab", tab, "tab", "tab-chip"));
  // A row of one tab shows the tab's chips too, in the order they came.
  await expect(page.locator(`.app-side [data-plugin="fake"]`)).toHaveText(["row-chip", "tab-chip"]);
  // Same plugin, target, slot and key: the chip changes in place.
  await host(page, "decorate", deco("fake", "sideTab", side, "row", "row-2"));
  await expect(page.locator(`.app-side [data-key="row"]`)).toHaveText("row-2");
  await host(page, "setPluginEnabled", "fake", false);
  await expect(page.locator(".pl-item")).toHaveCount(0);
  // A stopped plugin cannot decorate.
  await host(page, "decorate", deco("fake", "sideTab", side, "row", "again"));
  await expect(page.locator(".pl-item")).toHaveCount(0);
});

test("a plugin's panel section calls its Rust half", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Backslash");
  const section = page.locator('aside.ctx-pinned section[data-section="fake.notes"]');
  await expect(section.locator("h3")).toHaveText("Fake Notes");
  await section.getByRole("button", { name: "Ping" }).click();
  await expect(section.locator(".fake-answer")).toHaveText("pong");
  expect(await field(page, "pluginCalls")).toContain("fake.ping");
  await host(page, "setPluginEnabled", "fake", false);
  await expect(section).toHaveCount(0);
});

test("plugin actions reach the palette and the menu; a clashing chord loses", async ({ page }) => {
  const { side } = await start(page);
  // Plugins register their actions after the first pane shows; a slow machine takes a moment.
  const actions = () => page.evaluate(() => (window as any).__terminescu.host.pluginActions()) as Promise<{ id: string; accel: string; menu: string }[]>;
  await expect
    .poll(actions)
    .toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: "fake.hello", accel: "Cmd+Alt+J", menu: "Plugins" }),
        expect.objectContaining({ id: "fake.steal", accel: "" }),
      ]),
    );
  await page.locator(".term-pane").first().click();
  // ⌘⌥J runs the plugin's action: it calls its Rust half, which decorates the row.
  await page.keyboard.press("Meta+Alt+J");
  await expect(page.locator(`.app-side [data-key="hello"]`)).toHaveText("hi");
  // ⌘T keeps opening a terminal: beside a row of one tab, a new row.
  await page.keyboard.press("Meta+T");
  await expect.poll(() => page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].sideTabs.length)).toBe(2);
  await page.keyboard.press("Meta+P");
  const steal = page.locator(".palette-item", { hasText: "Fake New Tab" });
  await expect(steal).toBeVisible();
  await expect(steal.locator(".ao-dim")).toHaveCount(0);
  await expect(page.locator(".palette-item", { hasText: "Fake Hello" }).locator(".ao-dim")).toHaveText("⌥⌘J");
  await page.keyboard.press("Escape");
  await host(page, "setPluginEnabled", "fake", false);
  await expect.poll(() => page.evaluate(() => (window as any).__terminescu.host.pluginActions().filter((a: { id: string }) => a.id.startsWith("fake.")).length)).toBe(0);
});

test("a pane view without its plugin keeps its place and names the plugin", async ({ page }) => {
  await start(page);
  // The Claude plugin draws transcripts first; off, the fake viewer takes them.
  await host(page, "setPluginEnabled", "claude", false);
  await page.locator(".term-pane").first().click();
  const pane = (await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane"))!;
  await page.evaluate((p) => (window as any).__terminescu.host.setAgent(p, { session: "sess-plug", state: "done", lastMessage: null }), pane);
  await page.keyboard.press("Meta+Shift+L");
  await expect(page.locator(".fake-view")).toBeVisible();
  await expect(page.locator(".tr-pane")).toHaveCount(0);
  const widths = () => page.locator(".app-pane .ao-tp").evaluateAll((els) => els.map((e) => Math.round(e.getBoundingClientRect().width)));
  const before = await widths();
  await host(page, "setPluginEnabled", "viewer", false);
  const notice = page.locator(".plugin-missing");
  // The first plugin that draws the kind is named.
  await expect(notice).toContainText("Needs the Claude plugin");
  expect(await widths()).toEqual(before);
  await notice.getByRole("button", { name: "Open Settings" }).click();
  await expect(page.getByRole("dialog").locator("#ss-t")).toHaveText("Settings › Plugins");
});

test("a transcript pane reads through the Claude plugin", async ({ page }) => {
  await start(page);
  await page.locator(".term-pane").first().click();
  const pane = (await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane"))!;
  await page.evaluate((p) => {
    const h = (window as any).__terminescu.host;
    h.appendTranscript("sess-read", [{ id: "u1", offset: 0, timestamp: null, kind: "user", text: "hello there" }]);
    h.setAgent(p, { session: "sess-read", state: "done", lastMessage: null });
  }, pane);
  await page.keyboard.press("Meta+Shift+L");
  await expect(page.locator(".tr-pane .tr-text").first()).toHaveText("hello there");
  expect(await field(page, "pluginCalls")).toContain("claude.read");
});

test("switching a plugin off clears what it shows", async ({ page }) => {
  const { side } = await start(page);
  await host(page, "decorate", deco("fake", "sideTab", side, "row", "row-chip"));
  await page.keyboard.press("Meta+Backslash");
  await expect(page.locator('section[data-section="fake.notes"]')).toBeVisible();
  await page.keyboard.press("Meta+Comma");
  const settings = page.getByRole("dialog");
  await settings.getByRole("button", { name: "Plugins", exact: true }).click();
  await expect(settings.locator(".fake-settings")).toBeVisible();
  await settings.getByLabel("Plugin Fake").uncheck();
  await expect(page.locator(".pl-item")).toHaveCount(0);
  await expect(page.locator('section[data-section="fake.notes"]')).toHaveCount(0);
  await expect(settings.locator(".fake-settings")).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).__terminescu.host.pluginActions().filter((a: { id: string }) => a.id.startsWith("fake.")).length)).toBe(0);
  // Back on, it starts clean: what it showed went with it.
  await settings.getByLabel("Plugin Fake").check();
  await expect(page.locator('section[data-section="fake.notes"]')).toBeVisible();
  await expect(page.locator(".pl-item")).toHaveCount(0);
});

test("a plugin stopped after errors says so until switched off and on", async ({ page }) => {
  await start(page);
  await host(page, "pausePlugin", "fake");
  await page.keyboard.press("Meta+Comma");
  const settings = page.getByRole("dialog");
  await settings.getByRole("button", { name: "Plugins", exact: true }).click();
  const row = settings.locator('.plugin-set[data-plugin="fake"]');
  await expect(row.getByRole("status")).toHaveText(/stopped after errors/);
  await expect(page.locator('section[data-section="fake.notes"]')).toHaveCount(0);
  await settings.getByLabel("Plugin Fake").uncheck();
  await settings.getByLabel("Plugin Fake").check();
  await expect(row.getByRole("status")).toHaveCount(0);
});

test("the row selection reaches plugins and a plugin variable shows read-only", async ({ page }) => {
  const { side, window } = await start(page);
  await expect.poll(() => field(page, "rowsSelected")).toContain(`${window} ${side}`);
  await host(page, "provideVariable", "fake", side, "branch", "main");
  await page.keyboard.press("Meta+Backslash");
  const row = page.locator('aside.ctx-pinned .ctx-var[data-var="branch"]');
  await expect(row).toHaveAttribute("data-provided", "");
  await expect(row.locator(".ctx-meta")).toHaveText("from fake · read-only");
  await expect(row.getByRole("button", { name: "Remove branch" })).toHaveCount(0);
  await expect(row.getByLabel("Storage of branch")).toHaveCount(0);
  await host(page, "setPluginEnabled", "fake", false);
  await expect(row).toHaveCount(0);
});

test("Settings › Statuses offers a rule on a plugin fact", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Comma");
  const settings = page.getByRole("dialog");
  await settings.getByLabel("New global status").fill("Dirty");
  await settings.getByRole("button", { name: "Add", exact: true }).click();
  await settings.getByLabel("Rule kind").selectOption("fact");
  await settings.getByLabel("Fact name").fill("not a fact");
  await settings.getByRole("button", { name: "Add rule" }).click();
  await expect(page.getByText(/no fact name/)).toBeVisible();
  await settings.getByLabel("Fact name").fill("git.dirty");
  await settings.getByRole("button", { name: "Add rule" }).click();
  await expect(settings.getByText("git.dirty", { exact: true })).toBeVisible();
});
