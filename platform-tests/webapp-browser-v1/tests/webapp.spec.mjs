import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { expect, test } from "@playwright/test";

const appRoot = process.env.SEMAPRAX_WEBAPP_ROOT;
if (!appRoot) throw new Error("set SEMAPRAX_WEBAPP_ROOT to a freshly generated webapp directory");
const root = resolve(appRoot);
let server;
let baseURL;
let dataDir;

test.beforeAll(async () => {
  dataDir = mkdtempSync(join(tmpdir(), "semaprax-webapp-browser-"));
  server = spawn(process.execPath, [join(root, "server.mjs"), "--port", "0", "--host", "127.0.0.1", "--data", dataDir, "--setup"], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const ready = new Promise((resolveReady, reject) => {
    const timeout = setTimeout(() => reject(new Error(`server did not start: ${output}`)), 10_000);
    const consume = (chunk) => {
      output += chunk.toString();
      const match = /listening on (http:\/\/127\.0\.0\.1:\d+\/)/.exec(output);
      if (match) { clearTimeout(timeout); resolveReady(match[1]); }
    };
    server.stdout.on("data", consume);
    server.stderr.on("data", consume);
    server.once("exit", (code) => { clearTimeout(timeout); reject(new Error(`server exited ${code}: ${output}`)); });
  });
  baseURL = await ready;

  const create = async (path, row) => {
    const response = await fetch(new URL(`api/${path}`, baseURL), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(row),
    });
    const text = await response.text();
    if (response.status !== 201) throw new Error(`fixture ${path} failed (${response.status}): ${text}`);
    return JSON.parse(text);
  };
  await create("team", { name: "Browser Smoke Team", description: "Local browser fixture" });
  await create("customer", { company: "Browser Smoke Customer", contact: "Smoke User", email: "customer@example.test", phone: "555-0100", tier: "Free", seats: 3 });
  await create("project", { team_id: 1, customer_id: 1, name: "Browser Smoke Project", code: "SMOKE604", status: "Planned", budget: 1000, start_day: 1, due_day: 10 });
  await create("milestone", { project_id: 1, title: "Browser milestone", due_day: 5, done: false });
  await create("sprint", { project_id: 1, name: "Browser sprint", start_day: 1, end_day: 10 });
});

test.afterAll(async () => {
  if (server && server.exitCode === null) {
    server.kill("SIGTERM");
    for (let attempt = 0; attempt < 30 && server.exitCode === null; attempt++) await delay(100);
    if (server.exitCode === null) server.kill("SIGKILL");
  }
  if (dataDir) rmSync(dataDir, { recursive: true, force: true });
});

test("sign in, create and advance a task, filter its list, and export CSV", async ({ page }) => {
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto(baseURL);
  await page.goto(new URL("#/member/new", baseURL).href);
  await page.getByLabel("team_id").selectOption("1");
  await page.getByLabel("name").fill("Browser Smoke Admin");
  await page.getByLabel("email").fill("smoke-admin@example.test");
  await page.getByLabel("role").selectOption("Admin");
  await page.getByLabel("active").check();
  await page.getByLabel("password").fill("browser-smoke-passphrase");
  await page.getByRole("button", { name: "Create" }).click();

  await expect(page.getByRole("heading", { name: /Sign in to/ })).toBeVisible();
  await page.getByLabel("email").fill("smoke-admin@example.test");
  await page.getByLabel("password").fill("browser-smoke-passphrase");
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("button", { name: "Sign out" })).toBeVisible();

  await page.goto(new URL("#/task/new", baseURL).href);
  await page.getByLabel("project_id").selectOption("1");
  await page.getByLabel("milestone_id").selectOption("1");
  await page.getByLabel("sprint_id").selectOption("1");
  await page.getByLabel("member_id").selectOption("1");
  await page.getByLabel("title").fill("Browser smoke task");
  await page.getByLabel("details").fill("Created and advanced in Chromium");
  await page.getByLabel("priority").selectOption("Medium");
  await page.getByLabel("status").selectOption("Todo");
  await page.getByLabel("estimate").fill("8");
  await page.getByLabel("spent").fill("1");
  await page.getByRole("button", { name: "Create" }).click();
  await expect(page.getByRole("heading", { name: "Task 1" })).toBeVisible();

  await page.getByRole("link", { name: "Edit" }).click();
  await page.getByLabel("status").selectOption("Doing");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("status: Todo → Doing")).toBeVisible();

  await page.goto(new URL("#/task", baseURL).href);
  await page.getByLabel("filter status").selectOption("Doing");
  await expect(page.getByRole("row").filter({ hasText: "Browser smoke task" })).toBeVisible();
  const csvLink = page.getByRole("link", { name: "Export CSV" });
  await expect(csvLink).toHaveAttribute("href", "/api/task?format=csv&status=Doing");
  const [download] = await Promise.all([page.waitForEvent("download"), csvLink.click()]);
  const csv = readFileSync(await download.path(), "utf8");
  expect(csv).toContain("Browser smoke task");
  expect(csv).toContain(",Doing,");
  expect(pageErrors).toEqual([]);
});
