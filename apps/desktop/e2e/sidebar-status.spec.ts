import { expect, test, type Page } from "@playwright/test";

// Owner requests of 2026-11-05 on the mock host: a click on a sidebar status opens a quick
// pick: a box for a new status, then the current, this side tab's and the global statuses
// (tasks 8.18, 8.21), and the empty space under the
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

test("a click on a side tab's status opens the quick pick with the box focused", async ({ page }) => {
  await start(page);
  const side = (await workspace(page)).windows[0].sideTabs[0];
  await mock(page, "saveStatus", side.id, look("PROJ-482 review"));

  const row = page.locator(".ao-side .ao-st").first();
  // A one-line row shows an unset status only under the mouse (task 8.77).
  await expect(row.locator(".ao-stat")).toBeHidden();
  await row.hover();
  await row.getByRole("button", { name: /^Status Idle: edit the status of / }).click();
  const quick = page.getByRole("dialog", { name: /^Status of / });
  await expect(quick.getByLabel("New status")).toBeFocused();
  await expect(quick.getByRole("menuitemradio")).toHaveText(["Idle", "PROJ-482 review", "In progress", "Review", "Testing", "Blocked", "Done"]);
  await expect(quick.getByRole("menuitemradio", { name: "Auto" })).toHaveAttribute("aria-checked", "true");
  await expect(page.locator(".status-editor")).toHaveCount(0);

  // A global default, picked with the mouse.
  await quick.getByRole("menuitemradio", { name: "Blocked" }).click();
  await expect(quick).toHaveCount(0);
  await expect(row.locator(".ao-stat.manual")).toHaveText("Blocked");
  // The chip click neither started a drag nor selected another tab.
  expect((await workspace(page)).windows[0].sideTabs).toHaveLength(1);

  // A new one typed in the box; it stays offered, checked, next time.
  await row.getByRole("button", { name: /^Status Blocked/ }).click();
  await expect(quick.getByRole("menuitemradio", { name: "Blocked" })).toHaveAttribute("aria-checked", "true");
  await page.keyboard.type("Waiting on QA");
  await page.keyboard.press("Enter");
  await expect(row.locator(".ao-stat.manual")).toHaveText("Waiting on QA");
  await row.getByRole("button", { name: /^Status Waiting on QA/ }).click();
  await expect(quick.getByRole("menuitemradio").first()).toHaveText("Waiting on QA");
  await expect(quick.getByRole("menuitemradio").first()).toHaveAttribute("aria-checked", "true");

  // Down from the box walks the list: the typed one, then Auto clears it.
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(row.locator(".ao-stat.manual")).toHaveCount(0);
});

test("a tab's status chip opens the quick pick on that tab; Edit status opens the editor", async ({ page }) => {
  await start(page);
  const side = (await workspace(page)).windows[0].sideTabs[0];
  await mock(page, "send", { type: "CreateTab", sideTabId: side.id });
  const first = (await workspace(page)).windows[0].sideTabs[0].tabs[0];
  await mock(page, "send", { type: "SetStatus", targetId: first.id, status: look("Deploying") });

  await page.locator(".ao-st-kids .ao-st-kid").first().getByRole("button", { name: /^Status Deploying/ }).click();
  await page.getByRole("dialog", { name: /^Status of / }).getByRole("button", { name: /^Edit status…/ }).click();
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
