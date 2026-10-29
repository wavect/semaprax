#!/usr/bin/env python3
"""Refresh the current-source custody pins after an intentional edit.

Run from a checkout whose edited sources are final:

    python3 scripts/refresh-source-custody.py --label <short-change-label>

The script never edits the sources you changed.  It rebinds only the custody
records that authenticate *current* bytes, in this order:

1. finite overlay v2 page (`docs/evidence/finite-source-overlay-v2-20261018`):
   every composed MAIN_E177 row whose physical file no longer matches its
   current pin gets the new `[length, sha256]` pin, and its previous pin is
   appended as a named view (`whole-pre-overlay-v2-current` on the first
   drift of an original row, `whole-before-<label>` afterwards).  A drifted
   original row that is not yet a replacement is admitted as an `old519_index`
   replacement in predecessor order.  The new bytes are archived under
   `e/<sha256>`.  The edit is made directly on the V11 wire, so existing rows
   stay byte-identical;
2. the policy's page pin, the POLICY/REPLACEMENTS constants in
   `overlay_page.py` and `src/source_reflows_merge/main_e177_overlay_v2.rs`;
3. the pin chain overlay_page.py -> guard_payload.py -> check-source.py (x2)
   and verify-or-refresh.py;
4. `docs/evidence/a3-e47-final-custody-20261016/{hir,projection}.json`:
   sources, chunks and closure come from the canonical-byte generator; every
   existing class chain is retained and only its edge offsets are relocated;
5. `docs/evidence/source-boundary-domains-20261016/current-domains.json`
   (members and the three full-manifest/raw/production digests).

Every rewritten record is re-validated with the repository's own decoder
(overlay compose + physical/archive check) or with a port of the Rust check
(edge text at offset, domain digests) before anything is written.

`--check` reports drift without writing.  A brand-new file that must become an
overlay *addition* (new compiled child of the source-inventory carrier) is
reported, not admitted: add it by hand as in d70e4af22 (policy/page additions,
both ADDITIONS constants and a `main_e177_overlay_compiled.rs` arm), then rerun.
"""
from pathlib import Path
import argparse, copy, hashlib, json, re, subprocess, sys

OVERLAY_DIR = 'docs/evidence/finite-source-overlay-v2-20261018'
POLICY = OVERLAY_DIR + '/policy.json'
PAGE = OVERLAY_DIR + '/page.json'
OVERLAY_PY = OVERLAY_DIR + '/overlay_page.py'
OVERLAY_RS = 'src/source_reflows_merge/main_e177_overlay_v2.rs'
GUARD = 'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py'
GUARD_HOLDERS = (
    'docs/evidence/accepted-final6-source-binding-20261017/check-source.py',
    'docs/evidence/accepted-strict-guard-generation-20261017/check-source.py',
    'docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py',
)
AUTHORITY_DIR = 'docs/evidence/a3-e47-final-custody-20261016'
AUTHORITY_GENERATOR = 'docs/evidence/canonical-byte-source-20261022/current-source-authority.py'
DOMAINS = 'docs/evidence/source-boundary-domains-20261016/current-domains.json'
DOMAIN_BASE = {
    'hir': ('docs/evidence/source-boundary-domains-20261016/hir-complete-a82e-manifest.json',
            'ed7cdfdf39c3cfe6a90ba84764ca643716b9ac4883757dbbd0efc3d6b9a53abd'),
    'projection': ('docs/evidence/source-boundary-domains-20261016/projection-complete-a82e-manifest.json',
                   'f943a9f16b7f4bb1f2b4f0574d8e07db57e275661211f523c9f79217922937ee'),
}
PRE_OVERLAY = 'whole-pre-overlay-v2-current'
sha = lambda b: hashlib.sha256(b).hexdigest()


class Repo:
    def __init__(self, root, dry):
        self.root, self.dry, self.written = root, dry, []

    def raw(self, path):
        return (self.root / path).read_bytes()

    def write(self, path, data):
        if isinstance(data, str):
            data = data.encode()
        target = self.root / path
        if target.exists() and target.read_bytes() == data:
            return False
        self.written.append(path)
        if not self.dry:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        return True


def replace_once(text, old, new, what):
    assert text.count(old) == 1, f'{what}: expected exactly one {old[:80]!r}'
    return text.replace(old, new, 1)


