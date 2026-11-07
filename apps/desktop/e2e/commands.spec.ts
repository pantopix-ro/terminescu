import { expect, test, type Page } from "@playwright/test";

// Task 6.7 on the mock host: saved commands in the sidebar. Run types the line and Enter into
// its target, Insert types it without Enter, a missing name is asked for, a secret goes in as
// $NAME (D24, D35).

const PR = "https://github.com/acme/api/pull/918";

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
}

const firstSide = (page: Page) =>
  page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].sideTabs[0].id as string);

const typed = (page: Page, pane: string) =>
  page.evaluate((p) => (window as any).__terminescu.host.typed.get(p) ?? "", pane) as Promise<string>;

// A new command is global unless `local` ticks "this side tab only" (D24 amendment).
async function addCommand(page: Page, name: string, body: string, target: string, local = false) {
  await page.getByRole("button", { name: "New command" }).click();
  await page.getByLabel("Command name").fill(name);
  await page.getByLabel("Command", { exact: true }).fill(body);
  await page.getByLabel("Run in").selectOption(target);
  if (local) await page.locator(".cmd-form").getByLabel("this side tab only").check();
  await page.locator(".cmd-form").getByRole("button", { name: "Save" }).click();
  await expect(page.locator(`.cmd-row[data-command="${name}"]`)).toBeVisible();
}

const addVariable = (page: Page, side: string, name: string, value: string, mode: string) =>
  page.evaluate(
    ({ side, name, value, mode }) =>
      (window as any).__terminescu.host.addVariable(side, { name, value, mode, source: "typed", originTabId: null }),
    { side, name, value, mode },
  );

test("run in a new tab asks for a missing variable, then types the line and Enter", async ({ page }) => {
  await start(page);
  await addCommand(page, "checks", "gh pr checks {{pr_api}}", "newTab");
  await page.getByRole("button", { name: "Run checks" }).click();
  await page.getByLabel("pr_api").fill(PR);
  await page.locator(".cmd-ask").getByRole("button", { name: "Run" }).click();
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(2);
  const pane = await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane");
  expect(await typed(page, pane!)).toBe(`gh pr checks ${PR}\r`);
  // The value was used once and never saved.
  const side = await firstSide(page);
  const vars = await page.evaluate((s) => (window as any).__terminescu.host.variables(s), side);
  expect(vars).toEqual([]);
});

test("insert into the focused pane types no Enter; a secret goes in as $NAME", async ({ page }) => {
  await start(page);
  const side = await firstSide(page);
  const pane = (await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane"))!;
  await addVariable(page, side, "pr_api", PR, "saved");
  await addVariable(page, side, "API_TOKEN", "tok-example", "secret");
  await addCommand(page, "pr", 'curl -H "Authorization: Bearer {{API_TOKEN}}" {{pr_api}}', "focused");
  await page.getByRole("button", { name: "Insert pr" }).click();
  await expect.poll(() => typed(page, pane)).toBe(`curl -H "Authorization: Bearer $API_TOKEN" ${PR}`);
  expect(await typed(page, pane)).not.toContain("tok-example");
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(1);
});

test("Run refuses a value with a line break; Insert pastes it bracketed", async ({ page }) => {
  await start(page);
  const side = await firstSide(page);
  const pane = (await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane"))!;
  await addVariable(page, side, "note", "first\nsecond", "session");
  await addCommand(page, "echo", "echo {{note}}", "focused");
  await page.getByRole("button", { name: "Run echo" }).click();
  await expect(page.locator(".cmd-section [role=alert]")).toContainText("note holds a line break");
  expect(await typed(page, pane)).toBe("");
  await page.getByRole("button", { name: "Insert echo" }).click();
  await expect.poll(() => typed(page, pane)).toBe("\x1b[200~echo first\nsecond\x1b[201~");
});

test("a global command shows in every side tab, a local one only in its own", async ({ page }) => {
  await start(page);
  await addCommand(page, "local", "ls", "focused", true);
  await addCommand(page, "status", "git status", "newSideTab");
  await page.keyboard.press("Meta+N");
  await expect(page.locator(".ao-st[data-side]")).toHaveCount(2);
  await expect(page.locator(".cmd-row")).toHaveCount(1);
  await expect(page.locator('.cmd-row[data-command="status"] .cmd-meta')).toHaveText("new side tab · global");
  await page.getByRole("button", { name: "Run status" }).click();
  await expect(page.locator(".ao-st[data-side]")).toHaveCount(3);
  await expect(page.locator(".ao-st[data-side]").nth(2)).toHaveAttribute("aria-current", "true");
  await page.getByRole("button", { name: "Delete status" }).click();
  await expect(page.locator(".cmd-row")).toHaveCount(0);
});

test("a command saved in one side tab runs in another with that side tab's variable", async ({ page }) => {
  await start(page);
  await addCommand(page, "checks", "gh pr checks {{pr_api}}", "focused");
  await expect(page.locator('.cmd-row[data-command="checks"] .cmd-meta')).toContainText("global");
  await page.keyboard.press("Meta+N");
  await expect(page.locator(".ao-st[data-side]")).toHaveCount(2);
  const second = await page.evaluate(() => (window as any).__terminescu.host.workspace().windows[0].sideTabs[1].id as string);
  await addVariable(page, second, "pr_api", "https://github.com/acme/api/pull/77", "saved");
  await page.locator(".app-pane .term-pane").first().click();
  const pane = (await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane"))!;
  await page.getByRole("button", { name: "Run checks" }).click();
  await expect.poll(() => typed(page, pane)).toBe("gh pr checks https://github.com/acme/api/pull/77\r");
});
