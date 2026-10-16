"""Read-only byte and digest proof; never executes a compiler or a Rust test."""
from pathlib import Path
import hashlib
import json
import struct

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = Path(__file__).resolve().parent

def sha(data):
    return hashlib.sha256(data).hexdigest()

def graph_digest(wire):
    parsed = json.loads(wire)
    field = ',"graph_digest":' + json.dumps(parsed['graph_digest'])
    assert wire.count(field) == 1
    payload = wire.replace(field, '', 1).encode()
    return 'sha256:' + sha(b'semaprax.project-semantic-graph.artifact-digest.v1\0' + struct.pack('<Q', len(payload)) + payload)

def inverse(table, source, current):
    rows = [r for r in json.loads(table.read_text()) if r['source'] == source]
    assert len(rows) <= 1
    if not rows:
        return current
    row = rows[0]
    assert sha(current.encode()) == row['current_sha256'], (str(table), source, 'physical source')
    for edit in row['edits']:
        assert current.count(edit['new']) == 1
        current = current.replace(edit['new'], edit['old'], 1)
    assert sha(current.encode()) == row['original_sha256'], (str(table), source, 'original source')
    return current

binding_raw = (EVIDENCE / 'binding.json').read_bytes()
results_raw = (EVIDENCE / 'results.json').read_bytes()
binding = json.loads(binding_raw)
results = json.loads(results_raw)
assert binding['source_subject'] == results['source_subject'] == '67b1b02141f19040fdd1b45bc9bea16dcff5d74e'
assert results['complete'] is True
assert len(results['results']) == 2
assert {r['fixture'] for r in results['results']} == {'baseline', 'renamed'}
facts = []
for fixture in ['baseline', 'renamed']:
    actual_path = EVIDENCE / 'actual-browser' / f'calculator-{fixture}.json'
    actual_raw = actual_path.read_bytes()
    receipt = next(r for r in results['results'] if r['fixture'] == fixture)
    assert receipt['executed'] is True and receipt['exit_code'] == 0
    assert receipt['resource_refusal'] is None and receipt['observation_errors'] == []
    assert receipt['wire_sha256'] == sha(actual_raw)
    assert receipt['source_subject'] == binding['source_subject']
    assert receipt['source_binding_status'] == 'unchanged'
    assert receipt['executable_sha256_after'] == binding['executable_sha256']
    actual = actual_raw.decode().removesuffix('\n')
    value = json.loads(actual)
    assert graph_digest(actual) == value['graph_digest']
    assert value['budget']['used_builder_bytes'] == 3468381
    domains = []
    # Independently authenticated old wires, not newly fabricated captures.
    paths = [
        ROOT / 'docs/evidence/actual83ca-pin-successor-20261016/actual-browser' / f'calculator-{fixture}.json',
        ROOT / 'docs/evidence/browser-main75b-independent-20261016' / f'actual55e-calculator-{fixture}.json',
    ]
    paths += sorted((ROOT / 'docs/evidence').glob(f'browser-graph-pins-*/**/*calculator-{fixture}.json'))
    for previous_path in paths:
        previous = previous_path.read_text().removesuffix('\n')
        prior = json.loads(previous)
        assert graph_digest(previous) == prior['graph_digest'], str(previous_path)
        bill = '"used_builder_bytes":3468381'
        assert actual.count(bill) == 1
        restored = actual.replace(bill, '"used_builder_bytes":' + str(prior['budget']['used_builder_bytes']), 1)
        field = '"graph_digest":' + json.dumps(value['graph_digest'])
        assert restored.count(field) == 1
        restored = restored.replace(field, '"graph_digest":' + json.dumps(prior['graph_digest']), 1)
        assert restored == previous, str(previous_path)
        domains.append({'path': str(previous_path.relative_to(ROOT)), 'raw_sha256': sha(previous_path.read_bytes()), 'builder': prior['budget']['used_builder_bytes'], 'complete_wire_restored': True})
    facts.append({'fixture': fixture, 'raw_sha256': sha(actual_raw), 'graph_digest': value['graph_digest'], 'builder': 3468381, 'complete_historical_domains': domains})

rows = json.loads((EVIDENCE / 'current-to-prior.json').read_text())
source_facts = []
chain = [
    'browser-main75b-independent-20261016/current-to-prior.json',
    'main8a-independent-custody-20261016/browser-current-to55e.json',
    'main65-private-merge-20261016/browser-current-to-75d8.json',
    'actual83ca-pin-successor-20261016/browser-current-to-83ca.json',
    'main-b80-browser-custody-20261016/current-to-df2.json',
    'actual47f-pin-fixture-successor-20261016/current-to-47f.json',
]
for row in rows:
    source = row['source']
    current = (ROOT / source).read_text()
    previous = inverse(EVIDENCE / 'current-to-prior.json', source, current)
    assert previous == (EVIDENCE / (Path(source).name + '.before.txt')).read_text()
    # Custody is itself restored at the outer seam; no old receiver routes
    # its own source through every later branch projection.
    if not source.endswith('/browser_custody.rs'):
        for table in chain:
            previous = inverse(ROOT / 'docs/evidence' / table, source, previous)
    source_facts.append({'source': source, 'current_sha256': row['current_sha256'], 'exact_preimage_sha256': row['original_sha256'], 'retained_inverse_chain_verified': True})
runner = 'platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs'
derived = inverse(EVIDENCE / 'current-to-retained75b-runner.json', runner, (ROOT / runner).read_text())
assert derived == (EVIDENCE / 'derived-retained75b-runner.mjs.txt').read_text()
for fixture in ['baseline', 'renamed']:
    old = json.loads((ROOT / f'docs/evidence/browser-graph-pins-75b314f70/current-hosted-calculator-{fixture}.json').read_text())
    assert old['graph_digest'] in derived

print(json.dumps({'scope': 'static byte and digest proof only; no compiler/build/test/browser execution', 'source_subject': binding['source_subject'], 'source_tree': binding['source_tree'], 'binding_sha256': sha(binding_raw), 'results_sha256': sha(results_raw), 'wires': facts, 'source_inverses': source_facts, 'derived75b_runner_sha256': sha(derived.encode())}, indent=2))
