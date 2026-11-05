import { expect, test, type Page } from "@playwright/test";

// Task 8.25 on the mock host: every new side tab gets its own neon on its edge and icon.

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

test("new side tabs open with different neon edges", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  for (let i = 0; i < 3; i++) await page.getByRole("button", { name: "Add a side tab" }).click();
  await expect(page.locator(".ao-st")).toHaveCount(4);

  const accents: string[] = (await workspace(page)).windows[0].sideTabs.map((s: { accent?: string }) => s.accent);
  expect(new Set(accents).size).toBe(4);
  expect(accents).not.toContain(undefined);

  const rows = page.locator(".ao-st");
  for (let i = 0; i < 4; i++) {
    const row = rows.nth(i);
    await expect(row).toHaveAttribute("style", new RegExp(`--st-accent: var\\(--neon-${accents[i]}\\)`));
    // The neon washes 12% into the row's background (the Wash board).
    await expect(row).toHaveAttribute("style", new RegExp(`--st-bg: color-mix\\(in srgb, var\\(--neon-${accents[i]}\\) 12%, var\\(--panel(-2)?\\)\\)`));
    expect(await row.evaluate((el) => getComputedStyle(el).backgroundColor)).not.toBe(
      await row.evaluate(() => {
        const probe = document.createElement("i");
        probe.style.color = "var(--panel)";
        document.body.appendChild(probe);
        const c = getComputedStyle(probe).color;
        probe.remove();
        return c;
      }),
    );
    // The icon takes the neon too, and no neon is amber.
    const [icon, amber] = await row.evaluate((el) => [
      getComputedStyle(el.querySelector(".ao-i")!).color,
      getComputedStyle(document.documentElement).getPropertyValue("--amber").trim(),
    ]);
    expect(icon).not.toBe(amber);
    expect(icon).toBe(await row.evaluate((el, n) => {
      const probe = document.createElement("i");
      probe.style.color = `var(--neon-${n})`;
      el.appendChild(probe);
      const c = getComputedStyle(probe).color;
      probe.remove();
      return c;
    }, accents[i]));
  }
});

// Task 8.26: each tab has its own neon on its icon and on its square under the row's name.
test("a side tab's tabs show their neons on their icons and squares", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  for (let i = 0; i < 2; i++) await page.getByRole("button", { name: "New tab", exact: true }).click();
  const side = (await workspace(page)).windows[0].sideTabs[0];
  const neons: string[] = side.tabs.map((t: { accent?: string }) => t.accent);
  expect(neons).toHaveLength(3);
  expect(new Set(neons).size).toBe(3);

  const row = page.locator(".ao-st").first();
  const cells = row.locator(".ao-strip .ao-cell");
  await expect(cells).toHaveCount(3);
  for (let i = 0; i < 3; i++) {
    await expect(cells.nth(i)).toHaveAttribute("style", `background: var(--neon-${neons[i]});`);
    await expect(page.locator(`.ao-st-kid[data-kid="${side.tabs[i].id}"] .ao-i`)).toHaveAttribute("style", `color: var(--neon-${neons[i]});`);
    await expect(page.locator(`.ao-tab[data-tab="${side.tabs[i].id}"] .ao-i`)).toHaveAttribute("style", `color: var(--neon-${neons[i]});`);
  }
  // A one-tab row shows its single square too.
  await page.getByRole("button", { name: "Add a side tab" }).click();
  await expect(page.locator(".ao-st").nth(1).locator(".ao-cell")).toHaveCount(1);
});
