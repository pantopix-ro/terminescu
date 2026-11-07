import { expect, test } from "@playwright/test";

// Task 8.43 on the mock host: files dropped from Finder on a terminal go in as shell-quoted
// paths without Enter; dropped on the tab bar they still open in text tabs.

test("a Finder drop on a terminal types the quoted paths, a drop on the tab bar opens them", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?top-tabs");
  const term = page.locator(".term-pane").first();
  await expect(term).toBeVisible();
  const pane = (await term.getAttribute("data-pane"))!;
  const box = (await term.boundingBox())!;
  const at = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  await page.evaluate(
    (at) => (window as any).__terminescu.host.dropFiles(["/Users/owner/acme-api/README.md", "/Users/owner/My Files/it's.txt"], at),
    at,
  );
  const inputs = () => page.evaluate((p) => (window as any).__terminescu.host.inputs.get(p) ?? "", pane) as Promise<string>;
  await expect.poll(inputs).toContain(`/Users/owner/acme-api/README.md '/Users/owner/My Files/it'\\''s.txt' `);
  expect(await inputs()).not.toContain("\r");
  await expect(page.locator(".app-main .ao-tab")).toHaveCount(1);

  await page.evaluate(() => (window as any).__terminescu.host.files.set("/Users/owner/acme-api/CHANGELOG.md", "# Changes\n"));
  const bar = (await page.locator(".ao-tabs").boundingBox())!;
  await page.evaluate(
    (at) => (window as any).__terminescu.host.dropFiles(["/Users/owner/acme-api/CHANGELOG.md"], at),
    { x: bar.x + bar.width - 40, y: bar.y + bar.height / 2 },
  );
  await expect(page.locator(".app-main .ao-tab").nth(1)).toContainText("CHANGELOG.md");
});
