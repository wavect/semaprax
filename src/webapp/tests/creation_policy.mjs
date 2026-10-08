import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
const { entities } = await import(pathToFileURL(join(process.argv[2], 'schema.js')));
const entity = path => entities.find(e => e.path === path);
const preview = (path, u) => entity(path).canWrite.create(u);
const viewer = { id: 1n, role: 'Viewer', active: true, divisor: 0n };
const agent = { ...viewer, role: 'Agent', divisor: 1n };
const admin = { ...agent, role: 'Admin' };
const guarded = ['owned','reverse','conditional','match_own','guarded','local','textual',
  'negated','literal_match','bound_match','requires','requires_role','ensures_role',
  'ensures_unknown','default_item'];
for (const path of guarded) {
  assert.equal(preview(path, viewer), false, `${path}: definitive Viewer refusal`);
  assert.equal(preview(path, agent), null, `${path}: an Agent's unknown row must remain unknown`);
}
assert.equal(preview('match_own', admin), true);
assert.equal(preview('disjoin', viewer), null);
assert.equal(preview('disjoin', admin), true);
assert.equal(preview('row_only', viewer), null, 'no invented zero/default row');
assert.equal(preview('unknown_trap', viewer), null, 'row-dependent division is unknown');
assert.equal(preview('trap', viewer), false, 'known division trap cannot permit any row');
assert.equal(preview('trap', agent), null);
assert.equal(preview('not_trap', viewer), false, 'NOT does not invert a trap into permission');
assert.equal(preview('not_trap', agent), null);
const known = entity('account_only').canWrite;
assert.equal(known.row, false);
assert.equal(known.create, undefined, 'account-only metadata retains its exact test');
assert.equal(known.test({}, viewer), false);
assert.equal(known.test({}, agent), true);
assert.equal(preview('owned', {}), null, 'missing account input is not coerced to false');

// Every definitive refusal is checked against the original authoritative
// function on valid concrete values, including errors and boundary arithmetic.
let refused = 0;
for (const u of [viewer, agent, admin]) {
  for (const e of entities.filter(e => e.canWrite?.create)) {
    const abstract = e.canWrite.create(u);
    assert.ok(abstract === null || typeof abstract === 'boolean', e.path);
    for (const owner of [1n, 2n]) for (const score of [-9223372036854775808n, -1n, 0n, 1n, 9223372036854775807n])
      for (const state of ['Draft','Approved']) for (const label of ['','é']) {
        const row = { owner, score, state, label };
        let allowed = false;
        try { allowed = e.canWrite.test(row, u) === true; } catch { /* checked failure denies */ }
        if (abstract === false) {
          refused++;
          assert.equal(allowed, false, `${e.path}: false preview must never hide a permitted concrete row`);
        }
      }
  }
}
assert.ok(refused > 500);
console.log('creation-policy: role restrictions, unknown fields, helpers, match, contracts and trap soundness pass');
