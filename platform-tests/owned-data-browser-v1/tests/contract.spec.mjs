import { test, expect } from '@playwright/test';
import { runBrowserBoundaries } from './browser-boundaries.mjs';

test('generated direct-Bytes package contract', async ({ page, context }) => {
  const supplied = process.env.SEMAPRAX_OWNED_DATA_PACKAGE_URL;
  expect(typeof supplied === 'string' && supplied.length > 0,
    'SEMAPRAX_OWNED_DATA_PACKAGE_URL must name the provisioned package directory').toBe(true);
  const base = new URL(supplied);
  expect(base.protocol).toBe('http:');
  expect(base.hostname).toBe('127.0.0.1');
  expect(base.username + base.password + base.search + base.hash).toBe('');
  expect(base.href, 'use an exact normalized directory URL').toBe(supplied);
  expect(base.pathname.endsWith('/')).toBe(true);
  expect(/%(?:2f|5c)/i.test(base.pathname), 'encoded path separators are not admitted').toBe(false);

  const documentUrl = new URL('__browser_contract__.html', base).href;
  const allowed = new Set(['semaprax.bindings.js', 'semaprax.js', 'app.wasm', 'semaprax.api.json',
    'package.json', 'semaprax.bindings.d.ts']
    .map(name => new URL(name, base).href));
  const violations = [];
  await context.route('**/*', async route => {
    const url = route.request().url();
    if (url === documentUrl && route.request().isNavigationRequest()) {
      await route.fulfill({ status: 200, contentType: 'text/html', headers: {
        'Cross-Origin-Opener-Policy': 'same-origin',
        'Cross-Origin-Embedder-Policy': 'require-corp',
      }, body: '<!doctype html><meta charset="utf-8"><link rel="icon" href="data:,"><title>Owned data browser contract</title>' });
    } else if (allowed.has(url) && route.request().method() === 'GET') {
      // Do not follow even a loopback server's redirect outside this inventory.
      const response = await route.fetch({ maxRedirects: 0 });
      if (response.status() !== 200) {
        violations.push(`${url}: HTTP ${response.status()}`);
        await route.abort();
      } else await route.fulfill({ response });
    } else {
      violations.push(url);
      await route.abort();
    }
  });
  await page.goto(documentUrl);
  const result = await page.evaluate(runBrowserBoundaries, base.href);
  expect(violations, 'all resource requests must stay inside the exact package inventory').toEqual([]);
  expect(result).toEqual({ bytes: [0, 255, 195, 40], fresh: true, instantiated: 2,
    capacityCases: 3, failureRounds: 16 });
});
