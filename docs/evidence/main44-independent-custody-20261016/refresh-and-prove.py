"""Bounded static Main44 source/JSON proof. --refresh binds final joined bytes.
Never compiles, invokes the compiler, runs Rust tests, or acquires producer authority.
"""
from pathlib import Path
import argparse
import hashlib
import json
import re
import posixpath

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = Path(__file__).resolve().parent

def sha(value):
    return hashlib.sha256(value).hexdigest()

def read(path):
    return (ROOT / path).read_text()

def invert(record, current):
    assert sha(current.encode()) == record['after_sha256'], record['path']
    for edit in record['inverse']:
        assert current.count(edit['after']) == 1, record['path']
        current = current.replace(edit['after'], edit['before'], 1)
    assert sha(current.encode()) == record['before_sha256'], record['path']
    return current

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--refresh', action='store_true')
args = parser.parse_args()
physical_paths = json.loads((EVIDENCE / 'physical-paths.json').read_text())
keys = json.loads((EVIDENCE / 'input-keys.json').read_text())
assert keys == sorted(set(keys))
if args.refresh:
    rows = []
    for path in keys:
        old = (EVIDENCE / 'raw' / (path + '.txt')).read_text()
        new = read(physical_paths.get(path, path))
        rows.append({'source': path, 'current_sha256': sha(new.encode()),
                     'original_sha256': sha(old.encode()),
                     'edits': [{'new': new, 'old': old}]})
    raw = (json.dumps(rows, indent=2) + '\n').encode()
    (EVIDENCE / 'current-to-main44.json').write_bytes(raw)
    path = ROOT / 'src/source_reflows_merge/main44_custody.rs'
    code = path.read_text()
    spec = ('// MAIN44_INPUT_SPEC\n    Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), '
            '"/docs/evidence/main44-independent-custody-20261016/current-to-main44.json"), '
            f'length: {len(raw)}, sha256: {list(hashlib.sha256(raw).digest())} }}')
    code, count = re.subn(r'// MAIN44_INPUT_SPEC\n    Spec \{[^\n]+\}', lambda _: spec, code)
    assert count == 1
    path.write_text(code)
rows = json.loads((EVIDENCE / 'current-to-main44.json').read_text())
assert [row['source'] for row in rows] == keys
for row in rows:
    current = read(physical_paths.get(row['source'], row['source']))
    assert sha(current.encode()) == row['current_sha256'], row['source']
    assert len(row['edits']) == 1 and current == row['edits'][0]['new']
    old = row['edits'][0]['old']
    assert sha(old.encode()) == row['original_sha256']
    assert old == (EVIDENCE / 'raw' / (row['source'] + '.txt')).read_text()
raw_sources = {row['source']: row['edits'][0]['old'] for row in rows}
facts = []
for name, count, original_sha in [
    ('compact-inverse7.json', 7, '99a8a6c0ba7694c8268f54a10e69ad9d70ffad90db354ac60b80d26a45ae6f80'),
    ('receiver-inverse12.json', 12, '7548f9e7d429e7ef8fb25c422d9b2614d088681a6342a3e1195749381d4bde12'),
]:
    raw = (EVIDENCE / name).read_bytes()
    assert sha(raw) == original_sha
    records = json.loads(raw)
    assert len(records) == count
    for record in records:
        restored = invert(record, raw_sources[record['path']])
        parent = 'compact-staged-observations-v1' if count == 7 else 'receiver-source-custody-v1'
        assert restored == read('docs/evidence/' + parent + '/' + record['archive'])
    facts.append({'table': name, 'sha256': original_sha, 'rows': count,
                  'complete_original_inverse_and_archive_match': True})
