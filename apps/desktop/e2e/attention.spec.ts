import { expect, test, type Page } from "@playwright/test";

// Task 6.6 on the mock host: the amber count in the status line, `⌘⇧A` to the session waiting
// longest, and the chime, off by default. The dock badge and the notification need the app.

const setAgent = (page: Page, pane: string, state: string | null) =>
  page.evaluate(
    ({ pane, state }) =>
      (window as any).__terminescu.host.setAgent(pane, state ? { session: `sess-${pane}`, state, lastMessage: null } : null),
    { pane, state },
  );

/** Every pane, by side tab, in sidebar order. */
const panes = (page: Page) =>
  page.evaluate(() => {
    const ws = (window as any).__terminescu.host.workspace();
    const ids = (l: any): string[] => (l.children ? l.children.flatMap(ids) : l.transcript ? [] : [l.id]);
    return ws.windows[0].sideTabs.map((s: any) => s.tabs.flatMap((t: any) => ids(t.layout)));
  }) as Promise<string[][]>;

test("the status line counts waiting sessions and ⌘⇧A jumps to the oldest", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  await page.keyboard.press("Meta+D");
  await expect(page.locator(".term-pane")).toHaveCount(2);
  await page.keyboard.press("Meta+Shift+KeyN");
  await expect(page.locator(".ao-st[data-side]")).toHaveCount(2);
  const [first, second] = await panes(page);
  const [a, b] = first as [string, string];
  const c = second![0]!;
  const status = page.locator(".app-status");
  await expect(status).toContainText("2 side tabs · 2 tabs");
  await expect(status.locator(".need")).toHaveCount(0);

  await setAgent(page, a, "waiting");
  await setAgent(page, c, "waiting");
  await setAgent(page, b, "running");
  await expect(status.locator(".need")).toHaveText("2 needs you");
  await expect(status).toContainText("1 running");

  // The side tab in front is the second one; A waited first.
  await page.keyboard.press("Meta+Shift+A");
  const sides = page.locator(".ao-st[data-side]");
  await expect(sides.first()).toHaveAttribute("aria-current", "true");
  await expect(page.locator(`.ao-tp[data-pane="${a}"]`)).toHaveClass(/focused/);

  // A answered: C waits longest now, and the amber segment jumps there too.
  await setAgent(page, a, "running");
  await expect(status.locator(".need")).toHaveText("1 needs you");
  await status.locator(".need").click();
  await expect(sides.nth(1)).toHaveAttribute("aria-current", "true");

  await setAgent(page, c, null);
  await expect(status.locator(".need")).toHaveCount(0);
  await page.keyboard.press("Meta+Shift+A");
  await expect(page.locator(".app-note")).toHaveText("No session is waiting.");
});

test("the chime is off by default and plays once per new wait when on", async ({ page }) => {
  await page.goto("/");
  await page.locator(".term-pane").first().click();
  const a = (await panes(page))[0]![0]!;
  const host = () => page.evaluate(() => (window as any).__terminescu.host.chimes() as number);
  await setAgent(page, a, "waiting");
  expect(await host()).toBe(0);
  await setAgent(page, a, null);
  await page.evaluate(() => (window as any).__terminescu.host.setAttentionSettings({ notify: true, chime: true }));
  await setAgent(page, a, "waiting");
  await setAgent(page, a, "waiting");
  expect(await host()).toBe(1);
});
