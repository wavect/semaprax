// Per-version compatibility profiles (data in compat/profiles.json, produced by scripts/qualify.mjs).
// A version not listed is refused with the qualification path; there is no version range.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const FILE = join(dirname(fileURLToPath(import.meta.url)), '..', 'compat', 'profiles.json');
const CLI_KEYS = ['dir', 'noRefresh', 'json', 'limit', 'maxDirs', 'fixed', 'direction', 'depth', 'in', 'noReuse'];

export const QUALIFICATION_PATH = 'run `node scripts/qualify.mjs <graft>` in packages/semaprax-harness-adapters/graft, add the printed '
  + 'entry to compat/profiles.json, add the version to harness-provider.json upstream.versions and support.tested, and run the real-graft tests (HARNESS_GRAFT_NEW)';

let cached = null;
export function loadProfiles(file = FILE) {
  if (file === FILE && cached) return cached;
  const doc = JSON.parse(readFileSync(file, 'utf8'));
  if (doc.schema !== 'semaprax.graft-compat.v1' || typeof doc.profiles !== 'object') throw new Error('compat/profiles.json: bad schema');
  for (const [v, p] of Object.entries(doc.profiles)) {
    for (const k of CLI_KEYS) if (typeof p.cli?.[k] !== 'string') throw new Error(`profile ${v}: cli.${k} missing`);
    if (!Array.isArray(p.parsed_extensions) || !Number.isInteger(p.wiring_meta_version)) throw new Error(`profile ${v}: incomplete`);
  }
  if (file === FILE) cached = doc;
  return doc;
}

export const testedVersions = (doc = loadProfiles()) => Object.keys(doc.profiles);

// Exact-version lookup; null for a version nobody qualified.
export const profileFor = (version, doc = loadProfiles()) => (Object.hasOwn(doc.profiles, version) ? doc.profiles[version] : null);
