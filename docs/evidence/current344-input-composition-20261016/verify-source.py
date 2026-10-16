#!/usr/bin/env python3
"""Read-only source composition proof; no Rust build or executable qualification."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
BASE = 'a82e00cc83f99b16249181c45710d962598577db'
sha = lambda text: hashlib.sha256(text.encode()).hexdigest()
def read(path):
    return (ROOT / path).read_text()
def original(path):
    return subprocess.check_output(['git', 'show', BASE + ':' + path], cwd=ROOT).decode()
def project(row, current):
    assert sha(current) == row['current_sha256'], row['source']
    for edit in row['edits']:
        assert edit['new'] and current.count(edit['new']) == 1, row['source']
        current = current.replace(edit['new'], edit['old'], 1)
    assert sha(current) == row['original_sha256'], row['source']
    return current

def joined_projection_root():
    source = read('tests/workspace/semantic_graph/source_inventory.rs')
    start = source.index('macro_rules! semantic_graph_projection_root_source_inventory')
    stop = source.index('\nmacro_rules!', start + 1)
    pieces = []
    for match in re.finditer(r'include_str!\(\s*"([^"]+)"\s*\)|"(\\n)"', source[start:stop]):
        pieces.append((ROOT / 'tests/workspace' / match[1]).read_text() if match[1] else '\n')
    return ''.join(pieces)

rows = json.loads((HERE / 'bridges.json').read_text())
assert len(rows) == 4 and len({row['source'] for row in rows}) == 4
restored = {}
for row in rows:
    current = joined_projection_root() if row['source'] == 'expected_projection::root_join' else read(row['source'])
    restored[row['source']] = project(row, current)
    for wrong in [current + ' ', '', restored[row['source']]]:
        assert sha(wrong) != row['current_sha256']

# The direct-reader assertions and all old expected bytes stay exactly intact.
p = 'src/cleanup_plan/replay/live_oracle/source_custody/a.rs'
assert read(p).replace('let mut current = crate::source_reflows_merge::current344::project("src/cleanup_plan.rs", include_str!("../../../../cleanup_plan.rs"));', 'let mut current = include_str!("../../../../cleanup_plan.rs").to_owned();') == original(p)
p = 'src/cleanup_plan/replay/live_oracle/source_custody/b.rs'
assert read(p).replace('let mut current = crate::source_reflows_merge::current344::project("src/hir/ids.rs", include_str!("../../../../hir/ids.rs"));', 'let mut current = include_str!("../../../../hir/ids.rs").to_owned();') == original(p)
cleanup = restored['src/cleanup_plan.rs']
cleanup = cleanup.replace('\n#[cfg(test)]\npub(crate) use replay::EpsilonAppCensus;\n', '')
cleanup = cleanup.replace('\n#[cfg(test)]\npub(crate) use replay::with_financed_test_oracles;\n', '')
assert cleanup == read('docs/evidence/opt810-live-oracle/src__cleanup_plan.rs.before')
ids = restored['src/hir/ids.rs']
ids = ids.replace('\n#[cfg(test)]\npub(crate) use metered_owner::general_matches as oracle_owner_matches_with_work;\n', '')
ids = ids.replace('\n#[cfg(test)]\npub(in crate::hir) mod fixed_operation_tests;\n', '')
assert ids == read('docs/evidence/opt810-live-oracle/src__hir__ids.rs.before')
assert restored['tests/native/string_settlement/codec_allocation/string_vectors.rs'] == read('docs/evidence/codec808-frontier-witness/original-string_vectors.rs')
root = restored['expected_projection::root_join']
assert root == read('docs/evidence/projection-facade-frontier-8bb-20261016/current-root.txt')
root_row = json.loads(read('docs/evidence/projection-facade-frontier-8bb-20261016/root-input.json'))
assert project(root_row, root) == read('docs/evidence/projection-facade-frontier-8bb-20261016/original-root.txt')

# Follow the newly wired real dispatch to the unchanged release normalizer.
path = 'src/source_reflows_merge/runtime_json.rs'
current = read(path)
for table in ['docs/evidence/a3-e47-final-custody-20261016/current-to-Previous16c.json', 'docs/evidence/main8a-independent-custody-20261016/current-to55e.json']:
    row = next(row for row in json.loads(read(table)) if row['source'] == path)
    current = project(row, current)
assert sha(current) == '048584a387e19ce44d46428072db42fedd4a4f94f67f3fac0622a2ce3fc0961f'
for owner in ['release_custody.rs', 'main8a_custody.rs', 'maina3_custody.rs']:
    assert read('src/source_reflows_merge/' + owner) == original('src/source_reflows_merge/' + owner)
# Every original fixed evidence table and historical input remains untouched.
assert not subprocess.check_output(['git', 'diff', BASE, '--name-only', '--',
    'docs/evidence/maina3-independent-custody-20261016',
    'docs/evidence/main44-independent-custody-20261016',
    'docs/evidence/opt810-live-oracle',
    'docs/evidence/projection-facade-frontier-8bb-20261016',
    'docs/evidence/codec808-frontier-witness'], cwd=ROOT).strip()
for guard in ['tests/hir_module_boundaries.rs', 'tests/workspace/semantic_graph/source_inventory.rs', 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
    text = read(guard)
    assert len(re.findall(r'include_str!\("[^"]*src/source_reflows_merge/current344.rs"\)', text)) == 1
for path in ['src/source_reflows_merge/current344.rs', 'src/source_reflows_merge.rs', 'src/cleanup_plan/replay/live_oracle/source_custody/a.rs', 'src/cleanup_plan/replay/live_oracle/source_custody/b.rs', 'tests/workspace/semantic_graph.rs']:
    assert len(read(path).splitlines()) <= 1500, path
print('PASS: 4 exact bridges, 12 refusal preimages, 5 historical owner inputs, 3 raw joins; source-only')
