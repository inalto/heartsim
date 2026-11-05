// Browser smoke test for `trunk build --release` output (brief B21).
//
//   cd crates/heartsim-web && trunk build --release
//   cd tests/browser && npm install && npx playwright install chromium && npm test
//
// It serves dist/ over HTTP with plain static headers (the build needs no COOP/COEP), opens the page in
// headless Chromium on software WebGL, and checks what `heartsim-web` publishes in `window.heartsimStatus`:
//   * the canvas exists and has a size,
//   * the anatomy pack arrived and was loaded (`anatomy === 1`),
//   * frames are drawn and the ECG strip advances (`stripSamples` and `simTimeMs` grow),
//   * the screenshot is not a blank canvas,
//   * nothing panicked (console errors).
// Environment: HEARTSIM_DIST (default ../../dist), HEARTSIM_BROWSER (chromium | firefox | webkit),
// HEARTSIM_BROWSER_PATH (use an already installed browser binary instead of Playwright's).
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { dirname, extname, join, normalize, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, firefox, webkit } from 'playwright';

const here = dirname(fileURLToPath(import.meta.url));
const dist = resolve(process.env.HEARTSIM_DIST ?? join(here, '../../dist'));
const types = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.zip': 'application/zip',
};

const server = createServer(async (req, res) => {
  try {
    const url = new URL(req.url, 'http://localhost');
    const rel = normalize(decodeURIComponent(url.pathname)).replace(/^([/\\])+/, '');
    const file = join(dist, rel === '' ? 'index.html' : rel);
    if (!file.startsWith(dist)) throw new Error('outside dist');
    const body = await readFile(file);
    res.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream' });
    res.end(body);
  } catch {
    res.writeHead(404).end('not found');
  }
});
await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
const base = `http://127.0.0.1:${server.address().port}/`;

const engine = { chromium, firefox, webkit }[process.env.HEARTSIM_BROWSER ?? 'chromium'];
const browser = await engine.launch({
  executablePath: process.env.HEARTSIM_BROWSER_PATH || undefined,
  args: process.env.HEARTSIM_BROWSER ? [] : ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const failures = [];
const check = (ok, what) => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
  if (!ok) failures.push(what);
};

try {
  const page = await (await browser.newContext({ viewport: { width: 1280, height: 800 } })).newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  await page.goto(base);

  await page.waitForFunction(() => (window.heartsimStatus?.frames ?? 0) > 5, null, { timeout: 120_000 });
  const first = await page.evaluate(() => ({ ...window.heartsimStatus }));
  await page.waitForTimeout(3000);
  const second = await page.evaluate(() => ({ ...window.heartsimStatus }));

  const box = await page.locator('canvas#heartsim_canvas').boundingBox();
  check(box && box.width > 300 && box.height > 300, `canvas is ${box?.width}x${box?.height}`);
  check(second.anatomy === 1, 'anatomy pack downloaded and loaded');
  check(second.frames > first.frames, `frames advance (${first.frames} -> ${second.frames})`);
  check(second.stripSamples > first.stripSamples, `ECG strip advances (${first.stripSamples} -> ${second.stripSamples} samples)`);
  check(second.simTimeMs > first.simTimeMs, `simulation clock advances (${first.simTimeMs.toFixed(0)} -> ${second.simTimeMs.toFixed(0)} ms)`);
  check((await page.locator('#loading').count()) === 0, 'loading screen removed');

  const png = await page.screenshot();
  await import('node:fs/promises').then((fs) => fs.writeFile(join(here, 'smoke.png'), png));
  // a blank (single-colour) 1280x800 PNG is a few KB; a drawn UI with a strip and a heart is far larger
  check(png.length > 40_000, `screenshot has content (${png.length} bytes)`);
  check(!errors.some((e) => /panick|unreachable|RuntimeError/i.test(e)), `no panics (${errors.length} console errors)`);
  if (errors.length) console.log(errors.slice(0, 5).join('\n'));
} finally {
  await browser.close();
  server.close();
}
if (failures.length) {
  console.error(`\n${failures.length} check(s) failed`);
  process.exit(1);
}
console.log('\nweb smoke test passed');
