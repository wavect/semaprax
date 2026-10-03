import { expect, test } from "@playwright/test";
import { readFile, rm, stat } from "node:fs/promises";
import os from "node:os";
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

test("hostile snapshot strings stay inert in the generated file artifact", async ({ page }) => {
  const hostile = await writeOfflineBrowserFixture({ hostile: true });
  try {
    const requests = [];
    page.on("request", request => requests.push(request.url()));
    await page.goto(pathToFileURL(hostile.htmlPath).href);
    await expect(page.getByRole("heading", { name: "Meaning, mapped." })).toBeVisible();
    expect(await page.evaluate(() => globalThis.explorerAttack)).toBeUndefined();
    expect(requests.every(url => url.startsWith("file:"))).toBe(true);
    await expect(page.locator("svg[onload], a[href^='javascript:']")).toHaveCount(0);
  } finally {
    await rm(hostile.directory, { recursive: true, force: true });
  }
});

test("named-host performance: medium and renderer-stress views stay responsive after snapshot data is available", async ({ page, browserName }) => {
  test.skip(process.env.SEMAPRAX_EXPLORER_NAMED_HOST !== "1", "named-host measurement only");
  await page.addInitScript(() => {
    globalThis.explorerLongTasks = [];
    new PerformanceObserver(entries => globalThis.explorerLongTasks.push(...entries.getEntries().map(entry => entry.duration))).observe({ type: "longtask", buffered: true });
  });
  const medium = await writeOfflineBrowserFixture({ performance: "medium" });
  try {
    const [{ size: htmlBytes }, { size: jsonBytes }] = await Promise.all([stat(medium.htmlPath), stat(medium.jsonPath)]);
    const started = performance.now();
    await page.goto(pathToFileURL(medium.htmlPath).href);
    await expect(page.getByRole("heading", { name: "Meaning, mapped." })).toBeVisible();
    const firstUsableMs = performance.now() - started;
    const warmStarted = performance.now();
    await page.reload();
    await expect(page.getByRole("heading", { name: "Meaning, mapped." })).toBeVisible();
    const warmUsableMs = performance.now() - warmStarted;
    const interactions = await page.evaluate(async () => {
      const input = document.querySelector(".spx-search");
      const samples = [];
      for (let index = 0; index < 30; index++) {
        const start = performance.now();
        input.value = `fixture_${index}`;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        await new Promise(requestAnimationFrame);
        samples.push(performance.now() - start);
      }
      return samples.sort((a, b) => a - b);
    });
    const p95Ms = interactions[Math.ceil(interactions.length * 0.95) - 1];
    const maxLongTaskMs = Math.max(0, ...(await page.evaluate(() => globalThis.explorerLongTasks)));
    const machine = `${os.hostname()} cpu=${os.cpus()[0]?.model || "unknown"} cores=${os.cpus().length} ram=${Math.round(os.totalmem() / 1024 / 1024)}MiB`;
    console.log(`explorer named-host benchmark browser=${browserName} cold_first_usable_ms=${firstUsableMs.toFixed(1)} warm_first_usable_ms=${warmUsableMs.toFixed(1)} p95_interaction_ms=${p95Ms.toFixed(1)} max_long_task_ms=${maxLongTaskMs.toFixed(1)} html_bytes=${htmlBytes} json_bytes=${jsonBytes} ${machine}`);
    expect(firstUsableMs).toBeLessThanOrEqual(1000);
    expect(p95Ms).toBeLessThanOrEqual(100);
    expect(maxLongTaskMs).toBeLessThanOrEqual(100);

    const stress = await writeOfflineBrowserFixture({ performance: "renderer_stress" });
    try {
      const stressStarted = performance.now();
      await page.goto(pathToFileURL(stress.htmlPath).href);
      await expect(page.getByRole("heading", { name: "Meaning, mapped." })).toBeVisible();
      const stressUsableMs = performance.now() - stressStarted;
      console.log(`explorer named-host renderer_stress browser=${browserName} first_usable_ms=${stressUsableMs.toFixed(1)} ${machine}`);
      expect(stressUsableMs).toBeLessThanOrEqual(2000);
    } finally {
      await rm(stress.directory, { recursive: true, force: true });
    }
  } finally {
    await rm(medium.directory, { recursive: true, force: true });
  }
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
