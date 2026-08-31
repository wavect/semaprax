'use strict';
// Authored evidence only; not executed during implementation.
const test = require('node:test');
const assert = require('node:assert/strict');
const { verify, fetchReview, hash, canonical, safePath, SCHEMA } = require('../review');
const D = 'sha256:' + 'a'.repeat(64);
function fixture() {
  const row = { path: 'src/core.spx', base_source: 'module demo;\n', candidate_source: 'module demo;\n// é\n', source_diff: '--- base/src/core.spx\n+++ candidate/src/core.spx\n+// é\n' };
  row.base_digest = hash('semaprax.semantic-review.source-digest.v1\0', row.base_source);
  row.candidate_digest = hash('semaprax.semantic-review.source-digest.v1\0', row.candidate_source);
  row.source_diff_digest = hash('semaprax.candidate.source-diff.v1\0', row.source_diff);
  return { schema: SCHEMA, base_project_revision: D, candidate_project_revision: D, candidate_revision: D, source_authority: false, files: [row] };
}
function seal(core) { return canonical({ ...core, report_revision: hash(SCHEMA + '\0', canonical(core) + '\n') }) + '\n'; }
test('canonical review binds whole report and each exact source/diff byte string', () => {
  const core = fixture(), text = seal(core); assert.equal(verify(text, D).files[0].candidate_source, core.files[0].candidate_source);
  assert.throws(() => verify(text.trim(), D), /canonical/);
  assert.throws(() => verify(text, 'sha256:' + 'b'.repeat(64)), /binding/);
  assert.throws(() => verify(text.replace('module demo', 'module evil'), D), /digest/);
  for (const field of ['base_source', 'candidate_source', 'source_diff']) {
    const modified = structuredClone(core); modified.files[0][field] += 'tamper';
    assert.throws(() => verify(seal(modified), D), /file digest/);
  }
  const authority = structuredClone(core); authority.source_authority = true;
  assert.throws(() => verify(seal(authority), D), /binding/);
  const unknown = structuredClone(core); unknown.files[0].absolute_path = '/etc/passwd';
  assert.throws(() => verify(seal(unknown), D), /fields/);
});
test('canonical relative paths and changed-only sorted inventory reject filesystem tricks', () => {
  for (const path of ['/tmp/file.spx', '../file.spx', 'src/../file.spx', 'src//file.spx', 'C:\\file.spx', 'a\0.spx', 'src/file.rs']) assert.equal(safePath(path), false, path);
  for (const path of ['src/core.spx', 'module.spx']) assert.equal(safePath(path), true);
  const core = fixture(); core.files.push(structuredClone(core.files[0]));
  assert.throws(() => verify(seal(core), D), /unordered/);
  const oversized = fixture(); oversized.files = Array.from({ length: 17 }, () => structuredClone(oversized.files[0]));
  assert.throws(() => verify(seal(oversized), D), /binding/);
});
test('chunk assembly is byte-bound, exact-revision-bound and rejects skipped/truncated continuations', async () => {
  const text = seal(fixture()), bytes = Buffer.from(text), split = bytes.indexOf(Buffer.from('é'));
  const parts = [bytes.subarray(0, split).toString(), bytes.subarray(split).toString()];
  const calls = [];
  const call = async (method, params) => {
    calls.push(params.offset); const index = params.offset === 0 ? 0 : 1;
    return { image_revision: D, payload: { schema: 'semaprax.image-source-review-chunk.v1', report_schema: SCHEMA, image_revision: D, candidate_revision: D, offset: params.offset, total_bytes: bytes.length, chunk: parts[index], next_offset: index === 0 ? Buffer.byteLength(parts[0]) : null, source_authority: false } };
  };
  assert.equal((await fetchReview(call, D, D)).files.length, 1);
  assert.deepEqual(calls, [0, Buffer.byteLength(parts[0])]);
  await assert.rejects(fetchReview(async (method, params) => { const row = await call(method, params); row.payload.next_offset = null; return row; }, D, D), /Truncated/);
  await assert.rejects(fetchReview(async (method, params) => { const row = await call(method, params); row.payload.next_offset = 1; return row; }, D, D), /continuation/);
  await assert.rejects(fetchReview(async (method, params) => { const row = await call(method, params); row.payload.image_revision = 'sha256:' + 'b'.repeat(64); return row; }, D, D), /chunk/);
});
