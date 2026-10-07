#!/usr/bin/env node
// External API acceptance for the Agent own-row permission requirements.
// Run only against an isolated, disposable TeamDesk server; it creates data.

import assert from 'node:assert/strict';

const ENTITY_PATHS = [
  'team', 'member', 'customer', 'contact', 'project', 'milestone', 'sprint', 'task',
  'comment', 'timeentry', 'ticket', 'ticketreply', 'invoice', 'payment', 'vendor',
  'expense', 'asset', 'leave', 'document', 'release',
];
const TEST_PASSWORD = 'permission-test-only-605';
const evidence = [];

function options(argv) {
  const result = {};
  for (let i = 0; i < argv.length; i += 1) {
    const key = argv[i];
    if (!['--base-url', '--arm'].includes(key) || !argv[i + 1]) {
      throw new Error('usage: permission-api-self-test.mjs --arm semaprax|typescript --base-url http://127.0.0.1:PORT');
    }
    result[key.slice(2)] = argv[++i];
  }
  if (!['semaprax', 'typescript'].includes(result.arm) || !result['base-url']) {
    throw new Error('usage: permission-api-self-test.mjs --arm semaprax|typescript --base-url http://127.0.0.1:PORT');
  }
  return result;
}

const { arm, 'base-url': rawBase } = options(process.argv.slice(2));
const base = new URL(rawBase);
const apiBase = new URL('/api/', base).href;

