'use strict';
// Pure helpers for the harness provider profile.  The editor is a client of the
// same `semaprax harness status --json` contract as the CLI and external hosts;
// this module has no VS Code, process or filesystem dependency.  Metrics live in
// the existing token-report surface (token-report.js), not here.
const STATUS_SCHEMA = 'semaprax.harness-status.v1';
const PROVIDER_ID = /^[a-z0-9._-]+\/[a-z0-9._-]+$/;
const KIND = /^[a-z][a-z0-9._-]*(\/v\d+)?$/;
const text = (value, limit = 4096) => typeof value === 'string' && Buffer.byteLength(value) <= limit && !/[\u0000-\u001f\u007f]/.test(value);
const object = value => value && typeof value === 'object' && !Array.isArray(value);

function parseStatus(raw) {
  if (typeof raw !== 'string' || Buffer.byteLength(raw) > 1 << 20) throw new Error('Invalid harness status size');
  const doc = JSON.parse(raw);
  if (!object(doc) || doc.schema !== STATUS_SCHEMA || !Array.isArray(doc.bindings) || typeof doc.ok !== 'boolean') throw new Error('Unexpected harness status document');
  const bindings = doc.bindings.map(binding => {
    if (!object(binding) || !text(binding.kind, 128) || !text(binding.state, 64) || !text(binding.provider_id, 128) || !text(binding.provider_version, 128) || !text(binding.reason) || !Array.isArray(binding.candidates)) throw new Error('Invalid harness binding');
    return {
      kind: binding.kind, state: binding.state, providerId: binding.provider_id, providerVersion: binding.provider_version, reason: binding.reason,
      candidates: binding.candidates.map(c => ({ providerId: String(c.provider_id), verdict: String(c.verdict), detail: String(c.detail) }))
    };
  });
  return { ok: doc.ok, lock: String(doc.lock), configDigest: String(doc.config_digest), inactive: (doc.inactive || []).map(String), bindings };
}

// Selected provider per capability kind; an unbound kind has no entry.
function selectedProviders(status) {
  const out = {};
  for (const binding of status.bindings) if (binding.providerId) out[binding.kind] = binding.providerId;
  return out;
}

function tree(status) {
  return status.bindings.map(binding => ({
    label: `${binding.kind}: ${binding.state}${binding.providerId ? ` · ${binding.providerId}@${binding.providerVersion}` : ''}`,
    detail: binding.reason,
    children: binding.candidates.map(c => ({ label: `${c.providerId}: ${c.verdict}`, detail: c.detail, children: [] }))
  }));
}

function summary(status) {
  const lines = [`Harness profile ${status.ok ? 'resolved' : 'has unmet requirements'} (lock: ${status.lock})`, `Config digest: ${status.configDigest}`];
  for (const node of tree(status)) {
    lines.push(node.label, `    ${node.detail}`);
    for (const child of node.children) lines.push(`    candidate ${child.label}`);
  }
  for (const name of status.inactive) lines.push(`Inactive extension ${name} (visible, never active)`);
  lines.push('Scope: only Semaprax-routed calls are observed; model choice stays host-controlled. Metrics: use Show Token Report.');
  return lines.join('\n');
}

function providerId(value) {
  if (typeof value !== 'string' || value.length > 128 || !PROVIDER_ID.test(value) || value.split('/').some(part => part.startsWith('-') || part.startsWith('.'))) throw new Error('Invalid provider id');
  return value;
}
// Argv (after the harness executable) for each action; run with shell:false.
const statusArgv = project => ['status', '--json', '--project', String(project)];
const inspectArgv = id => ['inspect', providerId(id)];
const enableArgv = id => ['trust', providerId(id)];   // the user's approval of an adopted provider
const disableArgv = id => ['revoke', providerId(id)]; // takes effect on the next call

// `compiler` is the full toolchain executable, which forwards `harness <verb>`.
function command(compiler, argv) { return { file: compiler, args: ['harness', ...argv] }; }

module.exports = { parseStatus, selectedProviders, tree, summary, statusArgv, inspectArgv, enableArgv, disableArgv, command, providerId, KIND };
