import { expect, test, type Page } from "@playwright/test";

// Task 6.1: the context panel keeps variables in three modes, links and text, and pins from a
// terminal pane (D12, D21).

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".term-pane")).toHaveCount(1);
  await page.keyboard.press("Meta+Backslash");
  await expect(page.locator("aside.ctx-pinned")).toBeVisible();
}

test("rows and controls in the context panel take the amber line under the mouse (tasks 8.61, 8.65)", async ({ page }) => {
  await start(page);
  const panel = page.locator("aside.ctx-pinned");
  await panel.getByRole("button", { name: "Add a variable" }).click();
  for (const name of ["Auto-hide context", "Add"]) {
    const b = panel.getByRole("button", { name, exact: true }).first();
    await b.hover();
    await expect(b).toHaveCSS("outline-style", "solid");
  }
  const input = panel.locator("input.ao-input").first();
  await input.hover();
  await expect(input).toHaveCSS("outline-style", "solid");
  // A prompt row draws the line itself; the name button that fills it does not add a second.
  const row = panel.locator(".ctx-item").first();
  await row.locator(".link-plain").hover();
  expect(await row.evaluate((el) => getComputedStyle(el, "::before").boxShadow)).toContain("inset");
  await expect(row.locator(".link-plain")).toHaveCSS("outline-style", "none");
});

test("variables: add in each mode, mask a secret, refuse to unmask it", async ({ page }) => {
  await start(page);
  const panel = page.locator("aside.ctx-pinned");
  const add = async (name: string, value: string, mode: string, ttl?: string) => {
    await panel.getByRole("button", { name: "Add a variable" }).click();
    const form = panel.getByRole("form", { name: "Add a variable" });
    await form.getByLabel("Name").fill(name);
    await form.getByLabel("Storage").selectOption(mode);
    await form.getByLabel("Value").fill(value);
    if (ttl) await form.getByLabel("TTL").fill(ttl);
    await form.getByRole("button", { name: "Add" }).click();
    await expect(form).toHaveCount(0);
  };
  await add("ticket", "PROJ-482", "saved");
  await add("from_date", "2026-10-01", "session", "1h");
  await add("API_TOKEN", "tok-example", "secret");

  const row = (name: string) => panel.locator(`.ctx-var[data-var="${name}"]`);
  await expect(row("ticket").locator(".ctx-value")).toHaveText("PROJ-482");
  await expect(row("from_date")).toHaveAttribute("data-mode", "session");
  await expect(row("API_TOKEN").locator(".ctx-value")).not.toContainText("tok-example");
  await expect(page.locator("body")).not.toContainText("tok-example");

  // A saved value turns secret; a secret offers no other mode.
  await row("ticket").getByLabel("Storage of ticket").selectOption("secret");
  await expect(row("ticket")).toHaveAttribute("data-mode", "secret");
  await expect(row("ticket").locator(".ctx-value")).not.toContainText("PROJ-482");
  await expect(row("ticket").locator('option[value="saved"]')).toBeDisabled();

  // Edit a value in place.
  // The acts lie over the right end of the row, so click the value at its left edge.
  await row("from_date").locator(".ctx-value").click({ position: { x: 4, y: 4 } });
  await row("from_date").getByLabel("Value of from_date").fill("2026-11-01");
  await row("from_date").getByLabel("Value of from_date").press("Enter");
  await expect(row("from_date").locator(".ctx-value")).toHaveText("2026-11-01");

  await row("from_date").getByLabel("Remove from_date").click();
  await expect(row("from_date")).toHaveCount(0);
});

test("links are kept once; text is kept as typed", async ({ page }) => {
  await start(page);
  const panel = page.locator("aside.ctx-pinned");
  await panel.getByRole("button", { name: "Add a link or text" }).click();
  const form = page.getByRole("form", { name: "Add a link or text" });
  const field = form.getByLabel("Link or text");
  for (const v of ["https://github.com/acme/api/pull/918", "https://github.com/acme/api/pull/918", "api, web"]) {
    await field.fill(v);
    await field.press("Enter");
    // The box closes on submit; the head opens it again.
    await expect(form).toHaveCount(0);
    await panel.getByRole("button", { name: "Add a link or text" }).click();
  }
  const items = page.locator('.ctx-section[data-section="Links"] .ctx-item');
  await expect(items).toHaveCount(2);
  await expect(items.nth(0)).toHaveClass(/link/);
  await expect(items.nth(1)).toHaveText(/api, web/);
});

