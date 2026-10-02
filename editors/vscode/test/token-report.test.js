'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { validate, render } = require('../token-report');

const digest = character => 'sha256:' + character.repeat(64);
function projection(overrides = {}) {
  return {
    schema: 'semaprax.token-comparison.v1', comparison_identity: digest('a'), report_kind: 'projection', profile: 'graph',
    root_sha256: digest('b'), selection_sha256: digest('c'), source_revision: 'rev-123', producer_options_sha256: digest('d'),
    baseline: { sha256: digest('e'), utf8_bytes: 1000 }, actual: { sha256: digest('f'), utf8_bytes: 800 }, tokenizer: { name: 'cl100k_base', fingerprint: digest('1') },
    counts: { measurement_status: 'measured', baseline_tokens: 100, actual_tokens: 80, delta_tokens: 20, delta_fraction: { numerator: 20, denominator: 100 }, delta_percentage: 20 },
    baseline_kind: 'same_selected_json', actual_kind: 'compact_model-text', display_lf_in_measurement: false, compiler: {}, ...overrides
  };
}
function session(overrides = {}) {
  return {
    schema: 'semaprax.token-comparison-session.v1', comparison_identity: digest('a'), report_kind: 'session', event_stream_sha256: digest('b'), malformed_events: 1, events: 12,
    groups: [{ tokenizer: 'cl100k_base', tokenizer_fingerprint: digest('c'), boundary: 'response', reference_kind: 'paired', coverage: { events: 12, token_measured: 10, baseline_available: 8, paired: 8 }, outcomes: { ok: 10 }, statuses: { '200': 10 }, bytes: 400, tokens: 120, baseline_tokens: 100, paired_actual_tokens: 120, paired_baseline_tokens: 100 }], ...overrides
  };
}
test('the snapshot view renders a measured reduction from the shared report counts', () => {
  const text = render(validate(JSON.stringify(projection())));
  assert.match(text, /Report snapshot/i);
  assert.match(text, /Subject revision: rev-123/);
  assert.match(text, /Baseline tokens: 100/);
  assert.match(text, /20 tokens saved versus reference/);
  assert.match(text, /Tokenizer: cl100k_base/);
  assert.match(text, /Tokenizer fingerprint: sha256:1{64}/);
  assert.match(text, /Current revision not verified/);
});
test('versioned session snapshots render per-method totals and largest changes', () => {
  const value = session({
    schema: 'semaprax.token-comparison-session.v2',
    groups: [{ ...session().groups[0], methods: [
      { method: 'compact', events: 8, paired: 8, paired_actual_tokens: 120, paired_baseline_tokens: 100 },
    ], largest_reductions: [{ method: 'compact', delta_tokens: 20 }], largest_regressions: [{ method: 'context', delta_tokens: -5 }] }],
  });
  const text = render(validate(JSON.stringify(value)));
  assert.match(text, /Method totals:/);
  assert.match(text, /compact: 120 actual \/ 100 reference tokens; 8\/8 paired/);
  assert.match(text, /Largest reductions:[\s\S]*compact: 20 tokens saved/);
  assert.match(text, /Largest regressions:[\s\S]*context: \+5 tokens used/);
});
test('negative savings are words and a plus count, never a saved badge', () => {
  const value = projection({ counts: { measurement_status: 'measured', baseline_tokens: 80, actual_tokens: 100, delta_tokens: -20, delta_fraction: { numerator: -20, denominator: 80 }, delta_percentage: -25 } });
  assert.match(render(validate(JSON.stringify(value))), /\+20 tokens used versus reference/);
});
test('bytes-only reports explicitly leave model tokens unavailable', () => {
  const value = projection({ tokenizer: null, counts: { measurement_status: 'tokenizer_unavailable', baseline_tokens: null, actual_tokens: null, delta_tokens: null, delta_fraction: null, delta_percentage: null } });
  assert.match(render(validate(JSON.stringify(value))), /Model tokens unavailable; byte measurements remain separate/);
});
test('session snapshots keep paired coverage next to group totals and regressions', () => {
  const text = render(validate(JSON.stringify(session())));
  assert.match(text, /Measured pairs: 8\/12 responses/);
  assert.match(text, /Token-measured observations: 10\/12/);
  assert.match(text, /Unpaired observations: 4/);
  assert.match(text, /Partial group: only paired successful measurements/);
  assert.match(text, /\+20 tokens used versus reference/);
  assert.match(text, /Provider usage is not present/);
});
test('malformed, duplicate, and hostile report text never becomes a view', () => {
  assert.throws(() => validate('{"schema":"semaprax.token-comparison.v1","schema":"forged"}'), /Duplicate JSON key/);
  assert.throws(() => validate(JSON.stringify(projection({ source_revision: 'line\nbreak' }))), /Invalid projection token report/);
  assert.throws(() => validate(JSON.stringify(projection({ counts: { measurement_status: 'measured', baseline_tokens: 1, actual_tokens: 2, delta_tokens: 99, delta_fraction: { numerator: 99, denominator: 1 }, delta_percentage: 9900 } }))), /inconsistent signed delta/);
});
