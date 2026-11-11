// Captures the README images from reel.html (the interactive tour) with WebKit.
//   node docs/media/source/capture.mjs shots <out>    the stills, at 2x
//   node docs/media/source/capture.mjs frames <out>   every scene at 5 frames a second, at 1x
// docs/media/README.md has the encoding steps.
import { createRequire } from "module";
const require = createRequire(new URL("../../../apps/desktop/package.json", import.meta.url));
const { webkit } = require("@playwright/test");
const [mode, out] = process.argv.slice(2);
// [file, scene, second]: the moment each still shows.
const SHOTS = [["folders", 2, 7.2], ["split-panes", 3, 5.6], ["side-by-side", 4, 7.2], ["needs-you", 5, 2.3],
  ["context", 6, 7.6], ["commands", 7, 2.7], ["prompts", 8, 2.3], ["claude-insights", 9, 7.5], ["git", 10, 2.3], ["editor", 11, 4.8]];
const browser = await webkit.launch();
const page = await browser.newPage({ viewport: { width: 1232, height: 1100 }, deviceScaleFactor: mode === "shots" ? 2 : 1 });
await page.goto(new URL("reel.html", import.meta.url).href);
await page.evaluate(() => document.fonts.ready);
await page.waitForTimeout(800);
const stage = page.locator("#stage");
const scene = async (i) => { await page.locator(".chip").nth(i - 1).click(); await page.mouse.move(0, 0); };
if (mode === "shots") {
  for (const [name, i, t] of SHOTS) {
    await scene(i);
    await page.waitForTimeout(t * 1000);
    await page.keyboard.press("Space");
    await page.addStyleTag({ content: ".hud{display:none!important}" });
    await stage.screenshot({ path: `${out}/${name}.png` });
  }
} else {
  let f = 0;
  const n = await page.locator(".chip").count();
  for (let i = 1; i <= n; i++) {
    await scene(i);
    const dur = await page.evaluate((i) => SC[i - 1].dur, i);
    const t0 = Date.now();
    while (Date.now() - t0 < dur * 1000 - 100) {
      const t = Date.now();
      await stage.screenshot({ path: `${out}/${String(f++).padStart(4, "0")}.png` });
      await page.waitForTimeout(Math.max(0, 200 - (Date.now() - t)));
    }
  }
}
await browser.close();
