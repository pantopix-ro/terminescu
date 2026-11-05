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

test("variables: add in each mode, mask a secret, refuse to unmask it", async ({ page }) => {
  await start(page);
  const panel = page.locator("aside.ctx-pinned");
  const add = async (name: string, value: string, mode: string, ttl?: string) => {
    await panel.getByRole("button", { name: "+ variable" }).click();
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
  await expect(row("from_date").locator(".ctx-meta")).toContainText(/gone on reboot · (59m|1h) left/);
  await expect(row("API_TOKEN").locator(".ctx-value")).not.toContainText("tok-example");
  await expect(row("API_TOKEN").locator(".ctx-meta")).toHaveText("inserts as $API_TOKEN");
  await expect(page.locator("body")).not.toContainText("tok-example");

  // A saved value turns secret; a secret offers no other mode.
  await row("ticket").getByLabel("Storage of ticket").selectOption("secret");
  await expect(row("ticket")).toHaveAttribute("data-mode", "secret");
  await expect(row("ticket").locator(".ctx-value")).not.toContainText("PROJ-482");
  await expect(row("ticket").locator('option[value="saved"]')).toBeDisabled();

  // Edit a value in place.
  await row("from_date").locator(".ctx-value").click();
  await row("from_date").getByLabel("Value of from_date").fill("2026-11-01");
  await row("from_date").getByLabel("Value of from_date").press("Enter");
  await expect(row("from_date").locator(".ctx-value")).toHaveText("2026-11-01");

  await row("from_date").getByLabel("Remove from_date").click();
  await expect(row("from_date")).toHaveCount(0);
});

test("links are kept once; text is kept as typed", async ({ page }) => {
  await start(page);
  const form = page.getByRole("form", { name: "Add a link or text" });
  const field = form.getByLabel("Link or text");
  for (const v of ["https://github.com/acme/api/pull/918", "https://github.com/acme/api/pull/918", "api, web"]) {
    await field.fill(v);
    await field.press("Enter");
  }
  const items = page.locator(".ctx-item");
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
  await page.locator(".ctx-tab", { hasText: "Pins" }).click();
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
  await panel.getByRole("button", { name: "+ variable" }).click();
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
