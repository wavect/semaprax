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
   drift of an original row, the supplied label afterwards).  A drifted
   original row that is not yet a replacement is admitted as an `old519_index`
   replacement in predecessor order.  The new bytes are archived under
   `e/<sha256>`.  The V11/V12 name and digest encoding preserves the entire logical page,
   including every existing row, view and unknown field;
2. the policy's page pin, the POLICY/REPLACEMENTS constants in
   `overlay_page.py` and `src/source_reflows_merge/main_e177_overlay_v2.rs`;
3. the pin chain overlay_page.py -> guard_payload.py -> check-source.py (x2)
   and verify-or-refresh.py;
4. `docs/evidence/a3-e47-final-custody-20261016/{hir,projection}.json`:
   sources, chunks and closure come from the canonical-byte generator; current registration chains and production/test roles are regenerated,
   while other existing class metadata is retained;
5. declared-history current pins and exact current-to-predecessor inverses,
   after the overlay reader pins settle; sealed predecessor archives stay fixed;
6. `docs/evidence/source-boundary-domains-20261016/current-domains.json`
   (members and the three full-manifest/raw/production digests).

Every rewritten record is re-validated with the repository's own decoder
(overlay compose + physical/archive check) or with a port of the Rust check
(edge text at offset, domain digests) before anything is written.

`--check` reports drift in every custody family without writing and exits
nonzero when a refresh is needed. A brand-new file that must become an
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
OVERLAY_COMPILED_RS = 'src/source_reflows_merge/main_e177_overlay_compiled.rs'
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
INGRESS = 'docs/evidence/ordinary-ingress-successor-20261026/current-inputs.json'
INGRESS_RS = 'src/source_reflows_merge/ordinary_plan_inputs.rs'
sha = lambda b: hashlib.sha256(b).hexdigest()


def refresh_ordinary_ingress(repo):
    """Rebind drifted rows of the physical->latest ordinary-plan successor.

    Each row keeps its sealed `previous_*` pin; only the current pin, its
    `e/` archive and the exact current->previous inverse are recomputed.
    """
    import difflib
    raw = repo.raw(INGRESS)
    packet = json.loads(raw)
    assert (json.dumps(packet, separators=(',', ':')) + '\n').encode() == raw
    changed = []
    for row in packet['rows']:
        live = repo.raw(row['path'])
        if len(live) == row['current_length'] and sha(live) == row['current_sha256']:
            continue
        previous = repo.raw(row['previous_archive'])
        assert len(previous) == row['previous_length'] and sha(previous) == row['previous_sha256']
        a = live.decode().splitlines(keepends=True)
        b = previous.decode().splitlines(keepends=True)
        offsets = [0]
        for line in a:
            offsets.append(offsets[-1] + len(line.encode()))
        edits = []
        for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes():
            if tag != 'equal':
                edits.append(dict(after=''.join(a[i1:i2]), before=''.join(b[j1:j2]), start_byte=offsets[i1]))
        edits.reverse()
        assert len(edits) <= 512
        restored, last = live.decode(), len(live)
        for e in edits:  # the Rust reader's exact replay (byte offsets, descending)
            start, end = e['start_byte'], e['start_byte'] + len(e['after'].encode())
            assert end <= last
            data = restored.encode()
            assert data[start:end] == e['after'].encode()
            restored = (data[:start] + e['before'].encode() + data[end:]).decode()
            last = start
        assert restored.encode() == previous
        row.update(current_length=len(live), current_sha256=sha(live),
                   current_archive=f'e/{sha(live)}', inverse=edits)
        repo.write(f'e/{sha(live)}', live)
        changed.append(row['path'])
    if not changed:
        return []
    new = (json.dumps(packet, separators=(',', ':')) + '\n').encode()
    repo.write(INGRESS, new)
    rs = repo.raw(INGRESS_RS).decode()
    rs = replace_once(rs, f'const LENGTH: usize = {len(raw)};', f'const LENGTH: usize = {len(new)};', INGRESS_RS)
    rs = replace_once(rs, f'const SHA: &str = "{sha(raw)}";', f'const SHA: &str = "{sha(new)}";', INGRESS_RS)
    repo.write(INGRESS_RS, rs)
    return changed


MAIN043_SUCCESSORS = 'docs/evidence/source-coverage-254-20261024/latest-to-successors.json'
MAIN043_SUCCESSORS_RS = 'src/source_reflows_merge/runtime_json.rs'


