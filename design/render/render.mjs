import { createRequire } from 'module';
// playwright-core resolves from this repo, or from PLAYWRIGHT_FROM (any package.json that depends on it).
const require = createRequire(process.env.PLAYWRIGHT_FROM || import.meta.url);
const { chromium } = require('playwright-core');
import fs from 'fs'; import path from 'path';
const [proj, out, ...only] = process.argv.slice(2);
const tokens = fs.readFileSync(path.join(proj,'tokens.css'),'utf8');
const bundle = fs.readFileSync(path.join(proj,'components/bundle.css'),'utf8');
const imp = bundle.match(/@import url\("[^"]+"\);/)[0];
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 960, height: 400 }, deviceScaleFactor: 1 });
fs.mkdirSync(out, { recursive: true });
const comps = fs.readdirSync(path.join(proj,'components')).filter(c => fs.existsSync(path.join(proj,'components',c,'preview.html')) && (!only.length || only.includes(c)));
for (const c of comps) {
  let html = fs.readFileSync(path.join(proj,'components',c,'preview.html'),'utf8');
  const h = +(html.match(/height=(\d+)/)||[0,200])[1];
  html = html.replace('<head>', () => `<head><link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Geist+Mono:wght@400;600&family=Silkscreen&display=swap"><style>${tokens}\n${bundle.replace(imp,'')}</style>`).replace('<html', '<html data-theme="dark"');
  await page.setViewportSize({ width: 960, height: h });
  await page.setContent(html, { waitUntil: 'networkidle' });
  await page.evaluate(() => document.fonts.ready);
  const m = await page.evaluate(() => { const n=document.querySelector('.name'); if(!n) return null; return [...n.querySelectorAll('span')].map(s=>{const r=document.createRange(); r.selectNodeContents(s); return r.getBoundingClientRect().width;}); });
  if (m) console.log(c, 'name widths', m);
  console.log(await page.evaluate(()=>[document.styleSheets.length, document.styleSheets[1]?.cssRules.length, getComputedStyle(document.body).backgroundColor]));
  const sh = await page.evaluate(() => document.documentElement.scrollHeight);
  console.log(c, 'height', h, 'scroll', sh);
  await page.screenshot({ path: path.join(out, c + '.png') });
}
await browser.close();
