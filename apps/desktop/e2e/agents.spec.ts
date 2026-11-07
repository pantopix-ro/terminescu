import { expect, test } from "@playwright/test";

// Task 4.2 on the mock host: the mock plays the core's part and sets a pane's agent the way
// hook events would. In the app the same updates come from `terminescu hook`.

test("a waiting Claude session turns its tab and side tab amber until it runs again", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  const pane = await page.locator(".app-pane .ao-tp").first().getAttribute("data-pane");
  const set = (state: string | null) =>
    page.evaluate(
      ({ pane, state }) =>
        (window as any).__terminescu.host.setAgent(pane, state && { session: "sess-0b5c", state, lastMessage: null }),
      { pane, state },
    );
  const tab = page.locator(".ao-tabs [role=tab]").first();
  const side = page.locator(".ao-st").first();

  // The tab keeps the icon it opened with (task 8.20); Claude's state dot comes and goes.
  await expect(tab.locator(".ao-dot")).toHaveCount(0);
  await set("running");
  await expect(tab).toHaveAttribute("data-agent", "running");
  await expect(tab.locator(".ao-dot")).toHaveCount(1);
  await expect(tab).not.toHaveClass(/\bwait\b/);

  await set("waiting");
  await expect(tab).toHaveClass(/\bwait\b/);
  await expect(side).toHaveClass(/\bneed\b/);

  await set("running");
  await expect(tab).not.toHaveClass(/\bwait\b/);
  await expect(side).not.toHaveClass(/\bneed\b/);

  // SessionEnd: the tab shows its shell again.
  await set(null);
  await expect(tab.locator(".ao-dot")).toHaveCount(0);
  await expect(tab).not.toHaveAttribute("data-agent", /.+/);
});