test("⌘⇧P pins a terminal selection and the pin selects its rows again", async ({ page }) => {
  await start(page);
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  await page.locator(".term-pane").first().click();
  await page.keyboard.type("echo audit");
  await page.keyboard.press("Enter");
  await page.evaluate((id) => {
    const term = (window as any).__terminescu.terminals.get(id);
    term.selectLines(0, 0);
  }, pane);
  await page.keyboard.press("Meta+Shift+P");
  const pin = page.locator('.ctx-pin[data-kind="terminal"]');
  await expect(pin).toHaveCount(1);
  await expect(pin.locator(".ctx-quote")).toContainText(`mock ${pane} $`);

  await page.evaluate((id) => (window as any).__terminescu.terminals.get(id).clearSelection(), pane);
  await pin.locator(".ctx-pin-open").click();
  await expect
    .poll(() => page.evaluate((id) => (window as any).__terminescu.terminals.get(id).getSelection(), pane))
    .toContain(`mock ${pane} $`);
});

test("⌘E on a terminal selection opens the extract dialog", async ({ page }) => {
  await start(page);
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  await page.locator(".term-pane").first().click();
  await page.evaluate((id) => (window as any).__terminescu.terminals.get(id).selectLines(0, 0), pane);
  await page.keyboard.press("Meta+E");
  const dialog = page.getByRole("dialog", { name: "Extract to variable" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel("Variable name")).toHaveValue("text");
  await dialog.getByRole("radio", { name: "Session only" }).click();
  await dialog.getByLabel("Variable name").press("Enter");
  await expect(page.locator('.ctx-var[data-var="text"]')).toHaveAttribute("data-mode", "session");
});

test("inserting a secret types only $NAME, and a Claude pane says where the value goes", async ({ page }) => {
  await start(page);
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  const panel = page.locator("aside.ctx-pinned");
  await panel.getByRole("button", { name: "Add a variable" }).click();
  const form = panel.getByRole("form", { name: "Add a variable" });
  await form.getByLabel("Name").fill("API_TOKEN");
  await form.getByLabel("Storage").selectOption("secret");
  await form.getByLabel("Value").fill("tok-example");
  await form.getByRole("button", { name: "Add" }).click();

  await page.locator(".term-pane").first().click();
  await page.evaluate(
    (pane) => (window as any).__terminescu.host.setAgent(pane, { session: "sess-0b5c", state: "running", lastMessage: null }),
    pane,
  );
  await panel.getByRole("button", { name: "Insert API_TOKEN" }).click();
  const screen = () =>
    page.evaluate((id) => {
      const b = (window as any).__terminescu.terminals.get(id).buffer.active;
      return Array.from({ length: b.length }, (_, i) => b.getLine(i)?.translateToString(true) ?? "").join("\n");
    }, pane);
  await expect.poll(screen).toContain("$API_TOKEN");
  expect(await screen()).not.toContain("tok-example");
  await expect(page.locator(".app-note")).toContainText("Claude sees $API_TOKEN");
});

// Task 8.27: the three sections stack in one scrolling panel.
test("empty side tab shows four heads and no sentence; Pins hides while empty", async ({ page }) => {
  await start(page);
  const panel = page.locator("aside.ctx-pinned");
  const heads = panel.locator(".ctx-sec-head").filter({ visible: true });
  await expect(heads.locator("h3")).toHaveText(["Variables", "Links", "Files", "Prompts"]);
  await expect(panel.locator('[data-section="Pins"]')).toBeHidden();
  for (const s of ["Variables", "Links", "Files"]) {
    await expect(panel.locator(`[data-section="${s}"] .ctx-sec-head .ctx-count`)).toHaveText("0");
    await expect(panel.locator(`[data-section="${s}"]`)).not.toContainText(/yet|show up/);
  }
  await expect(panel.locator('[data-section="Prompts"] .ctx-count')).toHaveText("2");
  // A + only where the section has an add action.
  for (const [s, plus] of [["Variables", 1], ["Links", 1], ["Files", 0], ["Pins", 0], ["Prompts", 1]] as const)
    await expect(panel.locator(`[data-section="${s}"] .ctx-add-btn`)).toHaveCount(plus);
  await expect(panel.locator('[data-section="Context"]')).toHaveCount(0);
});

test("add a link from the head", async ({ page }) => {
  await start(page);
  const panel = page.locator("aside.ctx-pinned");
  await expect(panel.getByRole("form", { name: "Add a link or text" })).toHaveCount(0);
  await panel.getByRole("button", { name: "Add a link or text" }).click();
  const field = panel.getByRole("textbox", { name: "Link or text" });
  await field.fill("https://example.test/a");
  await field.press("Enter");
  await expect(panel.getByRole("form", { name: "Add a link or text" })).toHaveCount(0);
  await expect(panel.locator('[data-section="Links"] .ctx-sec-head .ctx-count')).toHaveText("1");
  // Escape closes it without adding.
  await panel.getByRole("button", { name: "Add a link or text" }).click();
  await panel.getByRole("textbox", { name: "Link or text" }).press("Escape");
  await expect(panel.getByRole("form", { name: "Add a link or text" })).toHaveCount(0);
});

async function addVar(page: Page, name: string, value: string, mode: string, ttl?: string) {
  const panel = page.locator("aside.ctx-pinned");
  await panel.getByRole("button", { name: "Add a variable" }).click();
  const form = panel.getByRole("form", { name: "Add a variable" });
  await form.getByLabel("Name").fill(name);
  await form.getByLabel("Storage").selectOption(mode);
  await form.getByLabel("Value").fill(value);
  if (ttl) await form.getByLabel("TTL").fill(ttl);
  await form.getByRole("button", { name: "Add" }).click();
  await expect(form).toHaveCount(0);
}

test("variable details on hover", async ({ page }) => {
  await start(page);
  await addVar(page, "from_date", "2026-11-01", "session", "1h");
  const row = page.locator('aside.ctx-pinned .ctx-var[data-var="from_date"]');
  expect((await row.boundingBox())!.height).toBeLessThan(34);
  await row.locator(".ctx-name").hover();
  const tip = page.getByRole("tooltip");
  await expect(tip).toContainText("2026-11-01");
  await expect(tip).toContainText(/gone on reboot · (59m|1h) left/);
  await expect(tip).toContainText("from typed");
  expect((await row.boundingBox())!.height).toBeLessThan(34);
  await page.mouse.move(600, 600);
  await expect(tip).toHaveCount(0);
});

test("secret stays hidden", async ({ page }) => {
  await start(page);
  await addVar(page, "API_TOKEN", "tok-example", "secret");
  const row = page.locator('aside.ctx-pinned .ctx-var[data-var="API_TOKEN"]');
  await row.locator(".ctx-name").hover();
  const tip = page.getByRole("tooltip");
  await expect(tip).toContainText("inserts as $API_TOKEN");
  await expect(tip).not.toContainText("tok-example");
  await expect(page.locator("body")).not.toContainText("tok-example");
});

test("Variables to Prompts stack one under the other and the panel scrolls", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 420 });
  await page.goto("/");
  await expect(page.locator(".term-pane")).toHaveCount(1);
  await page.keyboard.press("Meta+Backslash");
  const panel = page.locator("aside.ctx-pinned");
  const heads = panel.locator(".ctx-sec-head");
  // The Claude plugin's sections follow the app's own (add-claude-insights 2.2, 3.2), hidden
  // while the tab has no Claude session (polish-claude-insights D2).
  await expect(heads.locator("h3")).toHaveText(["Variables", "Links", "Files", "Pins", "Prompts", "Claude's plan", "What Claude changed"]);
  await expect(heads.filter({ visible: true }).locator("h3")).toHaveText(["Variables", "Links", "Files", "Prompts"]);
  await expect(panel.getByRole("tab")).toHaveCount(0);
  const tops = await panel.locator(".ctx-section:not([hidden])").evaluateAll((els) => els.map((e) => e.getBoundingClientRect().top));
  for (let i = 1; i < 4; i++) expect(tops[i - 1]!).toBeLessThan(tops[i]!);

  const scroll = panel.locator(".ctx-scroll");
  for (let i = 0; i < 6; i++) {
    await panel.getByRole("button", { name: "Add a variable" }).click();
    const form = panel.getByRole("form", { name: "Add a variable" });
    await form.getByLabel("Name").fill(`v${i}`);
    await form.getByLabel("Value").fill("example");
    await form.getByRole("button", { name: "Add" }).click();
  }
  const overflow = await scroll.evaluate((e) => e.scrollHeight > e.clientHeight);
  expect(overflow).toBe(true);
  await scroll.evaluate((e) => (e.scrollTop = e.scrollHeight));
  await expect(panel.locator('.ctx-section[data-section="Prompts"]')).toBeInViewport();
  await expect(panel.locator(".ctx-head")).toBeInViewport();
});

