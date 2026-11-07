import { expect, test, type Page } from "@playwright/test";

// Task 8.47 on the mock host: Settings › Terminal › Colours picks a scheme, imports one from
// Terminal.app or a file, and sets the opacity; every terminal changes at once.

const option = (page: Page, key: string) =>
  page.evaluate((key) => [...(window as any).__terminescu.terminals.values()][0].options[key], key);
const background = async (page: Page) => ((await option(page, "theme")) as { background: string }).background;
const rootHas = (page: Page, attr: string) => page.evaluate((a) => document.documentElement.hasAttribute(a), attr);

async function openColours(page: Page) {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+Comma");
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: "Terminal" }).click();
  return dialog;
}

test("a scheme and the opacity apply live, and Reset brings the defaults back", async ({ page }) => {
  const dialog = await openColours(page);
  const panel = await background(page);
  expect(await rootHas(page, "data-scheme")).toBe(false);
  // Imported shows only once something was imported.
  await expect(dialog.getByLabel("Colour theme").locator("option")).toHaveText(["Terminescu", "Terminal.app Basic", "Solarized Dark", "Solarized Light"]);

  await dialog.getByLabel("Colour theme").selectOption("solarizedDark");
  await expect.poll(() => background(page)).toBe("#002b36");
  expect(((await option(page, "theme")) as { red: string }).red).toBe("#dc322f");
  expect(await rootHas(page, "data-scheme")).toBe(true);

  await dialog.getByLabel("Terminal opacity").fill("80");
  await expect.poll(() => background(page)).toBe("rgba(0, 43, 54, 0.8)");
  expect(await option(page, "allowTransparency")).toBe(true);
  expect(await rootHas(page, "data-see-through")).toBe(true);
  // The sidebar stays solid.
  expect(await page.locator(".app-side").evaluate((el) => getComputedStyle(el).backgroundColor)).not.toMatch(/rgba\(.*, 0\)|transparent/);

  await dialog.getByRole("button", { name: "Reset to defaults" }).click();
  await expect.poll(() => background(page)).toBe(panel);
  expect(await option(page, "allowTransparency")).toBe(false);
  expect(await rootHas(page, "data-see-through")).toBe(false);
});

test("Import from Terminal.app takes its colours, font, cursor and opacity", async ({ page }) => {
  const dialog = await openColours(page);
  await dialog.getByRole("button", { name: "From Terminal.app" }).click();
  await expect(dialog.getByRole("status").filter({ hasText: "Imported Ocean" })).toHaveText(
    "Imported Ocean: colours, Menlo, 12 px, block cursor, 90% opacity.",
  );
  await expect.poll(() => background(page)).toBe("rgba(27, 43, 74, 0.9)");
  await expect(dialog.getByLabel("Colour theme")).toHaveValue("imported");
  expect(await option(page, "cursorStyle")).toBe("block");
  expect(await option(page, "fontFamily")).toMatch(/^"Menlo"/);

  // A file: an .itermcolors preset brings only colours; anything else is refused.
  await page.evaluate(() => (window as any).__terminescu.host.dialogAnswers.push("/Users/owner/themes/Harbor.itermcolors"));
  await dialog.getByRole("button", { name: "File…" }).click();
  await expect(dialog.getByRole("status").filter({ hasText: "Harbor" })).toHaveText("Imported Harbor: colours.");
  await page.evaluate(() => (window as any).__terminescu.host.dialogAnswers.push("/Users/owner/notes.txt"));
  await dialog.getByRole("button", { name: "File…" }).click();
  await expect(page.locator(".app-error")).toContainText("not a Terminal.app profile");
});