def unique_inverse(live, target):
    """String pairs the Rust reader applies with `replacen(after, before, 1)`
    after asserting `matches(after).count() == 1` and a non-empty `after`."""
    import difflib
    a, b = live.splitlines(keepends=True), target.splitlines(keepends=True)
    hunks = [op for op in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes() if op[0] != 'equal']
    current, pairs = list(a), []
    for _, i1, i2, j1, j2 in reversed(hunks):  # descending: earlier lines keep their indexes
        lo, hi = i1, i2
        while True:
            after = ''.join(current[lo:hi])
            text = ''.join(current)
            if after and text.count(after) == 1:
                break
            assert lo > 0 or hi < len(current), 'no unique context'
            lo, hi = max(lo - 1, 0), min(hi + 1, len(current))
        before = ''.join(current[lo:i1]) + ''.join(b[j1:j2]) + ''.join(current[i2:hi])
        pairs.append([after, before])
        current[lo:hi] = before.splitlines(keepends=True)
    return pairs


def apply_unique(text, pairs):
    for after, before in pairs:
        assert after and text.count(after) == 1
        text = text.replace(after, before, 1)
    return text


def refresh_main043_successors(repo):
    """Rebind drifted physical rows of the main043 RAW latest->checkpoint edge.

    The sealed checkpoint pin, archive and every sealed inverse pair stay; a
    physical->previous-current prefix is prepended so the reader still
    reconstructs the identical checkpoint from the complete new bytes.
    """
    raw = repo.raw(MAIN043_SUCCESSORS)
    packet = json.loads(raw)
    assert (json.dumps(packet, indent=2) + '\n').encode() == raw
    changed = []
    for row in packet['rows']:
        live = repo.raw(row['source'])
        if len(live) == row['length'] and sha(live) == row['sha256']:
            continue
        previous = repo.raw(f"e/{row['sha256']}")
        assert len(previous) == row['length'] and sha(previous) == row['sha256']
        checkpoint = repo.raw(row['archive'])
        assert row['archive'] == f"e/{row['checkpoint_sha256']}"
        assert len(checkpoint) == row['checkpoint_length'] and sha(checkpoint) == row['checkpoint_sha256']
        assert apply_unique(previous.decode(), row['inverse']).encode() == checkpoint
        prefix = unique_inverse(live.decode(), previous.decode())
        inverse = prefix + row['inverse']
        assert apply_unique(live.decode(), inverse).encode() == checkpoint
        row.update(length=len(live), sha256=sha(live), inverse=inverse)
        repo.write(f'e/{sha(live)}', live)
        changed.append(row['source'])
    if not changed:
        return []
    new = (json.dumps(packet, indent=2) + '\n').encode()
    repo.write(MAIN043_SUCCESSORS, new)
    rs = repo.raw(MAIN043_SUCCESSORS_RS).decode()
    rs = replace_once(rs, f'        {len(raw)},\n        "{sha(raw)}",',
                      f'        {len(new)},\n        "{sha(new)}",', MAIN043_SUCCESSORS_RS)
    repo.write(MAIN043_SUCCESSORS_RS, rs)
    return changed


DECLARED_HISTORY = 'docs/evidence/declared-history-inputs-20261105'
DECLARED_HISTORY_RS = 'src/source_reflows/declared_history_inputs.rs'