test("a removed variable comes back with Undo, and goes once the toast does", async ({ page }) => {
  await page.clock.install();
  await start(page);
  const panel = page.locator("aside.ctx-pinned");
  for (const [name, mode] of [["ticket", "saved"], ["API_TOKEN", "secret"]] as const) {
    await panel.getByRole("button", { name: "Add a variable" }).click();
    const form = panel.getByRole("form", { name: "Add a variable" });
    await form.getByLabel("Name").fill(name);
    await form.getByLabel("Storage").selectOption(mode);
    await form.getByLabel("Value").fill("example");
    await form.getByRole("button", { name: "Add" }).click();
  }
  const row = (name: string) => panel.locator(`.ctx-var[data-var="${name}"]`);
  const ctxOf = () =>
    page.evaluate(async () => {
      const t = (window as any).__terminescu;
      const side = t.host.workspace().windows[0].sideTabs[0].id;
      return (await t.host.variables(side)).map((v: { name: string }) => v.name);
    });

  // A secret comes back without the panel ever holding its value.
  await row("API_TOKEN").getByLabel("Remove API_TOKEN").click();
  await expect(row("API_TOKEN")).toHaveCount(0);
  await expect(page.locator(".toast")).toContainText("Removed API_TOKEN.");
  expect(await ctxOf()).toContain("API_TOKEN");
  await page.locator(".toast").getByRole("button", { name: "Undo" }).click();
  await expect(row("API_TOKEN")).toHaveAttribute("data-mode", "secret");

  await row("ticket").getByLabel("Remove ticket").click();
  await expect(row("ticket")).toHaveCount(0);
  await page.clock.runFor(11_000);
  await expect(page.locator(".toast")).toHaveCount(0);
  await expect.poll(ctxOf).not.toContain("ticket");
  expect(await ctxOf()).toContain("API_TOKEN");
});

