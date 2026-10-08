import assert from 'node:assert/strict';
import test from 'node:test';
import { asFloat, asI64, I64_MAX, I64_MIN, integerEdit, JsonNumber, parseJson, routeId, stringifyJson } from '../shared/json.ts';
import { names, normalizeRow, normalizeView, validate, withComputed } from '../shared/schema.ts';

const db = { ...Object.fromEntries(names.map((name) => [name, []])), Team: [{ id: 1n }], Customer: [{ id: 1n }], Member: [{ id: 1n, team_id: 1n }], Task: [], TimeEntry: [], Expense: [], Invoice: [], Payment: [], Ticket: [] };

test('i64 decoding preserves numeric lexemes, exact exponents and range boundaries', () => {
  for (const [text, expected] of [
    ['9223372036854775807', I64_MAX], ['-9223372036854775808', I64_MIN],
    ['9007199254740993', 9007199254740993n], ['9.007199254740993e15', 9007199254740993n],
    ['9223372036854775807.0', I64_MAX], ['-9.223372036854775808e18', I64_MIN],
    ['100e-2', 1n], ['-0', 0n], ['0e999999', 0n],
  ] as const) assert.equal(asI64(parseJson(text)), expected, text);
  for (const text of ['9223372036854775808', '-9223372036854775809', '1.5', '1.0000000000000000001',
    '9223372036854775807.1', '1e999999', '1e-999999', 'true', 'null', '"123"'])
    assert.equal(asI64(parseJson(text)), undefined, text);
  assert.equal(asI64(Number('9007199254740993')), undefined);
  assert.equal(asI64(1), 1n);
  assert.equal(asFloat(parseJson('9223372036854775807')), Number('9223372036854775807'));
  assert.equal(asFloat(parseJson('1e999999')), undefined);
  assert.equal(Object.is(asFloat(parseJson(stringifyJson(-0))), -0), true);
});

test('codec is strict JSON and preserves strings, object keys, nesting and numeric tokens', () => {
  const value = parseJson('{"__proto__":{"safe":true},"text":"quoted \\\" and 🐙","items":[-0,1.25,null]}');
  assert.equal(Object.getPrototypeOf(value), Object.prototype);
  assert.equal(Object.hasOwn(value, '__proto__'), true);
  assert.equal(value.text, 'quoted " and 🐙');
  assert.equal(asI64(value.items[0]), 0n);
  assert.equal(asFloat(value.items[1]), 1.25);
  const encoded = stringifyJson({ min: I64_MIN, max: I64_MAX, unsafe: 9007199254740993n, float: 1.25 });
  assert.equal(encoded, '{"min":-9223372036854775808,"max":9223372036854775807,"unsafe":9007199254740993,"float":1.25}');
  assert.equal(asI64(parseJson(encoded).unsafe), 9007199254740993n);
  for (const invalid of ['', '01', '+1', '1.', '.1', '[1,]', '{"a":1,}', 'true false', '"bad\nstring"', '{x:1}', 'NaN'])
    assert.throws(() => parseJson(invalid), SyntaxError, invalid);
  assert.throws(() => stringifyJson(Infinity));
  assert.equal(stringifyJson([undefined, , 1n]), '[null,null,1]');
});

test('integer forms and route references remain exact without Number coercion', () => {
  assert.equal(integerEdit(I64_MIN.toString()), I64_MIN);
  assert.equal(integerEdit(I64_MAX.toString()), I64_MAX);
  for (const partial of ['', '-', '1.5', '9223372036854775808']) assert.equal(integerEdit(partial), partial);
  assert.equal(routeId('9007199254740993'), 9007199254740993n);
  for (const invalid of ['0', '-1', '1e0', '1.5', '9223372036854775808']) assert.equal(routeId(invalid), undefined);
});

test('full-range dates, ticket comparisons, mixed floats and integer rollups stay typed', () => {
  const project = normalizeRow('Project', parseJson('{"team_id":1,"customer_id":1,"name":"Project","code":"EXACT","status":"Planned","budget":10,"start_day":9223372036854775807,"due_day":9223372036854775807}'));
  assert.deepEqual(validate('Project', project, db), []);
  assert.equal(withComputed('Project', project, db).duration, 0n);
  assert.equal(typeof project.budget, 'number');
  const sprint = normalizeRow('Sprint', parseJson('{"project_id":1,"name":"Sprint","start_day":-9223372036854775808,"end_day":-9223372036854775807}'));
  const refs = { ...db, Project: [{ id: 1n }] };
  assert.deepEqual(validate('Sprint', sprint, refs), []);
  assert.equal(withComputed('Sprint', sprint, refs).length, 1n);
  assert.deepEqual(validate('Sprint', { ...sprint, end_day: I64_MAX }, refs), ['end_day - start_day must be <= 30']);
  const ticket = normalizeRow('Ticket', parseJson('{"customer_id":1,"member_id":1,"subject":"Exact","body":"ok","severity":"Critical","state":"Open","sla_hours":9007199254740992,"age_hours":9007199254740993}'));
  assert.deepEqual(validate('Ticket', ticket, db), []);
  assert.equal(withComputed('Ticket', ticket, db).breached, true);
  assert.equal(withComputed('Ticket', ticket, db).escalation, 'page');
  assert.equal(withComputed('TimeEntry', { hours: 24n, rate: 1.25, billable: true }, db).amount, 30);
  const rollups = { ...db, Task: [{ project_id: 1n, spent: 9007199254740993n }], TimeEntry: [{ member_id: 1n, hours: 24n }] };
  assert.equal(withComputed('Project', { ...project, id: 1n }, rollups).spent, 9007199254740993n);
  assert.equal(withComputed('Member', { id: 1n }, rollups).hours, 24n);
  assert.equal(withComputed('Team', { id: 1n }, db).members, 1n);
  const view = normalizeView('Project', parseJson(stringifyJson(withComputed('Project', { ...project, id: 1n }, rollups))));
  assert.equal(view.spent, 9007199254740993n);
  assert.equal(view.expenses, 0);
  assert.deepEqual(validate('Ticket', { ...ticket, age_hours: new JsonNumber('9223372036854775808') }, db), ['age_hours must be of type int']);
});


test('a mistyped integer does not hide independent validation errors', () => {
  const input = normalizeRow('Project', parseJson('{"team_id":1,"customer_id":1,"name":"x","code":"x","status":"Planned","budget":-1,"start_day":9223372036854775808,"due_day":0}'));
  assert.deepEqual(validate('Project', input, db), [
    'start_day must be of type int', 'name must be in 2..80 bytes',
    'code must be in 2..12 bytes', 'budget must be >= 0',
  ]);
});
