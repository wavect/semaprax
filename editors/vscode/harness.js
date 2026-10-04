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

function summary(status, skills, updates) {
  const lines = [`Harness profile ${status.ok ? 'resolved' : 'has unmet requirements'} (lock: ${status.lock})`, `Config digest: ${status.configDigest}`];
  for (const node of tree(status)) {
    lines.push(node.label, `    ${node.detail}`);
    for (const child of node.children) lines.push(`    candidate ${child.label}`);
  }
  for (const name of status.inactive) lines.push(`Inactive extension ${name} (visible, never active)`);
  if (skills !== undefined) lines.push(...skillsLines(skills, updates));
  lines.push('Scope: only Semaprax-routed calls are observed; model choice stays host-controlled. Metrics: use Show Token Report.');
  return lines.join('\n');
}

// Default official skills: the same `skills status --json` document the CLI prints and the
// bridge returns from `bridge/skills/status` (which adds identity and delivery members).
const SKILLS_SCHEMA = 'semaprax.bridge-skills-status.v1';
const SKILL_ID = /^[a-z0-9][a-z0-9._-]{0,63}$/;
const DIGEST = /^(sha256:[0-9a-f]{64})?$/;
function parseSkillsStatus(raw) {
  if (typeof raw !== 'string' || Buffer.byteLength(raw) > 1 << 20) throw new Error('Invalid skills status size');
  const doc = JSON.parse(raw);
  if (!object(doc) || !Array.isArray(doc.skills) || (doc.schema !== undefined && doc.schema !== SKILLS_SCHEMA)) throw new Error('Unexpected skills status document');
  const skills = doc.skills.map(k => {
    if (!object(k) || !SKILL_ID.test(String(k.id)) || !text(k.mode, 64) || !text(String(k.version), 128) || !DIGEST.test(k.digest || '') || !(k.locked_revision === null || k.locked_revision === undefined || DIGEST.test(String(k.locked_revision)))) throw new Error('Invalid skill entry');
    return { id: k.id, version: String(k.version), digest: k.digest || '', defaultAvailable: k.default_available === true, selected: k.selected === true, mode: k.mode, modeSource: String(k.mode_source || ''), lockedRevision: k.locked_revision || null, disabled: k.disabled || null, omitted: k.omitted || null, applied: k.applied_to_model === true };
  });
  return {
    switchOff: doc.official_switch_off_by || null, skills, statusLines: (doc.status_lines || []).map(String),
    session: doc.session === undefined ? null : String(doc.session), project: doc.project === undefined ? null : String(doc.project),
    hostOwned: Array.isArray(doc.host_owned) ? doc.host_owned.map(h => ({ id: String(h.id), revision: String(h.revision) })) : [],
    modelRouting: doc.model_routing === undefined ? null : String(doc.model_routing), delivery: Array.isArray(doc.delivery) ? doc.delivery : []
  };
}
// Active mode and pinned revision per skill id: the facts CLI, bridge and editor must agree on.
function skillsFacts(parsed) {
  const out = {};
  for (const k of parsed.skills) out[k.id] = { mode: k.mode, lockedRevision: k.lockedRevision };
  return out;
}
function skillsAgree(a, b) {
  const x = skillsFacts(a), y = skillsFacts(b), diffs = [];
  for (const id of new Set([...Object.keys(x), ...Object.keys(y)])) {
    if (JSON.stringify(x[id]) !== JSON.stringify(y[id])) diffs.push(`${id}: ${JSON.stringify(x[id])} vs ${JSON.stringify(y[id])}`);
  }
  return { agree: diffs.length === 0, differences: diffs };
}
function parseUpdatesStatus(raw) {
  const doc = JSON.parse(raw);
  if (!object(doc) || doc.schema !== 'semaprax.updates-report.v1' || !Array.isArray(doc.sources)) throw new Error('Unexpected updates status document');
  return { offline: doc.offline === true, notice: doc.notice || null, pending: doc.sources.filter(s => s.candidate).map(s => ({ id: String(s.id), active: String(s.active || ''), candidate: String(s.candidate) })) };
}
// `skills` is a parsed status, `{ unavailable: reason }` or undefined; `updates` likewise.
function skillsLines(skills, updates) {
  if (!skills || skills.unavailable) return [`Default skills unavailable: ${skills ? skills.unavailable : 'not queried'} (this compiler may predate the default skills, or the harness refused the request)`];
  const lines = ['Default skills (official, revision-pinned per session):'];
  if (skills.switchOff) lines.push(`    all optional skills disabled by ${skills.switchOff}`);
  for (const k of skills.skills) {
    const revision = k.lockedRevision ? `pinned ${k.lockedRevision.slice(0, 19)}` : `not pinned yet (catalog ${k.digest.slice(0, 19)})`;
    lines.push(`    ${k.id} ${k.version}: ${k.defaultAvailable ? 'available' : 'unavailable'}, mode ${k.mode} (${k.modeSource || 'unknown source'}), ${revision}${k.selected ? ', active' : ''}`);
    if (k.disabled) lines.push(`        disabled: ${k.disabled}`);
  }
  for (const h of skills.hostOwned) lines.push(`    host already owns ${h.id} (${h.revision}); not injected again`);
  if (skills.modelRouting) lines.push(`    model routing: ${skills.modelRouting}`);
  if (!updates || updates.unavailable) lines.push(`    updates: unavailable${updates ? ` (${updates.unavailable})` : ''}`);
  else if (updates.pending.length === 0) lines.push('    updates: none pending');
  else for (const u of updates.pending) lines.push(`    update pending: ${u.id} ${u.active || '?'} -> ${u.candidate} (review with updates apply)`);
  return lines;
}
// Same derivation as the CLI's `skills::cli_defaults::project_id` (sha256 of the project path).
function projectId(resolvedPath) {
  return `p-${require('node:crypto').createHash('sha256').update(String(resolvedPath)).digest('hex').slice(0, 16)}`;
}
function skillsStatusArgv(project, session) {
  const id = projectId(project);
  return ['skills', 'status', '--json', '--project', id, ...(session ? ['--session', String(session)] : [])];
}
const updatesStatusArgv = () => ['updates', 'status', '--json', '--offline'];

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

module.exports = { parseSkillsStatus, skillsFacts, skillsAgree, parseUpdatesStatus, skillsLines, projectId, skillsStatusArgv, updatesStatusArgv, SKILLS_SCHEMA, parseStatus, selectedProviders, tree, summary, statusArgv, inspectArgv, enableArgv, disableArgv, command, providerId, KIND };