namespace = json.loads((EVIDENCE / 'namespace-bindings.json').read_text())
for binding in namespace:
    assert sha(binding['original'].encode()) == binding['original_sha256']
    current = read(binding['compiled_path'])
    assert current == binding['compiled'], binding['compiled_path']
    assert sha(current.encode()) == binding['compiled_sha256']
    assert binding['original'] == raw_sources.get(binding['original_path'], binding['original'])
    # Apart from the namespace root and vector module registrations, copied
    # predicates, expectations, test data, helper bodies and old arms are exact.
    if binding['original_path'].endswith('/path_array.rs'):
        expected = binding['original']
        expected_start = expected.index('use super::*;') + len('use super::*;')
        actual_start = current.index('use super::*;') + len('use super::*;')
        original_tail = expected[expected_start:]
        # All original helper/test declarations remain one complete ordered span.
        assert current.count(original_tail) == 1
        assert current[:actual_start] == expected[:expected_start]
    else:
        originals = list(re.finditer(r'include_str!\("([^"\n]+)"\)', binding['original']))
        targets = list(re.finditer(r'include_str!\(concat!\(env!\("CARGO_MANIFEST_DIR"\), "/([^"\n]+)"\)\)', current))
        assert len(originals) == len(targets), binding['compiled_path']
        restored = current
        for original_include, target in zip(originals, targets):
            original_path = posixpath.normpath(posixpath.join(posixpath.dirname(binding['original_path']), original_include[1]))
            frozen_path = 'docs/evidence/main44-independent-custody-20261016/raw/' + original_path + '.txt'
            expected_content = read(frozen_path) if (ROOT / frozen_path).exists() else read(original_path)
            assert read(target[1]) == expected_content, original_path
            restored = restored.replace(target[0], original_include[0], 1)
        if binding['original_path'].endswith('/sequence_vector.rs'):
            restored = restored.replace('#[path = "vector_before.rs"]\n', '').replace('#[path = "vector_tests.rs"]\n', '')
        assert restored == binding['original'], binding['compiled_path']
# Full incoming current functions retain every condition and operation. The
# sole body adapter sets a test-only flag around the whole indexed HIR receiver.
root_source = raw_sources['src/cleanup_plan/replay.rs']
def original_function(name):
    start = re.search(r'fn ' + name + r'(?:<[^>]*>)?\(', root_source).start()
    return root_source[start:root_source.index('\n}\n', start) + 3]
expected_current = '//! Complete incoming Main44 receivers; only namespace bindings are adapted.\nuse super::*;\n'
for name in ['sequence_skeleton_paths', 'append_expr_paths', 'split_status_paths', 'split_contract']:
    visibility = 'pub(in crate::cleanup_plan::replay)' if name == 'sequence_skeleton_paths' else 'pub(super)'
    expected_current += original_function(name).replace('fn ' + name + '(', visibility + ' fn ' + name + '(', 1) + '\n'
expected_current += original_function('hir_skeleton_paths_with_index_storage').replace('fn hir_skeleton_paths_with_index_storage', 'pub(super) fn hir_skeleton_paths_with_index_storage', 1) + '\n'
start = expected_current.index('pub(super) fn hir_skeleton_paths_with_index_storage')
body = expected_current.index('{', start) + 1
expected_current = expected_current[:body] + '\n    with_active(|| {\n' + expected_current[body:]
end = expected_current.rfind('\n}')
expected_current = expected_current[:end] + '\n    })' + expected_current[end:]
assert read('src/cleanup_plan/replay/path_array/main44/current.rs') == expected_current.rstrip() + '\n'
inventory = json.loads((EVIDENCE / 'new-source-inventory.json').read_text())
for source in inventory['sources']:
    assert (ROOT / source).is_file(), source
    for owner in inventory['inventories']:
        assert source in read(owner), (owner, source)
proof = {'scope': 'static source/JSON proof only; no build, compiler, browser or test execution',
         'incoming_subject': '44f52ebee749b2877ba001c5398acbf803a277a1',
         'complete_source_views': len(rows), 'namespace_adapters': len(namespace),
         'new_sources_in_every_authority_inventory': len(inventory['sources']),
         'complete_independent_tables': facts}
print(json.dumps(proof, indent=2))