def refresh_declared_history(repo):
    """Rebind complete current-to-history inverses after the overlay pins settle.

    Authenticate both sealed archives and replay the original inverse first.
    Preserve every historical byte, provenance field and unrelated table row.
    """
    import difflib
    changed = []
    holder = None
    for path in sorted((repo.root / DECLARED_HISTORY).glob('part-*.json')):
        relative = str(path.relative_to(repo.root))
        raw = repo.raw(relative)
        rows = json.loads(raw)
        assert (json.dumps(rows, separators=(',', ':')) + '\n').encode() == raw
        dirty = False
        for row in rows:
            live = repo.raw(row['source'])
            if len(live) == row['current_bytes'] and sha(live) == row['current_sha256']:
                continue
            current = repo.raw(row['current_archive'])
            previous = repo.raw(row['before_archive'])
            assert len(current) == row['current_bytes'] and sha(current) == row['current_sha256']
            assert len(previous) == row['before_bytes'] and sha(previous) == row['before_sha256']
            restored = current
            for start, after, before in reversed(row['inverse']):
                after, before = after.encode(), before.encode()
                assert restored[start:start + len(after)] == after
                restored = restored[:start] + before + restored[start + len(after):]
            assert restored == previous, 'complete declared predecessor must remain identical'
            a, b = live.splitlines(keepends=True), previous.splitlines(keepends=True)
            offsets = [0]
            for line in a:
                offsets.append(offsets[-1] + len(line))
            inverse = [[offsets[i], b''.join(a[i:j]).decode(), b''.join(b[k:l]).decode()]
                       for tag, i, j, k, l in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes()
                       if tag != 'equal']
            restored = live
            for start, after, before in reversed(inverse):
                after, before = after.encode(), before.encode()
                assert restored[start:start + len(after)] == after
                restored = restored[:start] + before + restored[start + len(after):]
            assert restored == previous, 'refreshed inverse must restore the complete sealed predecessor'
            archive = f'e/{sha(live)}'
            repo.write(archive, live)
            row.update(current_bytes=len(live), current_sha256=sha(live),
                       current_archive=archive, inverse=inverse)
            changed.append(row['source'])
            dirty = True
        if dirty:
            new = (json.dumps(rows, separators=(',', ':')) + '\n').encode()
            assert len(new) <= 2_097_152, 'unchanged declared-history table cap'
            repo.write(relative, new)
            if holder is None:
                holder = repo.raw(DECLARED_HISTORY_RS).decode()
            pattern = (r'("' + re.escape(relative) + r'",\s*include_str!\(concat!\(.*?\)\),\s*)'
                       + str(len(raw)) + r',\s*"' + sha(raw) + r'"')
            holder, count = re.subn(pattern, lambda m: m[1] + str(len(new)) + ',\n        "' + sha(new) + '"',
                                    holder, count=1, flags=re.S)
            assert count == 1, f'{relative}: exactly one compiled history-table pin'
    if holder is not None:
        repo.write(DECLARED_HISTORY_RS, holder)
    return changed


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


def compact_v11_names(ov, page, codec='v11'):
    """Choose the smallest sorted name dictionary without changing any V10 byte."""
    counts = {}
    def count(name):
        assert type(name) is str, 'v11 encoding name'
        counts[name] = counts.get(name, 0) + 1
        return name
    ov['v11_names_slots'](page, count)
    candidates = sorted(name for name, frequency in counts.items() if frequency > 1)
    # Large dictionaries retain the original complete sorted dictionary.
    if len(candidates) > 256:
        selected = tuple(candidates)
    else:
        choices = {0: (0, ())}
        for name in candidates:
            size = len(ov['canonical'](name))
            following = {}
            for number, (cost, selected) in choices.items():
                options = (
                    (number, cost + counts[name] * size, selected),
                    (number + 1, cost + size + 1 + counts[name] * len(str(number)), selected + (name,)),
                )
                for new_number, new_cost, new_selected in options:
                    if new_number not in following or new_cost < following[new_number][0]:
                        following[new_number] = (new_cost, new_selected)
            choices = following
        # Dictionary entries have a comma each except for the final entry.
        number, (_, selected) = min(choices.items(), key=lambda item: (item[1][0] - bool(item[0]), item[0]))
    names = list(selected)
    indices = {name: index for index, name in enumerate(names)}
    encoded = ov['v11_names_slots'](page, lambda name: indices.get(name, name))
    if codec == 'v13':
        result = ov['encode_v13_names'](encoded, names)
        inverse = ov['expand_v13_packed'](result)
    elif codec == 'v12':
        result = ov['encode_v12_names'](encoded, names)
        inverse = ov['expand_v12_packed'](result)
    else:
        assert codec == 'v11', 'known wire encoding'
        result = dict(schema=ov['V11_SCHEMA'], names=names, page=encoded)
        inverse = ov['expand_v11_names'](result)
    assert len(ov['canonical'](result)) <= ov['PAGE_CAP'], f"unchanged wire cap: {len(ov['canonical'](result))}/{ov['PAGE_CAP']}"
    assert ov['canonical'](inverse) == ov['canonical'](page), 'ENTIRE V10 page inverse'
    return result