test("a removed pin waits on its toast, and a second removal deletes the first", async ({ page }) => {
  await start(page);
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  await page.locator(".term-pane").first().click();
  const pinLine = async () => {
    await page.evaluate((id) => (window as any).__terminescu.terminals.get(id).selectLines(0, 0), pane);
    await page.keyboard.press("Meta+Shift+P");
  };
  await pinLine();
  await expect(page.locator(".ctx-pin")).toHaveCount(1);
  const pins = () =>
    page.evaluate(async () => {
      const t = (window as any).__terminescu;
      return (await t.host.pins(t.host.workspace().windows[0].sideTabs[0].id)).length;
    });

  await page.locator(".ctx-pin").getByLabel("Remove pin").click();
  await expect(page.locator(".ctx-pin")).toHaveCount(0);
  expect(await pins()).toBe(1);
  await page.locator(".toast").getByRole("button", { name: "Undo" }).click();
  await expect(page.locator(".ctx-pin")).toHaveCount(1);

  // Removing the pin, then a variable: the pin's toast gives way and the pin is deleted.
  await page.locator(".ctx-pin").getByLabel("Remove pin").click();
  const panel = page.locator("aside.ctx-pinned");
  await panel.getByRole("button", { name: "Add a variable" }).click();
  const form = panel.getByRole("form", { name: "Add a variable" });
  await form.getByLabel("Name").fill("ticket");
  await form.getByLabel("Value").fill("example");
  await form.getByRole("button", { name: "Add" }).click();
  await panel.locator('.ctx-var[data-var="ticket"]').getByLabel("Remove ticket").click();
  await expect(page.locator(".toast")).toContainText("Removed ticket.");
  await expect.poll(pins).toBe(0);
});

