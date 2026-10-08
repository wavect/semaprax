import assert from 'node:assert/strict';
import { spawn, type ChildProcess } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { asI64, I64_MAX, I64_MIN, parseJson, stringifyJson } from '../shared/json.ts';

const project = fileURLToPath(new URL('..', import.meta.url));

test('HTTP integers, references, computed output, CSV and audit survive restart', { timeout: 30000 }, async () => {
  const data = await mkdtemp(join(tmpdir(), 'teamdesk-exact-'));
  let child: ChildProcess | undefined;
  let cookie = '';
  const probe = createServer();
  await new Promise<void>((resolve) => probe.listen(0, '127.0.0.1', resolve));
  const port = (probe.address() as { port: number }).port;
  await new Promise<void>((resolve) => probe.close(() => resolve()));
  const base = `http://127.0.0.1:${port}/api`;
  const start = async () => {
    let errors = '';
    child = spawn(process.execPath, ['--experimental-strip-types', 'server/index.ts'], {
      cwd: project, env: { ...process.env, DATA_DIR: data, PORT: String(port) }, stdio: ['ignore', 'ignore', 'pipe'],
    });
    child.stderr!.on('data', (chunk) => { errors += chunk; });
    for (let attempt = 0; attempt < 100; attempt++) {
      if (child.exitCode !== null) throw new Error(`server exited: ${errors}`);
      try { await fetch(`${base}/setup`); return; } catch { await new Promise((r) => setTimeout(r, 25)); }
    }
    throw new Error(`server did not start: ${errors}`);
  };
  const stop = async () => {
    if (!child || child.exitCode !== null) return;
    const exited = new Promise<void>((resolve) => child!.once('exit', () => resolve()));
    child.kill('SIGTERM');
    await exited;
  };
  const request = async (method: string, path: string, input?: any, raw = false) => {
    const res = await fetch(base + path, { method,
      headers: { 'content-type': 'application/json', cookie },
      body: input === undefined ? undefined : raw ? input : stringifyJson(input),
    });
    const text = await res.text();
    return { res, text, body: text && res.headers.get('content-type')?.includes('application/json') ? parseJson(text) : undefined };
  };
  const signIn = async () => {
    const result = await request('POST', '/session', { email: 'admin@example.test', password: 'password123' });
    assert.equal(result.res.status, 200, result.text);
    cookie = result.res.headers.get('set-cookie')!.split(';')[0];
    assert.equal(asI64(result.body.id), 1n);
  };
  try {
    await start();
    assert.equal((await request('POST', '/setup', { name: 'Admin', email: 'admin@example.test', password: 'password123' })).res.status, 201);
    await signIn();
    const customer = await request('POST', '/customer', { company: 'Exact Co', contact: '', email: 'exact@example.test', phone: '', tier: 'Pro', seats: I64_MAX });
    assert.equal(customer.res.status, 201, customer.text);
    assert.equal(asI64(customer.body.seats), I64_MAX);
    assert.match(customer.text, /"seats":9223372036854775807/);
    const customer_id = asI64(customer.body.id)!;
    const madeProject = await request('POST', '/project', { team_id: 1n, customer_id, name: 'Exact Project', code: 'EXACT', status: 'Planned', budget: 10,
      start_day: I64_MAX, due_day: I64_MAX });
    assert.equal(madeProject.res.status, 201, madeProject.text);
    assert.equal(asI64(madeProject.body.duration), 0n);
    const project_id = asI64(madeProject.body.id)!;
    const sprint = await request('POST', '/sprint', { project_id, name: 'Exact Sprint', start_day: I64_MIN, end_day: I64_MIN + 1n });
    assert.equal(sprint.res.status, 201, sprint.text);
    assert.equal(asI64(sprint.body.length), 1n);
    const ticketInput = { customer_id, member_id: 1n, subject: 'Exact Ticket', body: 'Body', severity: 'Critical', state: 'Open',
      sla_hours: 9007199254740992n, age_hours: 9007199254740993n };
    const ticket = await request('POST', '/ticket', ticketInput);
    assert.equal(ticket.res.status, 201, ticket.text);
    assert.equal(asI64(ticket.body.age_hours), ticketInput.age_hours);
    assert.equal(ticket.body.breached, true);
    const id = asI64(ticket.body.id)!;
    const updated = await request('PUT', `/ticket/${id}`, { ...ticketInput, age_hours: ticketInput.age_hours + 1n });
    assert.equal(updated.res.status, 200, updated.text);
    const bad = await request('POST', '/ticket', stringifyJson(ticketInput).replace('"age_hours":9007199254740993', '"age_hours":9223372036854775808'), true);
    assert.equal(bad.res.status, 400, bad.text);
    const fractional = await request('POST', '/ticket', stringifyJson(ticketInput).replace('"age_hours":9007199254740993', '"age_hours":9007199254740993.1'), true);
    assert.equal(fractional.res.status, 400, fractional.text);
    await stop();
    const persisted = await readFile(join(data, 'teamdesk.json'), 'utf8');
    assert.match(persisted, /"age_hours":9007199254740994/);
    assert.match(persisted, /"start_day":-9223372036854775808/);
    await start();
    await signIn();
    const retained = await request('GET', `/ticket/${id}`);
    assert.equal(asI64(retained.body.age_hours), 9007199254740994n);
    const retainedSprint = await request('GET', '/sprint/1');
    assert.equal(asI64(retainedSprint.body.start_day), I64_MIN);
    assert.equal(asI64(retainedSprint.body.length), 1n);
    const history = await request('GET', `/ticket/${id}/history`);
    assert.equal(history.body.length, 2);
    assert.equal(asI64(history.body[1].changes.age_hours[0]), 9007199254740993n);
    assert.equal(asI64(history.body[1].changes.age_hours[1]), 9007199254740994n);
    const csv = await request('GET', '/ticket?format=csv');
    assert.match(csv.text, /9007199254740994/);
    assert.equal((await request('GET', '/ticket/9007199254740993')).res.status, 404);
    assert.equal((await request('GET', '/ticket/1.0')).res.status, 404);
  } finally {
    await stop();
    await rm(data, { recursive: true, force: true });
  }
});
