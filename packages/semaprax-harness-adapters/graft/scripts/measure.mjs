#!/usr/bin/env node
// Cold/warm/adoption measurement on a host-code corpus: node scripts/measure.mjs <corpus-dir> <graft> [<graft>...]
// Wall time per adapter call, with verification cost kept apart from index construction. Native baseline: rg.
import { execFileSync } from 'node:child_process';
import { cpSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ADOPT, Adapter, buildUserIndex, tmp, versionOf, GIT } from '../test/helpers.mjs';

const [corpus, ...bins] = process.argv.slice(2);
const timed = async (f) => { const t = process.hrtime.bigint(); const r = await f(); return [Number(process.hrtime.bigint() - t) / 1e6, r]; };
const row = (label, ms, r) => {
  const m = r.payload?.metadata?.refresh;
  console.log(`| ${label} | ${ms.toFixed(0)} | ${m?.outcome ?? r.status} | ${m ? `verify ${m.verification_ms} / index ${m.index_ms} / copy ${m.copied_bytes}B` : '-'} | ${r.payload?.items?.length ?? '-'} |`);
};
console.log('| step | wall ms | outcome | work split | items |\n| --- | ---: | --- | --- | ---: |');
for (const bin of bins) {
  Adapter.upstream = bin;
  const v = versionOf(bin);
  const mk = () => {
    const root = join(tmp('corpus'), 'p'); cpSync(corpus, root, { recursive: true });
    const g = (...a) => execFileSync(GIT, ['-C', root, '-c', 'user.email=t@t', '-c', 'user.name=t', ...a], { stdio: 'ignore' });
    g('init', '-q'); g('add', '-A'); g('commit', '-qm', 'i'); return root;
  };
  const q = { query: 'span_digest', mode: 'exact' };
  // owned cache
  let root = mk(); let a = new Adapter({ root, cache: tmp('c') });
  let [ms, r] = await timed(() => a.call('search', q)); row(`${v} cold (owned build)`, ms, r);
  [ms, r] = await timed(() => a.call('search', q)); row(`${v} warm (owned reuse)`, ms, r);
  const file = readdirSyncFirstRs(root);
  writeFileSync(file, readFileSync(file, 'utf8') + '\n// edit\n');
  [ms, r] = await timed(() => a.call('search', q)); row(`${v} one-file edit (incremental)`, ms, r);
  await a.close();
  // adoption
  root = mk(); const [bms] = await timed(async () => buildUserIndex(bin, root));
  console.log(`| ${v} user's own \`graft build\` (not adapter work) | ${bms.toFixed(0)} | - | - | - |`);
  a = new Adapter({ root, cache: tmp('c'), env: ADOPT('read-only') });
  [ms, r] = await timed(() => a.call('search', q)); row(`${v} adopt read-only (first)`, ms, r);
  [ms, r] = await timed(() => a.call('search', q)); row(`${v} adopt read-only (again)`, ms, r);
  await a.close();
  a = new Adapter({ root, cache: tmp('c'), env: ADOPT('copied-snapshot') });
  [ms, r] = await timed(() => a.call('search', q)); row(`${v} adopt copied-snapshot (first)`, ms, r);
  [ms, r] = await timed(() => a.call('search', q)); row(`${v} adopt copied-snapshot (again)`, ms, r);
  await a.close();
}
const [ms] = await timed(async () => { try { execFileSync('/usr/bin/grep', ['-rn', 'span_digest', corpus], { stdio: 'ignore' }); } catch { /* none */ } });
console.log(`| native grep -rn (baseline) | ${ms.toFixed(0)} | - | - | - |`);
function readdirSyncFirstRs(root) { return execFileSync('/usr/bin/find', [root, '-name', '*.rs', '-not', '-path', '*/.git/*']).toString().split('\n').filter(Boolean).sort()[0]; }