def refresh_distinct_owner_offsets(text, carriers, updates):
    import ast
    table = re.search(r"^ finite_distinct_owners=(\{[^\n]*\})$", text, re.M)
    assert table is not None, 'missing exact distinct-owner offsets'
    previous = ast.literal_eval(table[1])
    current = dict(previous)
    for (carrier, source), offsets in previous.items():
        if carrier not in updates:
            continue
        literal = 'include_str!("' + carriers[carrier]['base'] + source + '")'
        data = updates[carrier].decode()
        hits = tuple(match.start() for match in re.finditer(re.escape(literal), data))
        assert len(hits) == len(offsets), (carrier, source, 'distinct owner count changed')
        current[carrier, source] = hits
    if current == previous:
        return text
    return text[:table.start(1)] + repr(current) + text[table.end(1):]


def refresh_strict_module_pins(repo, guard, updates):
    config = copy.deepcopy(guard['CONFIG'])
    for source, data in updates.items():
        for table in ('modules', 'carriers'):
            if source in config[table]:
                config[table][source].update(length=len(data), sha256=sha(data))
    raw = refresh_distinct_owner_offsets(repo.raw(GUARD).decode(), config['carriers'], updates)
    if config != guard['CONFIG']:
        candidate = guard['_encode_configuration'](config)
        assert guard['_decode_configuration'](copy.deepcopy(candidate)) == config
        path = str(guard['_CONFIG_PATH'].relative_to(repo.root))
        old_config = repo.raw(path)
        new_config = (json.dumps(candidate, ensure_ascii=False, separators=(',', ':')) + '\n').encode()
        assert len(new_config) <= guard['_OVERLAY_NS']['PAGE_CAP']
        repo.write(f'e/{sha(old_config)}', old_config)
        repo.write(path, new_config)
        raw = replace_once(raw, f'_CONFIG_LENGTH={len(old_config)}', f'_CONFIG_LENGTH={len(new_config)}', GUARD)
        raw = replace_once(raw, f"_CONFIG_SHA256='{sha(old_config)}'", f"_CONFIG_SHA256='{sha(new_config)}'", GUARD)
    return repo.write(GUARD, raw)


