// Focused installed-CLI acceptance: node test/generated-export-browser.mjs <html> <json>
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { chromium, firefox } from '@playwright/test';

const [htmlArgument, jsonArgument] = process.argv.slice(2);
if (!htmlArgument || !jsonArgument) {
  throw new Error('expected generated HTML and JSON paths');
}
const htmlPath = resolve(htmlArgument);
const jsonSnapshot = JSON.parse(await readFile(resolve(jsonArgument), 'utf8'));
assert.equal(jsonSnapshot.schema, 'semaprax.explorer-snapshot.v1');

for (const [name, browserType] of [['chromium', chromium], ['firefox', firefox]]) {
  const browser = await browserType.launch({ headless: true });
  try {
    const context = await browser.newContext({ offline: true, viewport: { width: 1280, height: 900 } });
    const page = await context.newPage();
    const requests = [];
    const errors = [];
    page.on('request', request => requests.push(request.url()));
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(pathToFileURL(htmlPath).href);
    try {
      await page.getByRole('heading', { name: 'Meaning, mapped.' }).waitFor({ timeout: 5000 });
    } catch (error) {
      console.error(JSON.stringify({ browser: name, errors, requests, title: await page.title(), body: (await page.locator('body').innerText()).slice(0, 1000) }));
      throw error;
    }
    const embedded = JSON.parse(await page.locator('#snapshot').textContent());
    assert.deepEqual(embedded, jsonSnapshot);
    assert.equal(embedded.snapshot_digest, jsonSnapshot.snapshot_digest);
    assert.ok(requests.every(url => url.startsWith('file:')), requests.join('\n'));
    assert.deepEqual(errors, []);
    const modules = page.getByRole('button', { name: /\d+ declarations/ });
    assert.ok(await modules.count() > 0, 'module overview must be interactive');
    await modules.first().click();
    assert.ok(await page.locator('.spx-inspector').isVisible(), 'inspector must open');
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    console.log(JSON.stringify({ browser: name, version: browser.version(), checks: 6, snapshot_digest: embedded.snapshot_digest, requests: requests.length, page_errors: errors.length }));
    await context.close();
  } finally {
    await browser.close();
  }
}