# ---------------------------------------------------------------- overlay ---
def load_guard(repo):
    path = repo.root / GUARD
    ns = {'__file__': str(path)}
    raw = path.read_bytes()
    ns['_AUTHENTICATED_PROGRAM'] = raw
    exec(compile(raw, str(path), 'exec'), ns)
    ns['_overlay']()  # authenticates overlay_page.py against the guard's pin
    return ns


def refresh_overlay(repo, label):
    guard = load_guard(repo)
    ov = guard['_OVERLAY_NS']
    before = guard['packet_before_overlay']()
    policy_raw, page_raw = repo.raw(POLICY), repo.raw(PAGE)
    assert len(policy_raw) == ov['POLICY_LENGTH'] and sha(policy_raw) == ov['POLICY_SHA256'], \
        'overlay_page.py POLICY pin is already stale; fix it before refreshing'
    policy, wire = ov['parse'](policy_raw), ov['parse'](page_raw)
    assert policy['page']['length'] == len(page_raw) and policy['page']['sha256'] == sha(page_raw)
    assert wire['schema'] == ov['V11_SCHEMA'], 'refresh edits only the V11 wire'
    composed = ov['compose_values'](before, policy, wire)
    old_rows = before['rows']
    position = {r['source']: i for i, r in enumerate(old_rows)}
    replaced = {s: i for i, s in enumerate(policy['replacements'])}
    added = {b['source']: i for i, b in enumerate(policy['additions'])}
    names = wire['names']
    view_name = f'whole-before-{label}'

    def name_slot(name):
        return names.index(name) if name in names else name

    drift = []
    for row in composed['rows']:
        path = repo.root / row['source']
        if not path.is_file():
            continue
        data = path.read_bytes()
        cur = row['current']
        if len(data) != cur['length'] or sha(data) != cur['sha256']:
            drift.append((row['source'], data, cur))
    if not drift:
        return []
    inner = wire['page']
    for source, data, old in drift:
        new_pin = [len(data), sha(data)]
        blob = f'e/{new_pin[1]}'
        repo.write(blob, data)
        if source in replaced:
            slot = inner['replacements'][replaced[source]]
            if 'old519_index' in slot:
                ref = slot['old519_index']
                current_slot, views = ref[1], ref[2]
                ref[1] = new_pin
            else:
                current_slot, views = slot['current'], slot['views']
                slot['current'] = new_pin
            prior = old_rows[position[source]]['current']
            first = old == prior
            views.append([name_slot(PRE_OVERLAY if first else view_name)] + copy.deepcopy(current_slot))
        elif source in added:
            slot = inner['additions'][added[source]]
            views = slot['views']
            views.append([name_slot(view_name)] + copy.deepcopy(slot['current']))
            slot['current'] = new_pin
        elif source in position:
            index = position[source]
            order = [position[s] for s in policy['replacements']]
            at = sum(1 for i in order if i < index)
            policy['replacements'].insert(at, source)
            inner['replacements'].insert(
                at, {'old519_index': [index, new_pin, [[name_slot(PRE_OVERLAY), index]], None]})
            replaced = {s: i for i, s in enumerate(policy['replacements'])}
        else:
            raise SystemExit(f'{source}: drifted row is neither predecessor nor addition')
    page_bytes = ov['canonical'](wire)
    assert len(page_bytes) <= ov['PAGE_CAP'], 'overlay page exceeds its one-page cap'
    policy['page'] = dict(path=PAGE, length=len(page_bytes), sha256=sha(page_bytes))
    policy_bytes = ov['canonical'](policy)
    # Full decode with the repository's own composer, then authenticate every
    # row's current pin against the physical file and its e/ archive.
    check = ov['compose_values'](before, ov['parse'](policy_bytes), ov['parse'](page_bytes))
    for row in check['rows']:
        path = repo.root / row['source']
        if path.is_file():
            data = path.read_bytes()
            assert [len(data), sha(data)] == [row['current']['length'], row['current']['sha256']], row['source']
    repo.write(PAGE, page_bytes)
    repo.write(POLICY, policy_bytes)

    # Policy pins and replacement lists mirrored in Python and Rust.
    py = repo.raw(OVERLAY_PY).decode()
    py = re.sub(r"^POLICY_LENGTH = \d+$", f'POLICY_LENGTH = {len(policy_bytes)}', py, count=1, flags=re.M)
    py = re.sub(r"^POLICY_SHA256 = '[0-9a-f]{64}'$", f"POLICY_SHA256 = '{sha(policy_bytes)}'", py, count=1, flags=re.M)
    py = re.sub(r"^REPLACEMENTS = \(.*\)$", lambda _: 'REPLACEMENTS = ' + repr(tuple(policy['replacements'])), py, count=1, flags=re.M)
    rs = repo.raw(OVERLAY_RS).decode()
    rs = re.sub(r'^const POLICY_LENGTH: usize = \d+;', f'const POLICY_LENGTH: usize = {len(policy_bytes)};', rs, count=1, flags=re.M)
    rs = re.sub(r'^const POLICY_SHA256: &str = "[0-9a-f]{64}";', f'const POLICY_SHA256: &str = "{sha(policy_bytes)}";', rs, count=1, flags=re.M)
    rs = re.sub(r'^const REPLACEMENTS: &\[&str\] = &\[.*\];$',
                lambda _: 'const REPLACEMENTS: &[&str] = &' + json.dumps(policy['replacements'], separators=(',', ':')) + ';',
                rs, count=1, flags=re.M)
    for text in (py, rs):
        assert str(len(policy_bytes)) in text and sha(policy_bytes) in text
    old_py = repo.raw(OVERLAY_PY)
    repo.write(OVERLAY_PY, py)
    repo.write(OVERLAY_RS, rs)
    return [d[0] for d in drift], old_py


