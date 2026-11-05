import { expect, test, type Page } from "@playwright/test";

// Task 4.4 on the mock host: a transcript pane beside a Claude pane, pins on a selection that
// reopen at their quote, and extract to a variable.

const SESSION = "sess-0b5c";
const ANSWER =
  "Plan: copy the audit rows in batches of 5,000 rows on a cursor over id, then compare counts. PR: https://github.com/acme/api/pull/918";

async function start(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  const pane = (await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane"))!;
  await page.evaluate(
    ({ pane, session, answer }) => {
      const h = (window as any).__terminescu.host;
      h.setAgent(pane, { session, state: "done", lastMessage: null });
      h.appendTranscript(session, [
        { id: "u1", offset: 0, timestamp: null, kind: "user", text: "How do we move the audit table?" },
        { id: "a1", offset: 0, timestamp: null, kind: "assistant", text: answer },
        { id: "t1", offset: 0, timestamp: null, kind: "turnEnd" },
      ]);
    },
    { pane, session: SESSION, answer: ANSWER },
  );
  return pane;
}

/** Selects `phrase` inside the message with this entry id. */
const select = (page: Page, entry: string, phrase: string) =>
  page.evaluate(
    ({ entry, phrase }) => {
      const el = document.querySelector(`.tr-msg[data-entry="${entry}"] .tr-text`)!;
      const node = el.firstChild!;
      const at = node.textContent!.indexOf(phrase);
      const r = document.createRange();
      r.setStart(node, at);
      r.setEnd(node, at + phrase.length);
      const sel = window.getSelection()!;
      sel.removeAllRanges();
      sel.addRange(r);
    },
    { entry, phrase },
  );

test("⌘⇧L opens the transcript beside the Claude pane, with messages only", async ({ page }) => {
  const pane = await start(page);
  await page.keyboard.press("Meta+Shift+L");
  const tr = page.locator(".tr-pane");
  await expect(tr).toBeVisible();
  await expect(tr.locator(".tr-msg")).toHaveCount(2);
  await expect(tr.locator('.tr-msg[data-entry="a1"] .tr-text')).toHaveText(ANSWER);
  // The transcript pane has no shell: still one terminal.
  await expect(page.locator(".term-pane")).toHaveCount(1);
  const ws = await page.evaluate(() => (window as any).__terminescu.host.workspace());
  const layout = ws.windows[0].sideTabs[0].tabs[0].layout;
  expect(layout.children.map((c: any) => c.transcript ?? c.id)).toEqual([pane, SESSION]);
  // New lines show up while the pane is open.
  await page.evaluate((s) => {
    (window as any).__terminescu.host.appendTranscript(s, [
      { id: "u2", offset: 0, timestamp: null, kind: "user", text: "Go ahead." },
    ]);
  }, SESSION);
  await expect(tr.locator(".tr-msg")).toHaveCount(3);
});

test("a pinned phrase shows in Pins and reopens highlighted", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Shift+L");
  await expect(page.locator(".tr-msg")).toHaveCount(2);
  await page.keyboard.press("Meta+Backslash");
  await page.locator(".ctx-tab", { hasText: "Pins" }).click();

  const phrase = "batches of 5,000 rows on a cursor over id";
  await select(page, "a1", phrase);
  await page.locator('.tr-msg[data-entry="a1"]').hover();
  await page.locator('.tr-msg[data-entry="a1"] .tr-act', { hasText: "Pin" }).click();
  const pin = page.locator(".ctx-pin");
  await expect(pin).toHaveCount(1);
  await expect(pin.locator(".ctx-quote")).toHaveText(phrase);

  // Close the transcript, then open the pin: the transcript comes back with the phrase marked.
  await page.locator(".tr-pane").click();
  await page.keyboard.press("Meta+W");
  await expect(page.locator(".tr-pane")).toHaveCount(0);
  await page.locator(".term-pane").first().click();
  await pin.locator(".ctx-pin-open").click();
  await expect(page.locator(".tr-mark")).toHaveText(phrase);

  // ⌘⇧P pins the selection too.
  await select(page, "u1", "audit table");
  await page.locator(".tr-pane").focus();
  await page.keyboard.press("Meta+Shift+P");
  await expect(pin).toHaveCount(2);
  await expect(pin.nth(1).locator(".ctx-quote")).toHaveText("audit table");
  // The button without a selection pins the whole message.
  await page.evaluate(() => window.getSelection()!.removeAllRanges());
  await page.locator('.tr-msg[data-entry="u1"]').hover();
  await page.locator('.tr-msg[data-entry="u1"] .tr-act', { hasText: "Pin" }).click();
  await expect(pin).toHaveCount(3);
  await expect(pin.nth(2).locator(".ctx-quote")).toHaveText("How do we move the audit table?");
  await pin.nth(2).locator(".ctx-pin-x").click();
  await expect(pin).toHaveCount(2);
});

test("extract saves the selection as a variable with a suggested name", async ({ page }) => {
  await start(page);
  await page.keyboard.press("Meta+Shift+L");
  await expect(page.locator(".tr-msg")).toHaveCount(2);
  await page.keyboard.press("Meta+Backslash");

  await select(page, "a1", "https://github.com/acme/api/pull/918");
  await page.locator(".tr-pane").focus();
  await page.keyboard.press("Meta+E");
  const name = page.getByLabel("Variable name");
  await expect(name).toHaveValue("pr_api");
  await name.press("Enter");
  await expect(page.locator('.ctx-var[data-var="pr_api"] .ctx-value')).toHaveText("https://github.com/acme/api/pull/918");

  // The same name again is refused, and the value stays.
  await select(page, "a1", "Plan");
  await page.locator('.tr-msg[data-entry="a1"]').hover();
  await page.locator('.tr-msg[data-entry="a1"] .tr-act', { hasText: "Extract" }).click();
  await name.fill("pr_api");
  await name.press("Enter");
  await expect(page.locator(".x-dialog [role=alert]")).toContainText("already has a variable pr_api");
  await name.press("Escape");
  await expect(page.locator(".x-dialog")).toHaveCount(0);

  // A secret: masked in the dialog and the panel, inserted by reference (D21, D35).
  await select(page, "a1", "Plan");
  await page.locator('.tr-msg[data-entry="a1"] .tr-act', { hasText: "Extract" }).click();
  await name.fill("PLAN_NOTE");
  await page.getByRole("radio", { name: "Secret" }).click();
  await expect(page.locator(".x-value")).not.toContainText("Plan");
  await name.press("Enter");
  const secret = page.locator('.ctx-var[data-var="PLAN_NOTE"]');
  await expect(secret).toHaveAttribute("data-mode", "secret");
  await expect(secret.locator(".ctx-value")).not.toContainText("Plan");
  await expect(secret.locator(".ctx-meta")).toHaveText("inserts as $PLAN_NOTE");
});

test("⌘⇧L in a pane without Claude says so and opens nothing", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Shift+L");
  await expect(page.locator(".app-error")).toContainText("No Claude session");
  await expect(page.locator(".tr-pane")).toHaveCount(0);
});
