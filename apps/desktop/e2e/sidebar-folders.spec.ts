import { expect, test, type Page } from "@playwright/test";

// Tasks 8.90-8.92 on the mock host: a merge keeps the tabs' names, the sidebar's + makes an
// empty folder, and each window carries its row's name.

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
}

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());
const send = (page: Page, command: unknown) => page.evaluate((c) => (window as any).__terminescu.host.send(c), command);
const sides = async (page: Page) => (await workspace(page)).windows[0].sideTabs as { id: string; folder?: boolean; tabs: { id: string; title: string | null }[] }[];
const rows = (page: Page) => page.locator(".ao-side .ao-st");
const edit = (page: Page) => page.getByRole("dialog", { name: "Edit" });

test("merged rows keep their names as tabs of the folder", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "New terminal" }).click();
  await expect(rows(page)).toHaveCount(2);
  const [a, b] = await sides(page);
  await send(page, { type: "RenameSideTab", sideTabId: a!.id, title: "web" });
  await send(page, { type: "RenameSideTab", sideTabId: b!.id, title: "api" });
  await rows(page).nth(0).click({ button: "right" });
  await edit(page).getByLabel("Merge into").selectOption({ label: "api" });
  await expect(rows(page)).toHaveCount(1);
  await expect(page.locator(`[data-kid="${b!.tabs[0]!.id}"] .grow`)).toHaveText(/^api/);
  await expect(page.locator(`[data-kid="${a!.tabs[0]!.id}"] .grow`)).toHaveText(/^web/);
});

test("the sidebar's + makes an empty folder that tabs join and leave", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Choose side tab kind" }).click();
  await page.getByRole("menuitem", { name: /Folder/ }).click();
  await expect(rows(page)).toHaveCount(2);
  const [row, folder] = await sides(page);
  expect(folder).toMatchObject({ folder: true, tabs: [] });
  await expect(page.locator(".kid-empty")).toHaveText("Empty. Drag a tab here or press +.");
  // The first row stays selected; a click on the empty folder closes and opens it.
  await expect(rows(page).nth(0)).toHaveClass(/active/);
  await rows(page).nth(1).click();
  await expect(page.locator(".kid-empty")).toHaveCount(0);
  await expect(rows(page).nth(0)).toHaveClass(/active/);
  await rows(page).nth(1).click();
  await expect(page.locator(".kid-empty")).toBeVisible();
  // A row's only tab joins it; the folder keeps its tab as a folder's tab.
  await send(page, { type: "MoveTab", tabId: row!.tabs[0]!.id, toSideTabId: folder!.id, index: 0 });
  await expect(rows(page)).toHaveCount(1);
  await expect(page.locator(".ao-st-kid")).toHaveCount(1);
  await expect(page.locator(".kid-empty")).toHaveCount(0);
  // Its + adds a tab inside; with every tab closed the folder stays, empty.
  await page.getByRole("button", { name: /^New tab in / }).click();
  await expect(page.locator(".ao-st-kid")).toHaveCount(2);
  for (const t of (await sides(page))[0]!.tabs) await send(page, { type: "CloseTab", tabId: t.id });
  await expect(rows(page)).toHaveCount(1);
  await expect(page.locator(".kid-empty")).toBeVisible();
});

test("⌃⌘N and the empty space's menu make folders too", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Control+Meta+KeyN");
  await expect(rows(page)).toHaveCount(2);
  await page.locator(".side-fill").click({ button: "right" });
  await page.getByRole("menuitem", { name: /New folder/ }).click();
  await expect(rows(page)).toHaveCount(3);
  expect((await sides(page)).filter((s) => s.folder && !s.tabs.length)).toHaveLength(2);
});

test("the window is named after its selected row", async ({ page }) => {
  await start(page);
  await expect(page).toHaveTitle(/ · Terminescu$/);
  const [a] = await sides(page);
  await send(page, { type: "RenameTab", tabId: a!.tabs[0]!.id, title: "deploy" });
  await expect(page).toHaveTitle("deploy · Terminescu");
});