def refresh_overlay_v21(repo, guard, before, policy, wire, label, admit, attach_fragments=None):
    """Refresh the V21 transport while keeping both historical input sets."""
    ov = guard['_OVERLAY_NS']
    original = ov['compose_values'](before, policy, wire)
    logical = ov['expand_compressed_page'](wire)
    successor = ov['_v20_successor'](logical, ov['V21_OUTPUT_CAP'])
    rows = {row['source']: copy.deepcopy(row) for row in original['rows']}
    base = {row['source']: row for row in before['rows']}
    if admit:
        lookup = OVERLAY_COMPILED_RS.replace('compiled.rs', 'lookup.rs')
        header = 'pub(crate) fn compiled(source: &str) -> Option<&\'static str> {\n    match source {\n'
        arms = ''.join(f'        {json.dumps(s)} => Some(include_str!("../../{s}")),\n'
                       for s, _ in admit)
        repo.write(lookup, replace_once(repo.raw(lookup).decode(), header, header + arms, lookup))
    changed = []
    for source, previous in admit:
        current = repo.raw(source)
        assert source not in rows and current != previous
        current_sha = sha(current)
        repo.write(f'e/{current_sha}', current)
        views = []
        if previous is not None:
            previous_sha = sha(previous)
            repo.write(f'e/{previous_sha}', previous)
            views.append(dict(name=label, length=len(previous), sha256=previous_sha, archive=f'e/{previous_sha}'))
        rows[source] = dict(
            source=source, compiled=True,
            current=dict(length=len(current), sha256=current_sha, archive=f'e/{current_sha}'),
            views=views,
        )
        changed.append(source)
    for parent, children in (attach_fragments or {}).items():
        assert parent in ov['V3_ROOTS']
        fragments = rows[parent].setdefault('fragments', [])
        assert not {child['source'] for child in fragments}.intersection(children)
        fragments.extend(rows[child] for child in children)
    for source, row in rows.items():
        path = repo.root / source
        if not path.is_file():
            continue
        current, old = path.read_bytes(), row['current']
        digest = sha(current)
        if (len(current), digest) == (old['length'], old['sha256']):
            continue
        name = PRE_OVERLAY if source in base and old == base[source]['current'] else label
        view = dict(name=name, **old)
        if view not in row['views']:
            row['views'].append(view)
        row['current'] = dict(old, length=len(current), sha256=digest, archive=f'e/{digest}')
        repo.write(f'e/{digest}', current)
        changed.append(source)
    strict_carriers = {source: repo.raw(source) for source in guard['CONFIG']['carriers']}
    if not changed:
        if refresh_strict_module_pins(repo, guard, strict_carriers):
            return [], repo.raw(OVERLAY_PY)
        return []
    for row in rows.values():
        if 'fragments' in row:
            row['fragments'] = [copy.deepcopy(rows[child['source']]) for child in row['fragments']]
    owner = successor['registration']['owner']
    declaration = successor['registration']['text'].encode()
    owner_raw = repo.raw(owner)
    assert owner_raw.count(declaration) == 1
    start = owner_raw.index(declaration)
    assert owner_raw[start - 2:start] == b'\n\n'
    successor['current'] = rows[successor['source']]['current']
    successor['registration'].update(current=rows[owner]['current'], start=start)

    edges = ov['edges'](before)
    combined = list(rows.values())
    policy['new_fragment_edges'] = [dict(parent=parent, child=child)
                                    for parent, child in ov['edges'](dict(rows=combined))
                                    if (parent, child) not in edges]
    policy['replacements'] = []
    policy['additions'] = []
    compact = dict(schema=ov['V8_SCHEMA'], index=1, total_pages=1,
                   predecessor=policy['predecessor'], replacements=[], additions=[])
    carriers = {binding[1] for binding in ov['V8_CARRIERS']}

    def packed(row):
        result = copy.deepcopy(row)
        if row['source'] in (*ov['V3_ROOTS'], *carriers) and 'fragments' in row:
            result['fragments'] = [dict(canonical_source=child['source']) for child in row['fragments']]
        return result

    for index, prior in enumerate(before['rows']):
        source = prior['source']
        row = rows[source]
        if row == prior and source != successor['source']:
            continue
        policy['replacements'].append(source)
        current = packed(row)
        fragments = current.get('fragments') if row.get('fragments') != prior.get('fragments') else None
        if fragments is not None and source not in ov['V3_ROOTS'] and source not in carriers:
            compact['replacements'].append(current)
        else:
            count = len(prior['views'])
            assert row['views'][:count] == prior['views']
            compact['replacements'].append(dict(old519_index=[index, row['current'], row['views'][count:], fragments]))
    for source, row in rows.items():
        if source not in base:
            policy['additions'].append(dict(source=source, compiled=row['compiled']))
            compact['additions'].append(packed(row))

    v9 = ov['transform_v9_pin_slots'](compact, True)
    v9['schema'] = ov['V9_SCHEMA']
    v10 = ov['encode_v10_with_prefixes'](v9, logical['page']['archive_prefixes'])
    v11 = ov['transform_v11_digest_slots'](v10, True)
    v11['schema'] = ov['V11_SCHEMA']
    cap = ov['V21_OUTPUT_CAP']
    # V12 is an intermediate source representation. V20 and the compressed
    # page below still enforce their own decoded and physical transport caps.
    v12 = ov['encode_v12_view_labels'](v11, ov['SOURCE_CAP'])
    v20 = ov['encode_v20'](ov['encode_v20_compact_inner'](v12, cap), successor, cap)
    page = ov['canonical'](ov['encode_compressed_page'](v20))
    policy['page'] = dict(path=PAGE, length=len(page), sha256=sha(page))
    policy_bytes = ov['canonical'](policy)
    assert len(policy_bytes) <= ov['PAGE_CAP']
    assert ov['compose_values'](before, policy, ov['parse'](page))['rows'] == combined

    # Keep the existing compact source spellings whenever the source scope
    # stays fixed. For a new path reuse the reader's existing prefix aliases.
    old_py = repo.raw(OVERLAY_PY)
    py = old_py.decode()
    rs = repo.raw(OVERLAY_RS).decode()
    for name, values in (('REPLACEMENTS', policy['replacements']),
                         ('ADDITIONS', [binding['source'] for binding in policy['additions']])):
        if ov[name] == values:
            continue
        aliases = [(key, value) for key, value in ov.items()
                   if key.startswith('_') and isinstance(value, str) and value.endswith('/')]
        def spelling(path):
            return min([repr(path), *(key + '+' + repr(path[len(prefix):])
                                      for key, prefix in aliases if path.startswith(prefix))], key=len)
        py = re.sub(rf'^{name}=\[.*\]$', lambda _: name + '=[' + ','.join(map(spelling, values)) + ']',
                    py, count=1, flags=re.M)
        rs = re.sub(rf'^pub\(super\) const {name}: &\[&str\] = &\[(?s:.*?)\];$',
                    lambda _: f'pub(super) const {name}: &[&str] = &'
                    + json.dumps(values, separators=(',', ':')) + ';', rs, count=1, flags=re.M)
    py = re.sub(r'^POLICY_LENGTH\s*=\s*\d+$', f'POLICY_LENGTH={len(policy_bytes)}', py, count=1, flags=re.M)
    py = re.sub(r"^POLICY_SHA256\s*=\s*'[0-9a-f]{64}'$", f"POLICY_SHA256='{sha(policy_bytes)}'", py, count=1, flags=re.M)
    rs = re.sub(r'^const POLICY_LENGTH: usize = \d+;', f'const POLICY_LENGTH: usize = {len(policy_bytes)};', rs, count=1, flags=re.M)
    rs = re.sub(r'^const POLICY_SHA256: &str = "[0-9a-f]{64}";', f'const POLICY_SHA256: &str = "{sha(policy_bytes)}";', rs, count=1, flags=re.M)
    assert len(py.encode()) <= 65536
    assert sha(policy_bytes) in py and sha(policy_bytes) in rs

    strict_sources = {source: repo.raw(source) for source in (*changed, successor['source'], owner)
                      if source in guard['CONFIG']['modules']}
    strict_sources.update(strict_carriers)
    strict_sources[OVERLAY_RS] = rs.encode()
    refresh_strict_module_pins(repo, guard, strict_sources)

    for path, data in ((PAGE, page), (POLICY, policy_bytes), (OVERLAY_PY, py), (OVERLAY_RS, rs)):
        repo.write(path, data)
    return changed, old_py


