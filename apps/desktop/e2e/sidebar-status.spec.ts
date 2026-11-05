import { expect, test, type Page } from "@playwright/test";

// Owner requests of 2026-11-05 on the mock host: a click on a sidebar status opens a quick
// list of the global and this side tab's statuses (task 8.18), and the empty space under the
// last side tab shows a + that adds one.

const mock = (page: Page, fn: string, ...args: unknown[]) =>
  page.evaluate(({ fn, args }) => (window as any).__terminescu.host[fn](...args), { fn, args });

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
}

const look = (label: string) => ({ label, accent: "blue", background: null });

test("a click on a side tab's status opens the quick list of saved statuses", async ({ page }) => {
  await start(page);
  const side = (await workspace(page)).windows[0].sideTabs[0];
  await mock(page, "makeGlobal", (await mock(page, "saveStatus", side.id, look("Blocked"))).id);
  await mock(page, "saveStatus", side.id, look("PROJ-482 review"));

  const row = page.locator(".ao-side .ao-st").first();
  await row.getByRole("button", { name: /^Status Idle: edit the status of / }).click();
  const quick = page.getByRole("menu", { name: "Status" });
  await expect(quick.getByRole("menuitem")).toHaveText(["Idle", "PROJ-482 review", "Blocked", "Edit status…⌘⇧S"]);
  await expect(page.locator(".status-editor")).toHaveCount(0);

  await quick.getByRole("menuitem", { name: "Blocked" }).click();
  await expect(quick).toHaveCount(0);
  await expect(row.locator(".ao-stat.manual")).toHaveText("Blocked");
  // The chip click neither started a drag nor selected another tab.
  expect((await workspace(page)).windows[0].sideTabs).toHaveLength(1);

  // The current status carries the check; Auto clears it.
  await row.getByRole("button", { name: /^Status Blocked/ }).click();
  await expect(quick.getByRole("menuitem", { name: "Blocked" }).locator(".ao-i-check")).toHaveCount(1);
  await page.keyboard.press("Enter");
  await expect(row.locator(".ao-stat.manual")).toHaveCount(0);
});

test("a tab's status chip opens the quick list on that tab; Edit status opens the editor", async ({ page }) => {
  await start(page);
  const side = (await workspace(page)).windows[0].sideTabs[0];
  await mock(page, "send", { type: "CreateTab", sideTabId: side.id });
  const first = (await workspace(page)).windows[0].sideTabs[0].tabs[0];
  await mock(page, "send", { type: "SetStatus", targetId: first.id, status: look("Deploying") });

  await page.locator(".ao-st-kids .ao-st-kid").first().getByRole("button", { name: /^Status Deploying/ }).click();
  await page.getByRole("menu", { name: "Status" }).getByRole("menuitem", { name: "Edit status…" }).click();
  const editor = page.getByRole("dialog");
  await expect(editor.getByRole("tab", { name: /^Tab:/ })).toHaveAttribute("aria-selected", "true");
  await expect(editor.getByLabel("Status label")).toHaveValue("Deploying");
});

test("the empty space under the last side tab shows a + that adds one", async ({ page }) => {
  await start(page);
  const add = page.getByRole("button", { name: "Add a side tab" });
  await expect(add).toHaveCSS("opacity", "0");
  await page.locator(".side-fill").hover();
  await expect(add).toHaveCSS("opacity", "1");
  await add.click();
  await expect(page.locator(".ao-side .ao-st")).toHaveCount(2);
});
