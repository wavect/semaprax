import { test, expect } from "../../wasm-scalar-browser-v1/node_modules/@playwright/test/index.mjs";
import { readFileSync } from "node:fs";

const canonical = readFileSync(new URL("../../../examples/frame-payload-project/corpus.json", import.meta.url), "utf8");
const corpus = JSON.parse(canonical);
const encoded = process.env.SEMAPRAX_FRAME_BROWSER_URLS;
if (!encoded) throw new Error('SEMAPRAX_FRAME_BROWSER_URLS requires {"before":"http://127.0.0.1:PORT/","after":"http://127.0.0.1:PORT/"}');
const urls = JSON.parse(encoded);
if (!urls || Array.isArray(urls) || Object.keys(urls).sort().join(",") !== "after,before") throw new Error("exact before/after URL keys required");
for (const value of Object.values(urls)) {
  const url = new URL(value);
  if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || url.username || url.password || url.search || url.hash || !url.pathname.endsWith("/")) throw new Error("fixture URLs must be loopback HTTP directory URLs");
}
if (urls.before === urls.after) throw new Error("distinct before/after fixtures required");

test("both display names execute the identical canonical corpus in Chromium", async ({ browser }) => {
  const descriptors = [];
  for (const label of ["before", "after"]) {
    const base = urls[label];
    const context = await browser.newContext();
    try {
      await context.route("**/*", route => {
        if (new URL(route.request().url()).origin !== new URL(base).origin) return route.abort();
        return route.continue();
      });
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", error => errors.push(String(error)));
      const corpusResponse = page.waitForResponse(new URL("corpus.json", base).href);
      await page.goto(new URL("index.html", base).href);
      expect(await (await corpusResponse).text()).toBe(canonical);
      await expect(page.locator("html")).toHaveAttribute("data-status", "passed");
      expect(JSON.parse(await page.locator("#result").textContent())).toEqual({
        schema: "semaprax.frame-payload-consumer.v1",
        cases: corpus.cases.length,
        directCalls: corpus.cases.filter(row => row.valid).length,
      });
      expect(errors).toEqual([]);
      const api = await page.evaluate(async () => {
        const response = await fetch("./generated/semaprax.api.json");
        if (!response.ok) throw new Error("missing descriptor");
        return response.json();
      });
      descriptors.push(JSON.parse(api.descriptor));
    } finally { await context.close(); }
  }
  expect(descriptors[0].exports).toEqual(descriptors[1].exports);
  expect(descriptors[0].project_revision).not.toEqual(descriptors[1].project_revision);
});
