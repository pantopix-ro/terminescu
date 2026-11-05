import { expect, test, type Page } from "@playwright/test";

// Task 6.3: the prompt library on the mock host, which keeps the core's rules (D14).

async function open(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".term-pane")).toHaveCount(1);
  await page.keyboard.press("Meta+Shift+I");
  const lib = page.getByRole("dialog", { name: "Prompt library" });
  await expect(lib).toBeVisible();
  return lib;
}

test("search finds prompts by tag and body", async ({ page }) => {
  const lib = await open(page);
  const names = lib.getByRole("list", { name: "Prompts" }).locator(".pl-name");
  await expect(names).toHaveText(["Open PRs", "Review plan"]);
  await lib.getByLabel("Search prompts").fill("#plan");
  await expect(names).toHaveText(["Review plan"]);
  await lib.getByLabel("Search prompts").fill("confidence");
  await expect(names).toHaveText(["Review plan"]);
  await lib.getByLabel("Search prompts").fill("develop #plan");
  await expect(names).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(lib).toHaveCount(0);
});

test("saving makes a version, restore brings an old body back as a new one", async ({ page }) => {
  const lib = await open(page);
  await lib.locator('.pl-item[data-prompt="review-plan"]').click();
  const detail = lib.locator('.pl-detail[data-prompt="review-plan"]');
  const body = detail.getByLabel("Prompt body");
  const first = await body.inputValue();
  await body.fill("Review the plan for {{ticket}} on {{branch}}.");
  await body.press("Meta+s");
  await expect(detail.locator(".pl-version")).toHaveText("v2");
  await expect(detail).toContainText("{{ticket}} {{branch}}");

  // Escape with unsaved changes leaves the field and keeps the sheet.
  await body.fill("draft");
  await body.press("Escape");
  await expect(lib).toBeVisible();
  await body.fill("Review the plan for {{ticket}} on {{branch}}.");

  await detail.getByRole("button", { name: "versions" }).click();
  await detail.getByRole("button", { name: "Restore version 1" }).click();
  await expect(detail.locator(".pl-version")).toHaveText("v3");
  await expect(body).toHaveValue(first);
  await expect(detail.locator(".pl-ver")).toHaveCount(3);
});

test("duplicate, rename and archive", async ({ page }) => {
  const lib = await open(page);
  await lib.locator('.pl-item[data-prompt="open-prs"]').click();
  await lib.getByRole("button", { name: "duplicate" }).click();
  const copy = lib.locator('.pl-detail[data-prompt="open-prs-copy"]');
  await expect(copy.getByLabel("Prompt name")).toHaveValue("Open PRs copy");
  await copy.getByLabel("Tags").fill("git, release");
  await copy.getByLabel("Tags").press("Enter");
  await lib.getByLabel("Search prompts").fill("#release");
  await expect(lib.locator(".pl-name")).toHaveText(["Open PRs copy"]);
  await copy.getByRole("button", { name: "archive" }).click();
  await expect(lib.locator(".pl-name")).toHaveCount(0);
  await lib.getByLabel("archived").check();
  await expect(lib.locator(".pl-name")).toHaveText(["Open PRs copy"]);
});

test("a new prompt shows in the context panel and opens the library at it", async ({ page }) => {
  const lib = await open(page);
  await lib.getByRole("button", { name: "+ prompt" }).click();
  const form = lib.getByRole("form", { name: "New prompt" });
  await form.getByLabel("Prompt name").fill("Audit export");
  await form.getByLabel("Prompt body").fill("Check {{pr_api}} against PROJ-482.");
  await form.getByRole("button", { name: "Create" }).click();
  await expect(lib.locator('.pl-detail[data-prompt="audit-export"]')).toBeVisible();
  await page.keyboard.press("Escape");

  await page.keyboard.press("Meta+Backslash");
  await page.locator(".ctx-tab", { hasText: "Prompts" }).click();
  const row = page.locator('aside.ctx-pinned .ctx-item[data-prompt="audit-export"]');
  await expect(row).toContainText("{{pr_api}}");
  await row.getByRole("button", { name: "Audit export" }).click();
  await expect(page.locator('.pl-item.sel[data-prompt="audit-export"]')).toBeVisible();
});
