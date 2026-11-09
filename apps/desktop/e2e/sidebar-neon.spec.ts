import { expect, test, type Page } from "@playwright/test";

// Task 8.25 on the mock host: every new side tab gets its own neon on its edge and icon.

const workspace = (page: Page) => page.evaluate(() => (window as any).__terminescu.host.workspace());

test("new side tabs open with different neon edges", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  for (let i = 0; i < 3; i++) await page.keyboard.press("Meta+n");
  await expect(page.locator(".ao-st")).toHaveCount(4);

  const accents: string[] = (await workspace(page)).windows[0].sideTabs.map((s: { accent?: string }) => s.accent);
  expect(new Set(accents).size).toBe(4);
  expect(accents).not.toContain(undefined);
  // Task 8.51: a one-tab row wears its tab's neon, so the sidebar shows the tabs' colours.
  const worn: string[] = (await workspace(page)).windows[0].sideTabs.map(
    (s: { accent?: string; tabs: { accent?: string }[] }) => (s.tabs.length === 1 && s.tabs[0]?.accent) || s.accent,
  );

  const rows = page.locator(".ao-st");
  for (let i = 0; i < 4; i++) {
    const row = rows.nth(i);
    await expect(row).toHaveAttribute("style", new RegExp(`--st-accent: var\\(--neon-${worn[i]}\\)`));
    // The neon washes 12% into the row's background (the Wash board).
    await expect(row).toHaveAttribute("style", new RegExp(`--st-bg: color-mix\\(in srgb, var\\(--neon-${worn[i]}\\) 12%, var\\(--panel(-2)?\\)\\)`));
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
    }, worn[i]));
  }
});

// Task 8.26: each tab has its own neon on its icon and on its square under the row's name.
test("a side tab's tabs show their neons on their icons and squares", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
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
  // A one-tab row is one line with no squares; its icon edge wears its tab's neon (task 8.77).
  await page.keyboard.press("Meta+n");
  const single = page.locator(".ao-st").nth(1);
  await expect(single).toHaveClass(/single/);
  await expect(single.locator(".ao-cell")).toHaveCount(0);
  const accent = (await workspace(page)).windows[0].sideTabs[1].tabs[0].accent;
  expect(await single.getAttribute("style")).toContain(`--st-accent: var(--neon-${accent})`);
});

// Task 8.29: each tab in the tab bar washes its neon into its background and shows its own icon.
test("tabs in the tab bar take a 12% wash of their neon and each has an icon", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  await page.locator(".term-pane").first().click();
  for (let i = 0; i < 2; i++) await page.getByRole("button", { name: "New tab", exact: true }).click();
  const tabs: { id: string; accent: string; icon: string }[] = (await workspace(page)).windows[0].sideTabs[0].tabs;
  expect(new Set(tabs.map((t) => t.icon)).size).toBe(3);
  const bgs = new Set<string>();
  for (const t of tabs) {
    const tab = page.locator(`.ao-tab[data-tab="${t.id}"]`);
    await expect(tab).toHaveClass(/neon/);
    await expect(tab).toHaveAttribute("style", new RegExp(`--tab-bg: color-mix\\(in srgb, var\\(--neon-${t.accent}\\) 12%, var\\(--panel(-2)?\\)\\)`));
    await expect(tab.locator(`.ao-i-${t.icon}`)).toBeVisible();
    bgs.add(await tab.evaluate((el) => getComputedStyle(el).backgroundColor));
  }
  // The active tab is tinted too, so every tab reads differently.
  expect(bgs.size).toBe(3);
});