def rebind(repo, holder, target, old_raw):
    """Replace one exact (length, sha256) pin of `target` inside `holder`."""
    new_raw = repo.raw(target) if not repo.dry else None
    if new_raw is None or new_raw == old_raw:
        return
    text = repo.raw(holder).decode()
    old_sha, new_sha = sha(old_raw), sha(new_raw)
    assert text.count(old_sha) == 1, f'{holder}: pin of {target}'
    text = text.replace(old_sha, new_sha)
    if len(old_raw) != len(new_raw):
        name = Path(target).name
        lines = text.split('\n')
        hits = [i for i, l in enumerate(lines) if (name in l or new_sha in l) and re.search(rf'\b{len(old_raw)}\b', l)]
        assert len(hits) == 1, f'{holder}: length pin of {target}'
        lines[hits[0]] = re.sub(rf'\b{len(old_raw)}\b', str(len(new_raw)), lines[hits[0]], count=1)
        text = '\n'.join(lines)
    repo.write(holder, text)


# -------------------------------------------------------------- authority ---
def relocate(repo, classes):
    cache = {}
    for row in classes.values():
        for chain in row['chains']:
            for edge in chain['edges']:
                parent = edge['parent']
                text = cache.setdefault(parent, (repo.root / parent).read_bytes())
                needle = edge['text'].encode()
                if text[edge['start']:edge['start'] + len(needle)] == needle:
                    continue
                hits = [m.start() for m in re.finditer(re.escape(needle), text)]
                assert hits, f'edge text vanished from {parent}: {edge["text"][:80]!r}'
                # The edge moved with its parent bytes: take the nearest occurrence.
                edge['start'] = min(hits, key=lambda h: abs(h - edge['start']))


def refresh_authority(repo, fast):
    out = repo.root / 'target/refresh-source-custody/authority'
    if not fast:
        out.mkdir(parents=True, exist_ok=True)
        subprocess.run([sys.executable, str(repo.root / AUTHORITY_GENERATOR), '--repo', str(repo.root),
                        '--out', str(out)], check=True)
    for name in ('hir', 'projection'):
        path = f'{AUTHORITY_DIR}/{name}.json'
        raw = repo.raw(path)
        manifest = json.loads(raw)
        assert (json.dumps(manifest, separators=(',', ':')) + '\n').encode() == raw
        if fast:
            for p, pin in manifest['sources'].items():
                data = (repo.root / p).read_bytes()
                pin['length'], pin['sha256'] = len(data), sha(data)
        else:
            generated = json.loads((out / f'{name}.json').read_text())
            assert generated['parser_sha256'] == manifest['parser_sha256']
            classes = {p: manifest['classes'].get(p, generated['classes'][p]) for p in generated['classes']}
            manifest = dict(schema=manifest['schema'], parser_sha256=manifest['parser_sha256'],
                            sources=generated['sources'], classes=classes,
                            chunks=generated['chunks'], closure_only=generated['closure_only'])
        relocate(repo, manifest['classes'])
        repo.write(path, json.dumps(manifest, separators=(',', ':')) + '\n')


