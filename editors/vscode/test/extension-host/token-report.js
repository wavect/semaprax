'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vscode = require('vscode');

async function run() {
  const extension = vscode.extensions.getExtension('wavect.semaprax');
  assert.ok(extension, 'development extension must be installed');
  const api = await extension.activate();
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'semaprax-token-report-host-'));
  const sha = character => 'sha256:' + character.repeat(64);
  const write = (name, value) => { const file = path.join(directory, name); fs.writeFileSync(file, typeof value === 'string' ? value : JSON.stringify(value)); return vscode.Uri.file(file); };
  const projection = counts => ({ schema: 'semaprax.token-comparison.v1', comparison_identity: sha('a'), report_kind: 'projection', profile: 'graph', root_sha256: sha('b'), selection_sha256: sha('c'), source_revision: '<script>old</script>', producer_options_sha256: sha('d'), baseline: { sha256: sha('e'), utf8_bytes: 100 }, actual: { sha256: sha('f'), utf8_bytes: 120 }, tokenizer: counts.measurement_status === 'measured' ? { name: 'cl100k_base' } : null, counts, baseline_kind: 'same_selected_json', actual_kind: 'compact_model-text', display_lf_in_measurement: false, compiler: {} });
  try {
    api.enqueueReport(write('growth.json', projection({ measurement_status: 'measured', baseline_tokens: 10, actual_tokens: 12, delta_tokens: -2, delta_fraction: { numerator: -2, denominator: 10 }, delta_percentage: -20 })));
    await api.execute('showTokenReport');
    assert.match(vscode.window.activeTextEditor.document.getText(), /\+2 tokens used versus reference/);
    assert.match(vscode.window.activeTextEditor.document.getText(), /<script>old<\/script>/);
    api.enqueueReport(write('unavailable.json', projection({ measurement_status: 'tokenizer_unavailable', baseline_tokens: null, actual_tokens: null, delta_tokens: null, delta_fraction: null, delta_percentage: null })));
    await api.execute('showTokenReport'); assert.match(vscode.window.activeTextEditor.document.getText(), /Model tokens unavailable/);
    api.enqueueReport(write('partial.json', { schema: 'semaprax.token-comparison-session.v1', comparison_identity: sha('a'), report_kind: 'session', event_stream_sha256: sha('b'), malformed_events: 0, events: 3, groups: [{ tokenizer: 'cl100k_base', tokenizer_fingerprint: sha('c'), boundary: 'response', reference_kind: 'paired', coverage: { events: 3, token_measured: 2, baseline_available: 2, paired: 1 }, outcomes: { success: 1, error: 1 }, statuses: { measured: 2 }, bytes: 20, tokens: 999, baseline_tokens: 999, paired_actual_tokens: 8, paired_baseline_tokens: 10 }] }));
    await api.execute('showTokenReport'); assert.match(vscode.window.activeTextEditor.document.getText(), /Measured pairs: 1\/3 responses/);
    api.enqueueReport(write('malformed.json', '{"schema":"x","schema":"y"}')); await assert.rejects(api.execute('showTokenReport'), /Duplicate JSON key/);
    assert.equal(api.state().running, false, 'snapshot view must not start a session');
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
}
module.exports = { run };
