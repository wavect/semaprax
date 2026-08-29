import { test, expect } from '@playwright/test';

test('generated direct-Bytes package contract', async ({ page }) => {
  // The promotion job injects the exact package URL produced at its commit.
  const packageUrl = process.env.SEMAPRAX_OWNED_DATA_PACKAGE_URL;
  test.skip(!packageUrl, 'hosted package URL is required');
  await page.goto(packageUrl);
  const result = await page.evaluate(async () => {
    const api = await globalThis.semapraxOwnedData;
    const input = new Uint8Array([0, 0xff, 0xc3, 0x28]);
    const output = api.functions['frame.payload'](input);
    return { bytes: [...output], fresh: output !== input };
  });
  expect(result).toEqual({ bytes: [0, 255, 195, 40], fresh: true });
});
