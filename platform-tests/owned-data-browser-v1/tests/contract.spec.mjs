import { test, expect } from '@playwright/test';
import { runBrowserBoundaries } from './browser-boundaries.mjs';

test('generated direct and variant owned-Bytes package contract', async ({ page, context }) => {
  function packageDirectory(variable) {
    const supplied = process.env[variable];
    expect(typeof supplied === 'string' && supplied.length > 0,
      `${variable} must name the provisioned package directory`).toBe(true);
    const base = new URL(supplied);
    expect(base.protocol).toBe('http:');
    expect(base.hostname).toBe('127.0.0.1');
    expect(base.username + base.password + base.search + base.hash).toBe('');
    expect(base.href, 'use an exact normalized directory URL').toBe(supplied);
    expect(base.pathname.endsWith('/')).toBe(true);
    expect(/%(?:2f|5c)/i.test(base.pathname), 'encoded path separators are not admitted').toBe(false);
    return base;
  }
  const base = packageDirectory('SEMAPRAX_OWNED_DATA_PACKAGE_URL');
  const variants = packageDirectory('SEMAPRAX_OWNED_DATA_VARIANT_PACKAGE_URL');
  expect(variants.origin, 'both packages must share the isolated document origin').toBe(base.origin);
  expect(variants.href, 'provision two distinct package directories').not.toBe(base.href);

  const documentUrl = new URL('__browser_contract__.html', base).href;
  const files = ['semaprax.bindings.js', 'semaprax.js', 'app.wasm', 'semaprax.api.json',
    'package.json', 'semaprax.bindings.d.ts'];
  const allowed = new Set([base, variants].flatMap(directory => files.map(name => new URL(name, directory).href)));
  expect(allowed.size).toBe(12);
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
  const result = await page.evaluate(runBrowserBoundaries, { packageUrl: base.href, variantPackageUrl: variants.href });
  expect(violations, 'all resource requests must stay inside the exact package inventory').toEqual([]);
  expect(result).toEqual({ bytes: [0, 255, 195, 40], fresh: true, instantiated: 3, tamperCases: 2,
    capacityCases: 3, failureRounds: 16, offsetCases: 3, variantCalls: 96,
    variantActive: 64, variantInactive: 32, variantOversized: 4, variantRecoveries: 4,
    resizableCases: 8 });
});
