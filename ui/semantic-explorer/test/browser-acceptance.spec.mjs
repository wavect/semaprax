import { expect, test } from "@playwright/test";
import { readFile, rm } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { writeOfflineBrowserFixture } from "./offline-browser-fixture.mjs";

let fixture;
test.beforeAll(async () => { fixture = await writeOfflineBrowserFixture(); });
test.afterAll(async () => { await rm(fixture.directory, { recursive: true, force: true }); });

async function openOffline(page, width) {
  await page.setViewportSize({ width, height: 900 });
  const requests = [];
  const errors = [];
  page.on("request", request => requests.push(request.url()));
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(pathToFileURL(fixture.htmlPath).href);
  await expect(page.getByRole("heading", { name: "Meaning, mapped." })).toBeVisible();
  return { requests, errors };
}

test("file export opens offline and keeps its embedded JSON byte-identical", async ({ page }) => {
  const { requests, errors } = await openOffline(page, 1280);
  const json = await readFile(fixture.jsonPath, "utf8");
  const embedded = await page.locator("#snapshot").textContent();
  expect(`${embedded}\n`).toBe(json);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  expect(requests.every(url => url.startsWith("file:"))).toBe(true);
  expect(errors).toEqual([]);
});

test("keyboard navigation reaches a declaration, relation provenance, and overview", async ({ page }) => {
  await openOffline(page, 1280);
  const search = page.getByRole("searchbox", { name: "Search declarations" });
  await search.focus();
  await page.keyboard.type("alpha");
  await page.getByRole("button", { name: /app.*2 declarations/ }).last().focus();
  await page.keyboard.press("Enter");
  const alpha = page.getByRole("button", { name: "alpha · function" });
  await alpha.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { name: "alpha" })).toBeVisible();
  const edge = page.locator("path.spx-edge");
  await edge.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByText("Relationship", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Overview" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { name: "Select a module or declaration" })).toBeVisible();
});

for (const width of [768, 1280]) {
  test(`controls and inspector fit at ${width}px`, async ({ page }) => {
    await openOffline(page, width);
    await page.getByRole("button", { name: /app.*2 declarations/ }).last().click();
    await expect(page.getByRole("heading", { name: "app", exact: true })).toBeVisible();
    expect(await page.evaluate(() => ({ overflow: document.documentElement.scrollWidth > window.innerWidth, inspector: document.querySelector(".spx-inspector")?.getBoundingClientRect().width ?? 0 }))).toEqual({ overflow: false, inspector: expect.any(Number) });
    expect(await page.locator(".spx-inspector").evaluate(element => element.getBoundingClientRect().width)).toBeGreaterThan(0);
  });
}

test("reduced motion, dark color scheme, and forced colors preserve visible controls", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce", forcedColors: "active" });
  await openOffline(page, 768);
  await page.getByRole("searchbox", { name: "Search declarations" }).focus();
  expect(await page.locator(".spx-search").evaluate(element => ({ transition: getComputedStyle(element).transitionDuration, outline: getComputedStyle(element).outlineStyle, reduced: matchMedia("(prefers-reduced-motion: reduce)").matches, forced: matchMedia("(forced-colors: active)").matches }))).toEqual({ transition: "0s", outline: "solid", reduced: true, forced: true });
  await expect(page.getByRole("button", { name: "Theme" })).toBeVisible();
});