def refresh_overlay(repo, label, admit=()):
    guard = load_guard(repo)
    ov = guard['_OVERLAY_NS']
    before = guard['packet_before_overlay']()
    policy_raw, page_raw = repo.raw(POLICY), repo.raw(PAGE)
    assert len(policy_raw) == ov['POLICY_LENGTH'] and sha(policy_raw) == ov['POLICY_SHA256'], \
        'overlay_page.py POLICY pin is already stale; fix it before refreshing'
    policy, wire = ov['parse'](policy_raw), ov['parse'](page_raw)
    assert policy['page']['length'] == len(page_raw) and policy['page']['sha256'] == sha(page_raw)
    if wire['schema'] in ov['COMPRESSED_SCHEMAS']:
        return refresh_overlay_v21(repo, guard, before, policy, wire, label, admit)
    assert wire['schema'] in (ov['V11_SCHEMA'], ov['V12_SCHEMA'], ov['V13_SCHEMA']), 'known packed wire'
    wire_schema = wire['schema']
    composed = ov['compose_values'](before, policy, wire)
    old_rows = before['rows']
    position = {r['source']: i for i, r in enumerate(old_rows)}
    replaced = {s: i for i, s in enumerate(policy['replacements'])}
    added = {b['source']: i for i, b in enumerate(policy['additions'])}
    names = wire['names']
    # The caller's label names the complete superseded source directly.
    view_name = label

    def name_slot(name):
        return names.index(name) if name in names else name

    # Register all admitted children before capturing the compiled carrier's
    # current bytes. An intermediate carrier would consume a historical view
    # for a state that was never published.
    if admit:
        for source, _ in admit:
            assert source not in position and source not in added, f'{source}: already an overlay row'
        compiled_rs = repo.raw(OVERLAY_COMPILED_RS).decode()
        head = 'pub(super) fn compiled(source: &str) -> Option<&\'static str> {\n    match source {\n'
        arms = ''.join(f'        {json.dumps(s)} => Some(include_str!("../../{s}")),\n' for s, _ in admit)
        repo.write(OVERLAY_COMPILED_RS, replace_once(compiled_rs, head, head + arms, OVERLAY_COMPILED_RS))

    drift = []
    for row in composed['rows']:
        path = repo.root / row['source']
        if not path.is_file():
            continue
        data = path.read_bytes()
        cur = row['current']
        if len(data) != cur['length'] or sha(data) != cur['sha256']:
            drift.append((row['source'], data, cur))
    if not drift and not admit and wire_schema == ov['V13_SCHEMA']:
        return []
    if wire_schema in (ov['V12_SCHEMA'], ov['V13_SCHEMA']):
        unpacked = ov['expand_v13_packed'](wire) if wire_schema == ov['V13_SCHEMA'] else ov['expand_v12_packed'](wire)
        wire['page'] = ov['v11_names_slots'](unpacked, name_slot)
    inner = wire['page']
    # Admit an existing compiled file whose current bytes no MAIN_E177 row
    # covers as a new overlay addition: its live bytes become the current pin
    # and the superseded bytes a named view, as d70e4af22 did for new files.
    for source, previous in admit:
        assert source not in position and source not in added, f'{source}: already an overlay row'
        data = repo.raw(source)
        assert data != previous, f'{source}: nothing superseded'
        for blob in (data, previous):
            repo.write(f'e/{sha(blob)}', blob)
        policy['additions'].append(dict(compiled=True, source=source))
        inner['additions'].append(dict(compiled=True, current=[len(data), sha(data)], source=source,
                                       views=[[view_name, len(previous), sha(previous)]]))
        drift.append((source, data, None))
    for source, data, old in drift:
        new_pin = [len(data), sha(data)]
        blob = f'e/{new_pin[1]}'
        repo.write(blob, data)
        if old is None:
            continue  # admitted above
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
    # Intern repeated new view labels through the existing V11 encoder. Keep
    # every row and view; the encoder proves the entire V10 page round-trip
    # and enforces the unchanged page cap on the resulting wire.
    expanded = ov['v11_names_slots'](
        wire['page'], lambda value: names[value] if type(value) is int else value)
    wire = compact_v11_names(ov, expanded, codec='v13')
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

    # Policy pins and replacement lists mirrored in Python and Rust.
    py = repo.raw(OVERLAY_PY).decode()
    py = re.sub(r"^POLICY_LENGTH\s*=\s*\d+$", f'POLICY_LENGTH={len(policy_bytes)}', py, count=1, flags=re.M)
    py = re.sub(r"^POLICY_SHA256\s*=\s*'[0-9a-f]{64}'$", f"POLICY_SHA256='{sha(policy_bytes)}'", py, count=1, flags=re.M)
    py = re.sub(r"^REPLACEMENTS\s*=\s*\(.*\)$", lambda _: 'REPLACEMENTS=(' + ','.join(repr(s) for s in policy['replacements']) + ',)', py, count=1, flags=re.M)
    additions = [b['source'] for b in policy['additions']]
    py = re.sub(r"^ADDITIONS\s*=\s*\(.*\)$", lambda _: 'ADDITIONS=(' + ','.join(repr(s) for s in additions) + ',)', py, count=1, flags=re.M)
    rs = repo.raw(OVERLAY_RS).decode()
    rs = re.sub(r'^const POLICY_LENGTH: usize = \d+;', f'const POLICY_LENGTH: usize = {len(policy_bytes)};', rs, count=1, flags=re.M)
    rs = re.sub(r'^const POLICY_SHA256: &str = "[0-9a-f]{64}";', f'const POLICY_SHA256: &str = "{sha(policy_bytes)}";', rs, count=1, flags=re.M)
    rs = re.sub(r'^const REPLACEMENTS: &\[&str\] = &\[.*\];$',
                lambda _: 'const REPLACEMENTS: &[&str] = &' + json.dumps(policy['replacements'], separators=(',', ':')) + ';',
                rs, count=1, flags=re.M)
    rs = re.sub(r'^const ADDITIONS: &\[&str\] = &\[.*\];$',
                lambda _: 'const ADDITIONS: &[&str] = &' + json.dumps(additions, separators=(',', ':')) + ';',
                rs, count=1, flags=re.M)
    for text in (py, rs):
        assert str(len(policy_bytes)) in text and sha(policy_bytes) in text
    assert len(py.encode()) <= 65536, "unchanged Python decoder source cap"
    repo.write(PAGE, page_bytes)
    repo.write(POLICY, policy_bytes)
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
    manifests = {}
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
            # Registration can change its exact text or cfg boundary after a
            # merge. Authenticate the current complete graph rather than
            # retaining vanished edges or an obsolete production/test role.
            # Preserve any other class metadata from the previous manifest.
            classes = {}
            for source, current in generated['classes'].items():
                prior = copy.deepcopy(manifest['classes'].get(source, {}))
                prior.update(chains=current['chains'], role=current['role'])
                classes[source] = prior
            manifest = dict(schema=manifest['schema'], parser_sha256=manifest['parser_sha256'],
                            sources=generated['sources'], classes=classes,
                            chunks=generated['chunks'], closure_only=generated['closure_only'])
        relocate(repo, manifest['classes'])
        current = (json.dumps(manifest, separators=(',', ':')) + '\n').encode()
        manifests[name] = current
        repo.write(path, current)
    return manifests


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


