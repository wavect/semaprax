'use strict';
const crypto = require('node:crypto');
const { exact, digest, parse } = require('./protocol');
const MAX_REPORT = 16 * 1024 * 1024;
const SCHEMA = 'semaprax.project-candidate-source-review.v1';
function hash(domain, text) {
  const bytes = Buffer.from(text, 'utf8'), length = Buffer.alloc(8); length.writeBigUInt64LE(BigInt(bytes.length));
  return 'sha256:' + crypto.createHash('sha256').update(domain).update(length).update(bytes).digest('hex');
}
function canonical(value) {
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']';
  if (value && typeof value === 'object') return '{' + Object.keys(value).sort().map(key => JSON.stringify(key) + ':' + canonical(value[key])).join(',') + '}';
  return JSON.stringify(value);
}
function safePath(value) {
  return typeof value === 'string' && Buffer.byteLength(value) <= 240 && value.endsWith('.spx') &&
    !/[\\\u0000-\u001f\u007f:]/.test(value) && !value.startsWith('/') && value.split('/').every(part => part && part !== '.' && part !== '..');
}
function verify(text, candidate) {
  const value = parse(text, MAX_REPORT);
  exact(value, ['schema', 'base_project_revision', 'candidate_project_revision', 'candidate_revision', 'source_authority', 'files', 'report_revision']);
  if (value.schema !== SCHEMA || !digest(candidate) || value.candidate_revision !== candidate || value.source_authority !== false ||
      !digest(value.base_project_revision) || !digest(value.candidate_project_revision) || !digest(value.report_revision) ||
      !Array.isArray(value.files) || value.files.length > 16) throw new Error('Invalid source-review binding');
  if (canonical(value) + '\n' !== text) throw new Error('Noncanonical source-review bytes');
  const { report_revision, ...core } = value;
  if (hash(SCHEMA + '\0', canonical(core) + '\n') !== report_revision) throw new Error('Source-review digest mismatch');
  let previous;
  for (const row of value.files) {
    exact(row, ['path', 'base_source', 'candidate_source', 'base_digest', 'candidate_digest', 'source_diff', 'source_diff_digest']);
    if (!safePath(row.path) || (previous !== undefined && Buffer.compare(Buffer.from(previous), Buffer.from(row.path)) >= 0)) throw new Error('Invalid or unordered source path');
    previous = row.path;
    for (const [field, key, domain] of [
      ['base_source', 'base_digest', 'semaprax.semantic-review.source-digest.v1\0'],
      ['candidate_source', 'candidate_digest', 'semaprax.semantic-review.source-digest.v1\0'],
      ['source_diff', 'source_diff_digest', 'semaprax.candidate.source-diff.v1\0']]) {
      if (typeof row[field] !== 'string' || !digest(row[key]) || hash(domain, row[field]) !== row[key]) throw new Error('Source-review file digest mismatch');
    }
    if (row.base_source === row.candidate_source) throw new Error('Source-review contains an unchanged file');
  }
  return value;
}
async function fetchReview(call, image, candidate) {
  let offset = 0, total, chunks = [], bytes = 0;
  for (let count = 0; count < 16385; count++) {
    const response = await call('candidate/source-review', { image_revision: image, candidate_revision: candidate, offset, chunk_bytes: 65536 });
    const row = response.payload;
    exact(row, ['schema', 'report_schema', 'image_revision', 'candidate_revision', 'offset', 'total_bytes', 'chunk', 'next_offset', 'source_authority']);
    if (response.image_revision !== image || row.schema !== 'semaprax.image-source-review-chunk.v1' || row.report_schema !== SCHEMA ||
        row.image_revision !== image || row.candidate_revision !== candidate || row.offset !== offset || row.source_authority !== false ||
        !Number.isSafeInteger(row.total_bytes) || row.total_bytes < 1 || row.total_bytes > MAX_REPORT || typeof row.chunk !== 'string') throw new Error('Invalid source-review chunk');
    if (total !== undefined && total !== row.total_bytes) throw new Error('Source-review size changed'); total = row.total_bytes;
    const size = Buffer.byteLength(row.chunk); if (size < 1 || size > 65536 || bytes + size > total) throw new Error('Source-review chunk byte limit');
    chunks.push(row.chunk); bytes += size;
    if (row.next_offset === null) { if (bytes !== total) throw new Error('Truncated source review'); return verify(chunks.join(''), candidate); }
    if (!Number.isSafeInteger(row.next_offset) || row.next_offset !== bytes || row.next_offset <= offset || bytes >= total) throw new Error('Invalid source-review continuation');
    offset = row.next_offset;
  }
  throw new Error('Source-review chunk count exceeded');
}
module.exports = { verify, fetchReview, hash, canonical, safePath, MAX_REPORT, SCHEMA };
