import { expect, test } from "@playwright/test";

test("generated HTTPS package executes its bounded fixture provider in Chromium", async ({ page }) => {
  const origins = [];
  const failures = [];
  const pageErrors = [];
  const wasmResponses = [];
  page.on("request", (request) => origins.push(new URL(request.url()).origin));
  page.on("requestfailed", (request) => failures.push(request.url()));
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("response", (response) => {
    if (new URL(response.url()).pathname === "/generated/app.wasm") wasmResponses.push(response);
  });

  await page.goto("/index.html");
  const result = await page.evaluate(() => globalThis.semapraxHttpsResult);
  const expected = "HTTP/1.1 200 semaprax\r\ncontent-length: 2\r\n\r\nok";
  expect(result).toEqual({
    stdout: expected,
    reuseRejected: true,
    authenticationRejected: true,
  });
  await expect(page.locator("#status")).toHaveText("passed");
  await expect(page.locator("#stdout")).toHaveText(expected);
  expect(wasmResponses).toHaveLength(1);
  expect(wasmResponses[0].headers()["content-type"]).toBe("application/wasm");
  expect(new Set(origins)).toEqual(new Set([new URL(page.url()).origin]));
  expect(failures).toEqual([]);
  expect(pageErrors).toEqual([]);
});
