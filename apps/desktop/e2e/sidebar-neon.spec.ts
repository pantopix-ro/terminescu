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