def compiled_edge(text):
    """Port of tests/support/new_boundary_module.rs::compiled_edge."""
    text = text.strip()
    if text.startswith('include_str!'):
        return False
    if text.startswith('include!'):
        return True
    for m in re.finditer('mod', text):
        at = m.start()
        if at and (text[at - 1].isalnum() or text[at - 1] == '_'):
            continue
        tail = text[at + 3:]
        if not tail[:1].isspace():
            continue
        tail = tail.strip()
        if not (tail.endswith(';') or tail.endswith('{')):
            continue
        name = tail[:-1].strip()
        name = name[2:] if name.startswith('r#') else name
        if name and all(c.isalnum() or c == '_' for c in name):
            return True
    raise SystemExit(f'unrecognized actual registration edge: {text}')


def refresh_domains(repo):
    raw = repo.raw(DOMAINS)
    domains = json.loads(raw)
    for name, (base_path, base_sha) in DOMAIN_BASE.items():
        base_raw = repo.raw(base_path)
        assert sha(base_raw) == base_sha
        roots = set(json.loads(base_raw)['classes'])
        manifest_raw = repo.raw(f'{AUTHORITY_DIR}/{name}.json')
        full = json.loads(manifest_raw)
        classes = full['classes']
        edges = {}
        for row in classes.values():
            for chain in row['chains']:
                for edge in chain['edges']:
                    if compiled_edge(edge['text']):
                        edges.setdefault(edge['parent'], set()).add(edge['child'])
        members, pending = set(roots), list(roots)
        while pending:
            for child in edges.get(pending.pop(), ()):
                assert child in classes, f'actual registered descendant omitted: {child}'
                if child not in members:
                    members.add(child)
                    pending.append(child)
        sources = {p: (repo.root / p).read_text() for p in full['sources']}
        whole_raw, whole_production = [], []
        for chunk in full['chunks']:
            if 'path' in chunk:
                whole_raw.append(sources[chunk['path']])
                if classes[chunk['path']]['role'] == 'production':
                    whole_production.append(sources[chunk['path']])
            else:
                whole_raw.append(chunk['literal'])
                whole_production.append(chunk['literal'])
        for p in full['closure_only']:
            if classes[p]['role'] == 'production':
                whole_production.append(sources[p])
        scope = domains['domains'][name]
        scope['current_members'] = sorted(members)
        scope['production_members'] = sorted(p for p in members if classes[p]['role'] == 'production')
        scope['fully_authenticated_outside_production'] = sorted(
            p for p, v in classes.items() if v['role'] == 'production' and p not in members)
        scope['full_manifest_sha256'] = sha(manifest_raw)
        scope['full_raw_sha256'] = sha(''.join(whole_raw).encode())
        scope['full_authenticated_production_sha256'] = sha(''.join(whole_production).encode())
    repo.write(DOMAINS, json.dumps(domains, indent=2) + '\n')


def main():
    a = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    a.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[1])
    a.add_argument('--label', help='view-name suffix for superseded pins, e.g. prelude-inverse-d70e')
    a.add_argument('--check', action='store_true', help='report what would change; write nothing')
    a.add_argument('--fast', action='store_true',
                   help='skip the ~5 minute authority generator; valid only when no include!/mod/include_str! '
                        'registration or audit-inventory list changed')
    args = a.parse_args()
    repo = Repo(args.repo.resolve(), args.check)
    old_guard = repo.raw(GUARD)
    old_overlay_py = repo.raw(OVERLAY_PY)
    result = refresh_overlay(repo, args.label or 'unlabelled')
    if result:
        drifted, _ = result
        assert args.label, '--label is required when overlay rows drifted'
        print('overlay rows refreshed:', *drifted, sep='\n  ')
        if not args.check:
            rebind(repo, GUARD, OVERLAY_PY, old_overlay_py)
            for holder in GUARD_HOLDERS:
                rebind(repo, holder, GUARD, old_guard)
    if args.check:
        print('check only; overlay files that would change:', *repo.written, sep='\n  ')
        return
    refresh_authority(repo, args.fast)
    refresh_domains(repo)
    print('written:', *repo.written, sep='\n  ')


if __name__ == '__main__':
    main()
