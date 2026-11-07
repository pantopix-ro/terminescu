import { expect, test, type Page } from "@playwright/test";

// Task 6.8 on the mock host: a launcher command runs in its own tab, the side tab shows
// `n/m up`, and Restart types the command again into its tab once it is down (D24).

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
}

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

const typed = (page: Page, pane: string) =>
  page.evaluate((p) => (window as any).__terminescu.host.typed.get(p) ?? "", pane) as Promise<string>;

// The first pane of the tab a launcher command runs in.
const launcherPane = (page: Page, name: string) =>
  page.evaluate(async (name) => {
    const host = (window as any).__terminescu.host;
    const side = host.workspace().windows[0].sideTabs[0];
    const c = (await host.commands(side.id)).find((x: { name: string }) => x.name === name);
    const tab = side.tabs.find((t: { id: string }) => t.id === c.tabId);
    return tab.layout.id as string;
  }, name);

async function addLauncher(page: Page, name: string, body: string) {
  await page.getByRole("button", { name: "New command" }).click();
  await page.getByLabel("Command name").fill(name);
  await page.getByLabel("Command", { exact: true }).fill(body);
  await page.locator(".cmd-form").getByLabel("launcher").check();
  await page.locator(".cmd-form").getByRole("button", { name: "Save" }).click();
  await expect(page.locator(`.cmd-row[data-command="${name}"]`)).toBeVisible();
}

test("a launcher opens its tab, shows n/m up, and restarts once down", async ({ page }) => {
  await start(page);
  await addLauncher(page, "api", "pnpm dev --filter api");
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(2);
  const pane = await launcherPane(page, "api");
  expect(await typed(page, pane)).toBe("pnpm dev --filter api\r");

  const chip = page.locator(".ao-st").first().locator(".ao-stat");
  const row = page.locator('.cmd-row[data-command="api"]');
  await expect(chip).toHaveText("0/1 up");
  await expect(row.locator(".cmd-down")).toBeVisible();

  await mock(page, "runCommand", pane, "pnpm dev --filter api", null);
  await expect(chip).toHaveText("1/1 up");
  await expect(row.locator(".cmd-up")).toBeVisible();
  await expect(page.getByRole("button", { name: "Restart api" })).toHaveCount(0);

  await mock(page, "runCommand", pane, "pnpm dev --filter api", 1);
  await expect(chip).toHaveText("0/1 up");
  await page.getByRole("button", { name: "Restart api" }).click();
  // Typed again into the same tab: no new tab opens.
  await expect.poll(() => typed(page, pane)).toBe("pnpm dev --filter api\r".repeat(2));
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(2);
});

test("a new command starts global; a launcher stays in its side tab", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "New command" }).click();
  const form = page.locator(".cmd-form");
  const local = form.getByLabel("this side tab only");
  await expect(local).not.toBeChecked();
  await form.getByLabel("launcher").check();
  await expect(local).toBeChecked();
  await expect(local).toBeDisabled();
  await form.getByLabel("launcher").uncheck();
  await expect(local).toBeEnabled();
});

test("a manual status wins over a launcher's n/m up", async ({ page }) => {
  await start(page);
  await addLauncher(page, "web", "pnpm dev --filter web");
  const side = page.locator(".ao-st").first();
  await expect(side.locator(".ao-stat")).toHaveText("0/1 up");
  const id = await page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].sideTabs[0].id as string);
  await mock(page, "send", { type: "SetStatus", targetId: id, status: { label: "Parked", accent: "fg", background: null } });
  await expect(side.locator(".ao-stat")).toHaveText("Parked");
});
