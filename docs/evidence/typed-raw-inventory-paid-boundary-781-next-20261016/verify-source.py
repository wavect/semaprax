#!/usr/bin/env python3
"""Finite source custody only; no Rust compiler or authority generation."""
import argparse, hashlib, json, re, subprocess
from pathlib import Path
p = argparse.ArgumentParser()
p.add_argument('--repo', type=Path, default=Path.cwd())
root = p.parse_args().repo.resolve()
packet = root / 'docs/evidence/typed-raw-inventory-paid-boundary-781-next-20261016'
data = json.loads((packet / 'source-inverses.json').read_text())
sha = lambda b: hashlib.sha256(b).hexdigest()
def old(path):
    return subprocess.check_output(['git', 'show', data['base'] + ':' + path], cwd=root)
assert len(data['rows']) == 12 and len(data['new_sources']) == 8
for row in data['rows']:
    before = (packet / row['before_archive']).read_bytes()
    current = (root / row['source']).read_bytes()
    assert before == old(row['source'])
    assert sha(before) == row['before_sha256'] and sha(current) == row['after_sha256']
    a, b, at = before.decode(), current.decode(), row['offset']
    assert a[at:at + len(row['before'])] == row['before']
    assert b[at:at + len(row['after'])] == row['after']
    assert a[:at] + row['after'] + a[at + len(row['before']):] == b
    assert b[:at] + row['before'] + b[at + len(row['after']):] == a
caps = {}
for line in (root / 'tests/module-size-budget.tsv').read_text().splitlines():
    fields = line.split('\t')
    if len(fields) > 1 and fields[1].isdigit():
        caps[fields[0]] = int(fields[1])
for row in data['new_sources']:
    b = (root / row['source']).read_bytes()
    assert sha(b) == row['sha256']
    assert len(b.decode().splitlines()) == row['lines'] <= 1500
for row in data['rows']:
    if row['source'].endswith('.rs'):
        assert len((root / row['source']).read_text().splitlines()) <= caps.get(row['source'], 1500)
for row in data['unchanged_original_dependency_namespace']:
    assert (root / row['source']).read_bytes() == old(row['source'])
    assert sha(old(row['source'])) == row['sha256']
# Complete original builders, arbitrary callback API and full checker remain
# physical independent implementations. Only additive registration/reexports.
for path in ['src/cleanup.rs', 'src/cleanup/inventory_builder.rs', 'src/cleanup_plan/replay/inventory_coverage.rs']:
    assert (root / path).read_bytes().startswith(old(path))
child = 'src/cleanup_plan/replay/selected_context/child.rs'
assert (root / child).read_text() == old(child).decode().replace('pub(super) fn refuse(', 'pub(in super::super) fn refuse(', 1)
# Only the two selected paid entry calls change; old generic entries survive.
for path in ['src/cleanup_plan/replay/lexical_replay.rs', 'src/cleanup_plan/replay/scalar_replay.rs']:
    before = old(path).decode()
    assert (root / path).read_text() == before.replace('inventory_coverage::validate_inventory_coverage(program, function, budget)?;', 'inventory_coverage::paid::validate_inventory_coverage(program, function, budget)?;', 1)
new_root = root / 'src/cleanup/inventory_builder/typed.rs'
text = new_root.read_text()
for name in ['failure', 'receiving', 'queries', 'selected', 'traversal']:
    assert re.search(r'\bmod ' + name + ';', text)
    assert (new_root.parent / 'typed' / (name + '.rs')).is_file()
assert text.count('Err(InventoryFailure(Kind::Work))') == 1
assert 'match remaining.checked_sub(units)' in text and '*remaining = 0;' in text
assert 'physical.reserve(bytes).is_err()' not in text
assert 'match physical.reserve(bytes)' in text
assert 'TRACE_ENABLED.with' in text and 'enabled.replace(false)' in text
assert 'LOGICAL_TRACE.with' in text
# No Diagnostic can enter the opaque primitive API, including indirectly via
# generic classifier payloads. Work/physical tokens originate at real primitives.
for row in data['new_sources']:
    if '/typed/' in row['source'] or row['source'].endswith('/typed.rs'):
        code = re.sub(r'(?m)^\s*//.*$', '', (root / row['source']).read_text())
        assert 'Diagnostic' not in code and 'From<' not in code
        assert 'use super::*;' not in code
receiving = (new_root.parent / 'typed/receiving.rs').read_text()
assert 'CONTROL_INITIALIZERS: usize = 32;' in receiving
assert 'INITIAL_CONTROL_INITIALIZERS: usize = 43;' in receiving
assert 'const WORK: usize' in receiving and 'const PAYLOAD: usize' in receiving
for name, count in [('Controls', 32), ('Initial', 43)]:
    body = re.search(r'type ' + name + r"<[^=]+> = \((.*?)\n\);", receiving, re.S)[1]
    fields = [line for line in body.splitlines() if line.strip()]
    assert len(fields) == count
for label in ['REASON_OR_PHYSICAL', 'CLASSIFIER', 'REASON_OR_CLASSIFIER', 'REASON_OR_TYPE_DROP']:
    assert label in receiving
failure = (new_root.parent / 'typed/failure.rs').read_text()
assert 'pub(crate) struct InventoryFailure(pub(super) Kind);' in failure
assert 'Classifier(' not in failure and 'child_refusal::Refusal' not in failure
# Literal fixture custody: full existing source payloads, not rewritten syntax.
tests = (root / 'src/cleanup_plan/replay/inventory_coverage/typed_tests.rs').read_text()
assert tests.count('#[test]') == 2
literals = re.findall(r'r#"(.*?)"#', tests, re.S)
for source in ['src/cleanup_plan/replay/inventory_coverage.rs', 'src/cleanup_plan/replay/flat_records/tests.rs']:
    assert any(literal in old(source).decode() for literal in literals)
assert re.search(r'assert_eq!\(\s*actual_trace,\s*original_trace', tests)
assert 'assert_eq!(a, b,' in tests
assert 'Err(sentinel.clone())' in tests
assert 'first.floor, limit - 1' in tests
assert len(json.loads((packet / 'pending-selectors.json').read_text())) == 2
print(json.dumps({'status': 'PASS_SOURCE_ONLY', 'whole_inputs': 12, 'new_owners': 8, 'original_dependency_files': len(data['unchanged_original_dependency_namespace']), 'focused_owners_pending': 2, 'original_builder_callback_and_checker_preserved': True, 'issue_781': 'OPEN_RESIDUAL_SCHEMA_SUPPLEMENTAL_AND_OTHER_FAMILIES', 'rust_executed': False}, sort_keys=True))
