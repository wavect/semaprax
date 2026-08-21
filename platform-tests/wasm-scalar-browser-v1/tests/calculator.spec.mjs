import { expect, test } from "@playwright/test";

async function calculate(page, { left, operation, right, expected }) {
  await page.locator("#left").fill(left);
  await page.locator("#operation").selectOption(operation);
  await page.locator("#right").fill(right);
  await page.getByRole("button", { name: "Calculate" }).click();
  await expect(page.locator("#result")).toHaveText(expected);
}

test("loopback Chromium calculator preserves stable-ID success, failure, and re-entry", async ({ page }) => {
  const wasmResponses = [];
  const requestOrigins = [];
  const requestFailures = [];
  const pageErrors = [];
  page.on("request", (request) => requestOrigins.push(new URL(request.url()).origin));
  page.on("response", (response) => {
    if (new URL(response.url()).pathname === "/generated/app.wasm") {
      wasmResponses.push(response);
    }
  });
  page.on("requestfailed", (request) => requestFailures.push(request.url()));
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.goto("/index.html");
  await expect(page.getByRole("heading", { name: "SEMAPRAX calculator" })).toBeVisible();

  await calculate(page, {
    left: "19",
    operation: "calculator.add",
    right: "23",
    expected: "42",
  });
  await calculate(page, {
    left: "6",
    operation: "calculator.multiply",
    right: "7",
    expected: "42",
  });
  await calculate(page, {
    left: "10",
    operation: "calculator.subtract",
    right: "3",
    expected: "7",
  });
  await calculate(page, {
    left: "9223372036854775807",
    operation: "calculator.add",
    right: "1",
    expected: "semantic failure: semaprax.arithmetic.v1/1",
  });
  await calculate(page, {
    left: "84",
    operation: "calculator.divide",
    right: "0",
    expected: "semantic failure: semaprax.contract.v1/1",
  });
  await calculate(page, {
    left: "84",
    operation: "calculator.divide",
    right: "2",
    expected: "42",
  });

  await expect.poll(() => wasmResponses.length).toBe(1);
  expect(wasmResponses[0].status()).toBe(200);
  expect(new URL(wasmResponses[0].url()).origin).toBe("http://127.0.0.1:4173");
  expect(new Set(requestOrigins)).toEqual(new Set(["http://127.0.0.1:4173"]));
  expect(requestFailures).toEqual([]);
  expect(pageErrors).toEqual([]);
});