def refresh_domains(repo, manifests=None):
    raw = repo.raw(DOMAINS)
    domains = json.loads(raw)
    for name, (base_path, base_sha) in DOMAIN_BASE.items():
        base_raw = repo.raw(base_path)
        assert sha(base_raw) == base_sha
        roots = set(json.loads(base_raw)['classes'])
        manifest_raw = (manifests[name] if manifests is not None
                        else repo.raw(f'{AUTHORITY_DIR}/{name}.json'))
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
    a.add_argument('--check', action='store_true',
                   help='report every custody family without writing; exit nonzero on drift')
    a.add_argument('--fast', action='store_true',
                   help='skip the ~5 minute authority generator; valid only when no include!/mod/include_str! '
                        'registration or audit-inventory list changed')
    a.add_argument('--admit', action='append', default=[], metavar='PATH@REV',
                   help='admit an existing compiled file with no MAIN_E177 overlay row as an addition; '
                        'REV (a git revision) supplies the superseded bytes kept as view whole-before-<label>')
    args = a.parse_args()
    repo = Repo(args.repo.resolve(), args.check)
    admit = []
    for spec in args.admit:
        path, rev = spec.rsplit('@', 1)
        previous = subprocess.run(['git', '-C', str(repo.root), 'show', f'{rev}:{path}'],
                                  check=True, capture_output=True).stdout
        admit.append((path, previous))
    assert args.label or not admit, '--label is required with --admit'
    # Each family can rewrite a file another family pins (an admission edits
    # main_e177_overlay_compiled.rs, itself an overlay row; a packet pin edit
    # changes runtime_json.rs or ordinary_plan_inputs.rs), so repeat until a
    # whole pass writes nothing.
    for attempt in range(6):
        before = len(repo.written)
        ingress = refresh_ordinary_ingress(repo)
        if ingress:
            print('ordinary ingress rows refreshed:', *ingress, sep='\n  ')
        main043 = refresh_main043_successors(repo)
        if main043:
            print('main043 RAW successor rows refreshed:', *main043, sep='\n  ')
        old_guard = repo.raw(GUARD)
        old_overlay_py = repo.raw(OVERLAY_PY)
        result = refresh_overlay(repo, args.label or 'unlabelled', admit if attempt == 0 else ())
        if result:
            drifted, _ = result
            assert args.label, '--label is required when overlay rows drifted'
            print('overlay rows refreshed:', *drifted, sep='\n  ')
            if not args.check:
                rebind(repo, GUARD, OVERLAY_PY, old_overlay_py)
                for holder in GUARD_HOLDERS:
                    if GUARD.encode() in repo.raw(holder):
                        rebind(repo, holder, GUARD, old_guard)
        if args.check or len(repo.written) == before:
            break
    else:
        raise SystemExit('custody refresh did not reach a fixed point')
    declared = refresh_declared_history(repo)
    if declared:
        print('declared history rows refreshed:', *declared, sep='\n  ')
    if args.check:
        # Check the physical source pins too: an unchanged overlay does not
        # imply that the authority manifests still bind the current sources.
        # Pass the computed manifests directly to domain derivation so this
        # remains read-only and checks the same values a refresh would write.
        refresh_domains(repo, refresh_authority(repo, True))
        print('check only; overlay files that would change:', *repo.written, sep='\n  ')
        return int(bool(repo.written))
    refresh_authority(repo, args.fast)
    refresh_domains(repo)
    print('written:', *repo.written, sep='\n  ')


if __name__ == '__main__':
    raise SystemExit(main())