// refine-context-links: a link row shows its name, or a label from the URL; the URL is in the
// panel tooltip; a note sits under Notes; each shows once.
test("a link row shows its name and the URL in the tooltip", async ({ page }) => {
  await start(page);
  const panel = page.locator("aside.ctx-pinned");
  await page.evaluate(async () => {
    const host = (window as any).__terminescu.host;
    const side = (await host.workspace()).windows[0].sideTabs[0].id;
    for (const u of ["https://github.com/Acme/Shop-API/pull/918/files", "https://github.com/acme/shop-api/pull/918?utm_source=x", "https://claude.ai/code/artifact/abc-123"])
      await host.addItem(side, "link", u, "auto", null);
    for (let i = 0; i < 2; i++) await host.addItem(side, "text", "api, web", "typed", null);
    host.describeLink(side, "https://claude.ai/code/artifact/abc-123", "Release checklist");
    host.describeLink(side, "https://github.com/acme/shop-api/pull/918", "Retry failed payments", { text: "open", tone: "bad", tip: "PR #918 · Checks failing: lint" });
  });
  const rows = panel.locator('ul[aria-label="Links"] .ctx-item');
  await expect(rows.locator(".ctx-link-label")).toHaveText(["Retry failed payments", "Release checklist"]);
  await expect(panel.locator('[data-section="Links"] .ctx-sec-head .ctx-count')).toHaveText("2");
  await expect(rows.first().locator(".ctx-badge")).toHaveText("open");
  await expect(rows.first().locator(".ctx-badge")).toHaveClass(/pl-bad/);
  await expect(rows.locator("svg.ctx-link-icon")).toHaveCount(2);
  const link = rows.nth(1).locator("button.ctx-value");
  await expect(link).not.toHaveAttribute("title");
  await link.hover();
  const tip = page.getByRole("tooltip");
  await expect(tip).toContainText("Release checklist");
  await expect(tip).toContainText("https://claude.ai/code/artifact/abc-123");
  await expect(panel.locator('ul[aria-label="Notes"] .ctx-item')).toHaveText(["api, web×"]);
});

test("a captured branch goes to Notes", async ({ page }) => {
  await start(page);
  const panel = page.locator("aside.ctx-pinned");
  await expect(panel.locator('[data-group="Notes"]')).toHaveCount(0);
  await page.evaluate(async () => {
    const host = (window as any).__terminescu.host;
    const side = (await host.workspace()).windows[0].sideTabs[0].id;
    await host.addItem(side, "link", "https://example.test/a", "pasted", null);
    await host.addItem(side, "link", "https://example.test/b", "pasted", null);
    await host.addItem(side, "text", "feature/PROJ-482-retry", "auto", null);
  });
  await expect(panel.locator('[data-section="Links"] .ctx-sec-head .ctx-count')).toHaveText("2");
  await expect(panel.locator('[data-group="Notes"] .ctx-count')).toHaveText("1");
  await expect(panel.locator('ul[aria-label="Notes"]')).toContainText("feature/PROJ-482-retry");
});

test("copy a link as Markdown", async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as any;
    w.__copied = [];
    Object.defineProperty(navigator, "clipboard", { value: { writeText: async (t: string) => void w.__copied.push(t) } });
  });
  await start(page);
  const panel = page.locator("aside.ctx-pinned");
  await page.evaluate(async () => {
    const host = (window as any).__terminescu.host;
    const side = (await host.workspace()).windows[0].sideTabs[0].id;
    await host.addItem(side, "link", "https://github.com/acme/shop-api/pull/918", "auto", null);
    host.describeLink(side, "https://github.com/acme/shop-api/pull/918", "Retry failed payments");
  });
  const link = panel.locator('ul[aria-label="Links"] button.ctx-value');
  await expect(link).toHaveText("Retry failed payments");
  await link.click({ button: "right" });
  const menu = page.getByRole("menu");
  await expect(menu.getByRole("menuitem")).toHaveText(["Open", "Copy link", "Copy as Markdown", "Remove"]);
  await menu.getByRole("menuitem", { name: "Copy as Markdown" }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__copied.at(-1))).toBe("[Retry failed payments](https://github.com/acme/shop-api/pull/918)");
});