async function call(method, path, body, cookie) {
  const headers = {};
  if (body !== undefined) headers['content-type'] = 'application/json';
  if (cookie) headers.cookie = cookie;
  const response = await fetch(new URL(path, apiBase), {
    method,
    headers,
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const text = await response.text();
  let json = null;
  try { json = text ? JSON.parse(text) : null; } catch {}
  return { status: response.status, text, json, cookie: response.headers.get('set-cookie') ?? '' };
}

function expectStatus(label, response, expected) {
  assert.equal(response.status, expected, `${label}: expected HTTP ${expected}, got ${response.status}: ${response.text}`);
  evidence.push(`${label} ${response.status}`);
  return response.json;
}

async function create(path, body, cookie) {
  const response = await call('POST', path, body, cookie);
  const row = expectStatus(`POST ${path}`, response, 201);
  assert.ok(row && row.id !== undefined, `POST ${path}: response has no id`);
  return row;
}

async function get(path, cookie, expected = 200) {
  const response = await call('GET', path, undefined, cookie);
  return expectStatus(`GET ${path}`, response, expected);
}

async function login(email) {
  const response = await call('POST', 'session', {
    [arm === 'semaprax' ? 'login' : 'email']: email,
    password: TEST_PASSWORD,
  });
  const member = expectStatus(`POST session ${email}`, response, 200);
  const cookie = response.cookie.split(';', 1)[0];
  assert.match(cookie, /^sid=.+$/, 'sign-in did not return a session cookie');
  assert.ok(member && member.id !== undefined, 'sign-in response has no member id');
  return { member, cookie };
}

async function emptyRows(cookie, expectedTeams, expectedMembers) {
  for (const path of ENTITY_PATHS) {
    const rows = await get(path, cookie);
    assert.ok(Array.isArray(rows), `GET ${path} did not return a row list`);
    const expected = path === 'team' ? expectedTeams : path === 'member' ? expectedMembers : 0;
    assert.equal(rows.length, expected, `fresh database required: ${path} contains ${rows.length} rows`);
  }
}

async function bootstrap() {
  if (arm === 'typescript') {
    const status = await call('GET', 'setup');
    expectStatus('GET setup', status, 200);
    assert.equal(status.json?.needed, true, 'TypeScript setup endpoint is not in first-run state');
    const response = await call('POST', 'setup', {
      name: 'Permission Test Admin', email: 'permission-admin@example.test', password: TEST_PASSWORD,
    });
    const admin = expectStatus('POST setup first admin', response, 201);
    assert.ok(admin && admin.team_id !== undefined, 'setup response omitted the test Team reference');
    const { cookie } = await login('permission-admin@example.test');
    await emptyRows(cookie, 1, 1);
    return { admin, cookie };
  }

  const setup = await call('GET', 'session');
  expectStatus('GET session setup state', setup, 200);
  assert.equal(setup.json?.setup, true, 'SEMAPRAX server must run with --setup and an empty data directory');
  await emptyRows(undefined, 0, 0);
  const team = await create('team', { name: 'Permission Test Team', description: 'Disposable test data' });
  const admin = await create('member', {
    team_id: team.id, name: 'Permission Test Admin', email: 'permission-admin@example.test',
    role: 'Admin', active: true,
  });
  const setPassword = await call('PUT', `member/${admin.id}`, {
    team_id: team.id, name: 'Permission Test Admin', email: 'permission-admin@example.test',
    role: 'Admin', active: true, password: TEST_PASSWORD,
  });
  expectStatus('PUT first admin password in setup mode', setPassword, 200);
  const { cookie } = await login('permission-admin@example.test');
  await emptyRows(cookie, 1, 1);
  return { admin, team, cookie };
}

async function main() {
  const { team, cookie: adminCookie } = await bootstrap();
  const adminRows = await get('member', adminCookie);
  const admin = adminRows.find(row => row.email === 'permission-admin@example.test');
  assert.ok(admin, 'could not find the bootstrap Admin row');
  const teamId = team?.id ?? admin.team_id;
  assert.ok(teamId !== undefined, 'could not identify the test Team');

  const agents = [];
  for (const [name, email] of [
    ['Agent Owner A', 'permission-agent-a@example.test'],
    ['Agent Owner B', 'permission-agent-b@example.test'],
  ]) {
    agents.push(await create('member', {
      team_id: teamId, name, email, role: 'Agent', active: true, password: TEST_PASSWORD,
    }, adminCookie));
  }

  const customer = await create('customer', {
    company: 'Permission Test Customer', contact: 'Test Contact', email: 'permission-customer@example.test',
    phone: '000', tier: 'Free', seats: 1,
  }, adminCookie);
  const project = await create('project', {
    team_id: teamId, customer_id: customer.id, name: 'Permission Test Project', code: 'PERM605',
    status: 'Active', budget: 1000, start_day: 1, due_day: 30,
  }, adminCookie);
  const milestone = await create('milestone', {
    project_id: project.id, title: 'Permission Test Milestone', due_day: 20, done: false,
  }, adminCookie);
  const sprint = await create('sprint', {
    project_id: project.id, name: 'Permission Test Sprint', start_day: 1, end_day: 14,
  }, adminCookie);
  const vendor = await create('vendor', {
    name: 'Permission Test Vendor', email: 'permission-vendor@example.test',
  }, adminCookie);

  const owners = await Promise.all([
    login('permission-agent-a@example.test'),
    login('permission-agent-b@example.test'),
  ]);
  assert.notEqual(String(owners[0].member.id), String(owners[1].member.id), 'test accounts must be distinct');
  const [a, b] = owners;
  const makeRows = async (member, cookie, prefix) => {
    const taskInput = {
      project_id: project.id, milestone_id: milestone.id, sprint_id: sprint.id, member_id: member.id,
      title: `${prefix} task`, details: 'Permission test task', priority: 'Medium', status: 'Todo',
      estimate: 8, spent: 1,
    };
    const expenseInput = {
      project_id: project.id, vendor_id: vendor.id, member_id: member.id,
      description: `${prefix} expense`, amount: 12, day: 2, state: 'Draft',
    };
    const leaveInput = {
      member_id: member.id, kind: 'Vacation', state: 'Requested', start_day: 3, end_day: 4,
    };
    return {
      task: await create('task', taskInput, cookie), taskInput,
      expense: await create('expense', expenseInput, cookie), expenseInput,
      leave: await create('leave', leaveInput, cookie), leaveInput,
    };
  };
  const own = await makeRows(a.member, a.cookie, 'A');
  const other = await makeRows(b.member, b.cookie, 'B');
  for (const rows of [own, other]) {
    for (const kind of ['task', 'expense', 'leave']) {
      assert.equal(String(rows[kind].member_id), String(rows === own ? a.member.id : b.member.id),
        `${kind} row was not created for its designated Agent`);
    }
  }

  for (const kind of ['task', 'expense', 'leave']) {
    await get(`${kind}/${own[kind].id}`, a.cookie);
  }
  await get(`expense/${other.expense.id}`, a.cookie, 404);
  // Agents can read other Tasks and Leave rows; the ownership rule denies write.
  await get(`task/${other.task.id}`, a.cookie);
  await get(`leave/${other.leave.id}`, a.cookie);

  const ownTask = { ...own.taskInput, title: 'A own task updated' };
  expectStatus('PUT own Task', await call('PUT', `task/${own.task.id}`, ownTask, a.cookie), 200);
  const ownExpense = { ...own.expenseInput, amount: 13 };
  expectStatus('PUT own Expense', await call('PUT', `expense/${own.expense.id}`, ownExpense, a.cookie), 200);
  const ownLeave = { ...own.leaveInput, end_day: 5 };
  expectStatus('PUT own Leave', await call('PUT', `leave/${own.leave.id}`, ownLeave, a.cookie), 200);

  expectStatus('PUT other Agent Task denied', await call('PUT', `task/${other.task.id}`, {
    ...other.taskInput, title: 'A must not edit B task',
  }, a.cookie), 403);
  expectStatus('PUT other Agent Expense hidden/denied', await call('PUT', `expense/${other.expense.id}`, {
    ...other.expenseInput, amount: 999,
  }, a.cookie), 404);
  expectStatus('PUT other Agent Leave denied', await call('PUT', `leave/${other.leave.id}`, {
    ...other.leaveInput, end_day: 8,
  }, a.cookie), 403);

  assert.equal((await get(`task/${other.task.id}`, b.cookie)).title, other.task.title,
    'denied Task update changed the other owner row');
  assert.equal((await get(`expense/${other.expense.id}`, b.cookie)).amount, other.expense.amount,
    'denied Expense update changed the other owner row');
  assert.equal((await get(`leave/${other.leave.id}`, b.cookie)).end_day, other.leave.end_day,
    'denied Leave update changed the other owner row');
  assert.equal((await get(`task/${own.task.id}`, a.cookie)).title, ownTask.title,
    'own Task update was not stored');
  assert.equal((await get(`expense/${own.expense.id}`, a.cookie)).amount, ownExpense.amount,
    'own Expense update was not stored');
  assert.equal((await get(`leave/${own.leave.id}`, a.cookie)).end_day, ownLeave.end_day,
    'own Leave update was not stored');

  console.log(`external API permission self-test: passed (${evidence.length} recorded HTTP observations, ${arm}); distinct Agent owners, own read/update, other-row denial; not browser automation or built-in --self-test`);
  for (const row of evidence) console.log(`permission evidence: ${row}`);
}

main().catch(error => {
  console.error(`external API permission self-test failed (${arm}): ${error.message}`);
  process.exitCode = 1;
});
