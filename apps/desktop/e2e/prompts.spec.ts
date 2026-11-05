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

// Task 6.4: `⌘I` inserts prompts and variables into the focused terminal (D13).
async function insertSetup(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.locator(".term-pane")).toHaveCount(1);
  const pane = await page.evaluate(() => document.querySelector<HTMLElement>(".term-pane")!.dataset.pane!);
  await page.keyboard.press("Meta+Backslash");
  const panel = page.locator("aside.ctx-pinned");
  const add = async (name: string, value: string, mode: string) => {
    await panel.getByRole("button", { name: "+ variable" }).click();
    const form = panel.getByRole("form", { name: "Add a variable" });
    await form.getByLabel("Name").fill(name);
    await form.getByLabel("Storage").selectOption(mode);
    await form.getByLabel("Value").fill(value);
    await form.getByRole("button", { name: "Add" }).click();
    await expect(form).toHaveCount(0);
  };
  await add("ticket", "PROJ-482", "saved");
  await add("API_TOKEN", "tok-example", "secret");
  await page.locator(".term-pane").first().click();
  const inputs = () => page.evaluate((id) => (window as any).__terminescu.host.inputs.get(id) ?? "", pane);
  return { pane, panel, inputs };
}

test("⌘I inserts a prompt with a missing variable asked inline, and plain insert writes no \\r", async ({ page }) => {
  const { panel, inputs } = await insertSetup(page);
  await page.evaluate(() =>
    (window as any).__terminescu.host.createPrompt({
      name: "Deploy check",
      tags: [],
      body: "Check {{ticket}} on {{branch}}\nwith $X={{API_TOKEN}}",
    }),
  );
  const before = await inputs();
  await page.keyboard.press("Meta+I");
  const dialog = page.getByRole("dialog", { name: "Insert" });
  await dialog.getByLabel("Search prompts and variables").fill("deploy");
  await dialog.getByLabel("Search prompts and variables").press("Enter");
  await dialog.getByLabel("Value for branch").fill("feature/audit-export");
  await dialog.getByLabel("Save branch to the context").check();
  await expect(dialog.getByLabel("Text to insert")).toHaveValue("Check PROJ-482 on feature/audit-export\nwith $X=$API_TOKEN");
  await dialog.getByLabel("Text to insert").press("Meta+Enter");
  await expect(dialog).toHaveCount(0);

  await expect.poll(async () => (await inputs()).slice(before.length)).toBe(
    "\x1b[200~Check PROJ-482 on feature/audit-export\nwith $X=$API_TOKEN\x1b[201~",
  );
  expect((await inputs()).slice(before.length)).not.toContain("\r");
  expect(await inputs()).not.toContain("tok-example");
  await expect(panel.locator('.ctx-var[data-var="branch"] .ctx-value')).toHaveText("feature/audit-export");

  // The library counts the use.
  const used = await page.evaluate(async () => (await (window as any).__terminescu.host.searchPrompts("deploy"))[0].useCount);
  expect(used).toBe(1);
});

test("insert and send adds one Enter; a variable goes in from the list", async ({ page }) => {
  const { inputs } = await insertSetup(page);
  const before = await inputs();
  await page.keyboard.press("Meta+I");
  const dialog = page.getByRole("dialog", { name: "Insert" });
  await dialog.getByLabel("Search prompts and variables").fill("ticket");
  await expect(dialog.getByRole("option")).toHaveCount(2);
  await dialog.getByRole("option").filter({ hasText: "{{ticket}}" }).filter({ hasText: "saved" }).click();
  await expect.poll(async () => (await inputs()).slice(before.length)).toBe("PROJ-482");

  const mid = await inputs();
  await page.keyboard.press("Meta+I");
  await dialog.getByLabel("Search prompts and variables").fill("review plan");
  await dialog.getByLabel("Search prompts and variables").press("Enter");
  await expect(dialog.getByLabel("Text to insert")).toHaveValue(/^Review the plan for PROJ-482\. Rate/);
  await dialog.getByRole("button", { name: "Insert and send ⌘⇧↵" }).click();
  await expect.poll(async () => (await inputs()).slice(mid.length)).toMatch(/^\x1b\[200~Review the plan for PROJ-482\..*\n\x1b\[201~\r$/s);
});

// Task 6.5: save as prompt, offering `{{name}}` for context values in the text (D14).
test("saving a sent message that holds the branch value offers {{branch}}", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  const pane = (await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane"))!;
  await page.keyboard.press("Meta+Backslash");
  const panel = page.locator("aside.ctx-pinned");
  await panel.getByRole("button", { name: "+ variable" }).click();
  const form = panel.getByRole("form", { name: "Add a variable" });
  await form.getByLabel("Name").fill("branch");
  await form.getByLabel("Value").fill("feature/audit-export");
  await form.getByRole("button", { name: "Add" }).click();
  await page.evaluate((pane) => {
    const h = (window as any).__terminescu.host;
    h.setAgent(pane, { session: "sess-0b5c", state: "done", lastMessage: null });
    h.appendTranscript("sess-0b5c", [
      { id: "u1", offset: 0, timestamp: null, kind: "user", text: "Rebase feature/audit-export on develop and push." },
      { id: "t1", offset: 0, timestamp: null, kind: "turnEnd" },
    ]);
  }, pane);
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Shift+L");
  const msg = page.locator('.tr-msg[data-entry="u1"]');
  await msg.hover();
  await msg.getByRole("button", { name: "Save as prompt" }).click();

  const dialog = page.getByRole("dialog", { name: "Save as prompt" });
  await expect(dialog.getByLabel("Use {{branch}}")).toBeChecked();
  await expect(dialog.getByLabel("Prompt preview")).toHaveValue("Rebase {{branch}} on develop and push.");
  await dialog.getByLabel("Prompt name").fill("Rebase branch");
  await dialog.getByRole("button", { name: "Save prompt" }).click();

  const lib = page.getByRole("dialog", { name: "Prompt library" });
  await expect(lib.locator('.pl-item.sel[data-prompt="rebase-branch"]')).toBeVisible();
  await expect(lib.getByLabel("Prompt body")).toHaveValue("Rebase {{branch}} on develop and push.");
});

test("the insert dialog saves its edited text as a prompt", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+I");
  const dialog = page.getByRole("dialog", { name: "Insert" });
  await dialog.getByLabel("Search prompts and variables").fill("open prs");
  await dialog.getByLabel("Search prompts and variables").press("Enter");
  await dialog.getByLabel("Value for branch").fill("feature/audit-export");
  await dialog.getByLabel("Text to insert").fill("Open a PR from feature/audit-export into main.");
  await dialog.getByRole("button", { name: "Save as prompt" }).click();
  await dialog.getByLabel("Prompt name").fill("Open PRs to main");
  await dialog.getByRole("button", { name: "Save prompt" }).click();
  await expect(dialog).toContainText("saved as Open PRs to main");
  const body = await page.evaluate(async () => (await (window as any).__terminescu.host.searchPrompts("main"))[0].body);
  // `branch` was typed, not in the context: nothing to offer, the text is kept as written.
  expect(body).toBe("Open a PR from feature/audit-export into main.");
});

test("the empty space under the last prompt shows a + that opens a new prompt", async ({ page }) => {
  const lib = await open(page);
  const add = lib.getByRole("button", { name: "Add a prompt" });
  await expect(add).toHaveCSS("opacity", "0");
  await lib.locator(".pl-fill").hover();
  await expect(add).toHaveCSS("opacity", "1");
  await add.click();
  await expect(lib.getByRole("form", { name: "New prompt" })).toBeVisible();
});
