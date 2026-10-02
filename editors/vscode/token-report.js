'use strict';
// Offline token-report reader and renderer.  This module deliberately has no
// VS Code, process, network, or filesystem dependency: extension.js supplies
// an explicitly selected, bounded local file as text.
const { parse, exact } = require('./protocol');

const MAX_REPORT_BYTES = 16 * 1024 * 1024;
const COMPARISON_SCHEMA = 'semaprax.token-comparison.v1';
const SESSION_SCHEMA = 'semaprax.token-comparison-session.v1';
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const text = (value, limit = 4096) => typeof value === 'string' && Buffer.byteLength(value) <= limit && !/[\u0000-\u001f\u007f]/.test(value);
const count = value => Number.isSafeInteger(value) && value >= 0;
const nullableCount = value => value === null || count(value);
const digest = value => typeof value === 'string' && DIGEST.test(value);

function object(value) { return value && typeof value === 'object' && !Array.isArray(value); }
function fact(value) {
  exact(value, ['sha256', 'utf8_bytes']);
  if (!digest(value.sha256) || !count(value.utf8_bytes)) throw new Error('Invalid token report byte fact');
}
function counts(value) {
  exact(value, ['measurement_status', 'baseline_tokens', 'actual_tokens', 'delta_tokens', 'delta_fraction', 'delta_percentage']);
  if (!['measured', 'tokenizer_unavailable'].includes(value.measurement_status) ||
      !nullableCount(value.baseline_tokens) || !nullableCount(value.actual_tokens) ||
      !(value.delta_tokens === null || Number.isSafeInteger(value.delta_tokens)) ||
      !(value.delta_percentage === null || (typeof value.delta_percentage === 'number' && Number.isFinite(value.delta_percentage)))) {
    throw new Error('Invalid token report counts');
  }
  if (value.measurement_status === 'tokenizer_unavailable') {
    if ([value.baseline_tokens, value.actual_tokens, value.delta_tokens, value.delta_fraction, value.delta_percentage].some(item => item !== null)) throw new Error('Unavailable tokenizer report carries token counts');
    return;
  }
  if (value.baseline_tokens === null || value.actual_tokens === null || value.delta_tokens === null || value.delta_tokens !== value.baseline_tokens - value.actual_tokens) throw new Error('Measured token report has inconsistent signed delta');
  if (value.baseline_tokens === 0) {
    if (value.delta_fraction !== null || value.delta_percentage !== null) throw new Error('Zero-baseline token report has a percentage');
  } else {
    exact(value.delta_fraction, ['numerator', 'denominator']);
    if (!Number.isSafeInteger(value.delta_fraction.numerator) || value.delta_fraction.denominator !== value.baseline_tokens || value.delta_fraction.numerator !== value.delta_tokens || value.delta_percentage === null) throw new Error('Measured token report has inconsistent fraction');
  }
}
function tokenizer(value) {
  if (value === null) return;
  if (!object(value) || Object.keys(value).length > 16 || Object.entries(value).some(([key, item]) => !text(key, 128) || !(item === null || typeof item === 'boolean' || typeof item === 'number' || text(item, 4096)))) throw new Error('Invalid token report tokenizer metadata');
}
function projection(value) {
  if (value.report_kind === 'projection') {
    exact(value, ['schema', 'comparison_identity', 'report_kind', 'profile', 'root_sha256', 'selection_sha256', 'source_revision', 'producer_options_sha256', 'baseline', 'actual', 'tokenizer', 'counts', 'baseline_kind', 'actual_kind', 'display_lf_in_measurement', 'compiler']);
    if (!text(value.profile, 64) || !digest(value.root_sha256) || !digest(value.selection_sha256) || !text(value.source_revision, 4096) || !digest(value.producer_options_sha256) || !text(value.baseline_kind, 128) || !text(value.actual_kind, 128) || typeof value.display_lf_in_measurement !== 'boolean' || !object(value.compiler)) throw new Error('Invalid projection token report');
  } else if (value.report_kind === 'compare') {
    exact(value, ['schema', 'comparison_identity', 'report_kind', 'reference_kind', 'equivalence', 'baseline', 'actual', 'tokenizer', 'counts']);
    if (!text(value.reference_kind, 128) || !text(value.equivalence, 128)) throw new Error('Invalid comparison token report');
  } else throw new Error('Unknown token comparison report kind');
  if (value.schema !== COMPARISON_SCHEMA || !digest(value.comparison_identity)) throw new Error('Invalid token comparison report identity');
  fact(value.baseline); fact(value.actual); tokenizer(value.tokenizer); counts(value.counts);
}
function sessionGroup(value) {
  exact(value, ['tokenizer', 'tokenizer_fingerprint', 'boundary', 'reference_kind', 'coverage', 'outcomes', 'statuses', 'bytes', 'tokens', 'baseline_tokens', 'paired_actual_tokens', 'paired_baseline_tokens']);
  if (![value.tokenizer, value.tokenizer_fingerprint, value.boundary, value.reference_kind].every(item => item === null || text(item, 4096)) || !count(value.bytes) || !count(value.tokens) || !count(value.baseline_tokens) || !count(value.paired_actual_tokens) || !count(value.paired_baseline_tokens) || !object(value.coverage) || !object(value.outcomes) || !object(value.statuses)) throw new Error('Invalid session token-report group');
  exact(value.coverage, ['events', 'token_measured', 'baseline_available', 'paired']);
  if (![value.coverage.events, value.coverage.token_measured, value.coverage.baseline_available, value.coverage.paired].every(count) || value.coverage.token_measured > value.coverage.events || value.coverage.baseline_available > value.coverage.events || value.coverage.paired > value.coverage.token_measured || value.coverage.paired > value.coverage.baseline_available) throw new Error('Invalid session token-report coverage');
  for (const table of [value.outcomes, value.statuses]) if (Object.entries(table).some(([key, item]) => !text(key, 128) || !count(item))) throw new Error('Invalid session token-report table');
}
function session(value) {
  exact(value, ['schema', 'comparison_identity', 'report_kind', 'event_stream_sha256', 'groups', 'malformed_events', 'events']);
  if (value.schema !== SESSION_SCHEMA || value.report_kind !== 'session' || !digest(value.comparison_identity) || !digest(value.event_stream_sha256) || !Array.isArray(value.groups) || value.groups.length > 1024 || !count(value.malformed_events) || !count(value.events)) throw new Error('Invalid session token report');
  value.groups.forEach(sessionGroup);
}
function validate(textValue) {
  const value = parse(textValue, MAX_REPORT_BYTES, true);
  if (!object(value)) throw new Error('Token report must be a JSON object');
  if (value.schema === COMPARISON_SCHEMA) projection(value);
  else if (value.schema === SESSION_SCHEMA) session(value);
  else throw new Error('Unsupported token report schema');
  return value;
}
function line(label, value) { return `${label}: ${String(value)}\n`; }
function signed(countsValue) {
  if (countsValue.measurement_status !== 'measured') return 'Model tokens unavailable.';
  if (countsValue.delta_tokens > 0) return `${countsValue.delta_tokens} tokens saved versus reference.`;
  if (countsValue.delta_tokens < 0) return `+${-countsValue.delta_tokens} tokens used versus reference.`;
  return 'No token difference versus reference.';
}
function renderComparison(value) {
  const out = ['SEMAPRAX token report snapshot\n', 'Current revision not verified. This local report is not live monitoring or a billed counter.\n\n'];
  out.push(line('Comparison type', value.report_kind === 'projection' ? value.baseline_kind : value.reference_kind));
  if (value.report_kind === 'projection') {
    out.push(line('Subject revision', value.source_revision));
    out.push(line('Measured boundary', value.actual_kind));
  }
  out.push(line('Baseline payload bytes', value.baseline.utf8_bytes));
  out.push(line('Actual payload bytes', value.actual.utf8_bytes));
  if (value.counts.measurement_status === 'measured') {
    out.push(line('Baseline tokens', value.counts.baseline_tokens));
    out.push(line('Actual payload tokens', value.counts.actual_tokens));
    out.push(signed(value.counts) + '\n');
    if (value.counts.delta_percentage !== null) out.push(line('Reported percentage', `${value.counts.delta_percentage}%`));
  } else out.push('Model tokens unavailable; byte measurements remain separate.\n');
  out.push('\nProvider usage is not present in this report.\n');
  return out.join('');
}
function renderSession(value) {
  const out = ['SEMAPRAX session token report snapshot\n', 'Current revision not verified. This local report is not live monitoring or a billed counter.\n\n', line('Observed events', value.events), line('Malformed events excluded', value.malformed_events), '\nGrouped measurements\n'];
  if (!value.groups.length) out.push('No measured groups.\n');
  for (const [position, group] of value.groups.entries()) {
    out.push(`\nGroup ${position + 1}\n`);
    out.push(line('Tokenizer', group.tokenizer === null ? 'model tokens unavailable' : group.tokenizer));
    out.push(line('Tokenizer fingerprint', group.tokenizer_fingerprint === null ? 'unavailable' : group.tokenizer_fingerprint));
    out.push(line('Measured boundary', group.boundary === null ? 'unavailable' : group.boundary));
    out.push(line('Comparison type', group.reference_kind === null ? 'unavailable' : group.reference_kind));
    out.push(line('Measured pairs', `${group.coverage.paired}/${group.coverage.events} responses`));
    out.push(line('Token-measured observations', `${group.coverage.token_measured}/${group.coverage.events}`));
    out.push(line('Unpaired observations', group.coverage.events - group.coverage.paired));
    out.push(line('Outcome counts', Object.entries(group.outcomes).sort(([a], [b]) => a.localeCompare(b)).map(([key, count]) => `${key}=${count}`).join(', ') || 'none'));
    out.push(line('Status counts', Object.entries(group.statuses).sort(([a], [b]) => a.localeCompare(b)).map(([key, count]) => `${key}=${count}`).join(', ') || 'none'));
    if (group.coverage.paired !== group.coverage.events) out.push('Partial group: only paired successful measurements contribute to its reduction.\n');
    if (group.tokenizer === null || group.coverage.paired === 0) out.push('Paired token reduction unavailable for this group.\n');
    else {
      out.push(line('Paired actual payload tokens', group.paired_actual_tokens));
      out.push(line('Paired reference tokens', group.paired_baseline_tokens));
      const delta = group.paired_baseline_tokens - group.paired_actual_tokens;
      out.push(delta > 0 ? `${delta} tokens saved versus reference.\n` : delta < 0 ? `+${-delta} tokens used versus reference.\n` : 'No token difference versus reference.\n');
    }
    out.push(line('Payload bytes', group.bytes));
  }
  out.push('\nProvider usage is not present in this report.\n');
  return out.join('');
}
function render(value) { return value.schema === COMPARISON_SCHEMA ? renderComparison(value) : renderSession(value); }
module.exports = { MAX_REPORT_BYTES, validate, render, signed };
