"""Prepare the next *captured* browser pin layer; never builds or captures.

Run from a clean target checkout. All bindings are explicit; --apply is required
for writes. Old evidence and exact graph/source receivers remain immutable.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def digest(wire):
    value = json.loads(wire)
    field = ',"graph_digest":' + json.dumps(value['graph_digest'])
    require(wire.count(field) == 1, 'ambiguous graph digest field')
    payload = wire.replace(field, '', 1).encode()
    return 'sha256:' + sha(b'semaprax.project-semantic-graph.artifact-digest.v1\0'
                           + struct.pack('<Q', len(payload)) + payload)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('root', 'capture', 'head', 'tree', 'binding-sha256', 'receipt',
                 'receipt-sha256', 'evidence-name'):
        parser.add_argument('--' + name, required=True)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    root, capture = Path(args.root).resolve(), Path(args.capture).resolve()
    def git(*command):
        return subprocess.check_output(['git', '-C', str(root), *command], text=True).strip()
    require(re.fullmatch(r'[a-z0-9][a-z0-9-]+', args.evidence_name), 'unsafe evidence name')
    require(git('rev-parse', 'HEAD') == args.head, 'target HEAD differs from explicit capture subject')
    require(git('rev-parse', 'HEAD^{tree}') == args.tree, 'target tree differs')
    require(not git('status', '--porcelain'), 'target checkout must be clean')
    binding_raw = (capture / 'binding.json').read_bytes()
    require(sha(binding_raw) == args.binding_sha256, 'capture binding hash differs')
    binding = json.loads(binding_raw)
    receipt_raw = Path(args.receipt).resolve().read_bytes()
    require(sha(receipt_raw) == args.receipt_sha256 == binding['build_receipt_sha256'],
            'owning build receipt hash differs')
    receipt = json.loads(receipt_raw)
    for record in (binding, receipt):
        require(record['source_subject'] == args.head and record['source_tree'] == args.tree,
                'capture/build subject or tree differs')
    results_raw = (capture / 'results.json').read_bytes()
    results = json.loads(results_raw)
    require(results['source_subject'] == args.head and results['complete'] is True,
            'capture is incomplete or bound to another subject')
    rows = results['results']
    require(len(rows) == 2 and {r['fixture'] for r in rows} == {'baseline', 'renamed'},
            'need exactly two fixture execution receipts')
    old_evidence = 'browser-67b1b-successor-20261016'
    evidence = root / 'docs/evidence' / args.evidence_name
    require(not evidence.exists(), 'evidence destination already exists')
    prefix = 'tests/wasm/scalar_browser_ci_contract/'
    require(not (root / prefix / 'next_actual.rs').exists(), 'next layer already installed')
    output, facts, bills = {}, [], set()
    old_graphs, new_graphs = [], []
    for fixture in ('baseline', 'renamed'):
        raw = (capture / f'calculator-{fixture}.json').read_bytes()
        require(raw.endswith(b'\n') and not raw.endswith(b'\n\n'), 'expected one trailing newline')
        wire = raw.decode()[:-1]
        value = json.loads(wire)
        row = next(r for r in rows if r['fixture'] == fixture)
        require(row['executed'] is True and row['exit_code'] == 0
                and row['resource_refusal'] is None and row['observation_errors'] == []
                and row['source_subject'] == args.head and row['source_binding_status'] == 'unchanged'
                and row['executable_sha256_after'] == binding['executable_sha256']
                and row['wire_sha256'] == sha(raw), 'fixture receipt does not authenticate raw wire')
        require(digest(wire) == value['graph_digest'], 'actual graph digest mismatch')
        previous = (root / 'docs/evidence' / old_evidence / 'actual-browser'
                    / f'calculator-{fixture}.json').read_text()[:-1]
        prior = json.loads(previous)
        require(digest(previous) == prior['graph_digest'], 'retained graph digest mismatch')
        bill = value['budget']['used_builder_bytes']
        require(type(bill) is int and bill > 0, 'invalid actual builder bill')
        field = '"used_builder_bytes":' + str(bill)
        require(wire.count(field) == 1, 'ambiguous actual bill')
        restored = wire.replace(field, '"used_builder_bytes":' + str(prior['budget']['used_builder_bytes']), 1)
        field = '"graph_digest":' + json.dumps(value['graph_digest'])
        require(restored.count(field) == 1, 'ambiguous actual graph digest')
        restored = restored.replace(field, '"graph_digest":' + json.dumps(prior['graph_digest']), 1)
        require(restored == previous, 'non-bill graph drift requires a separately reviewed implementation')
        bills.add(bill)
        old_graphs.append(prior['graph_digest'])
        new_graphs.append(value['graph_digest'])
        output[f'docs/evidence/{args.evidence_name}/actual-browser/calculator-{fixture}.json'] = raw.decode()
        facts.append({'fixture': fixture, 'wire_sha256': sha(raw), 'builder_bytes': bill,
                      'graph_digest': value['graph_digest'], 'complete_67b_preimage': True})
    require(len(bills) == 1, 'fixture bills differ')
    bill = bills.pop()
    sources = {}
    def change(path, old, new):
        text = sources.setdefault(path, (root / path).read_text())
        require(text.count(old) == 1, 'source shape differs: ' + path)
        sources[path] = text.replace(old, new, 1)
    runner = 'platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs'
    change(runner, '// Actual local67b compiler records include the complete authenticated graph bill.',
           '// Actual compiler records bind the explicit fresh capture and its complete graph bill.')
    change(runner, f'// docs/evidence/{old_evidence} binds this graph capture.',
           f'// docs/evidence/{args.evidence_name} binds this graph capture.')
    for old, new in zip(old_graphs, new_graphs):
        change(runner, old, new)
    parent = 'tests/wasm/scalar_browser_ci_contract.rs'
    change(parent, 'historical_builder::current67b_digest(', 'historical_builder::next_actual_digest(')
    change(parent, 'historical_builder::current67b_drift_controls(', 'historical_builder::next_actual_drift_controls(')
    builder = prefix + 'historical_builder.rs'
    sources[builder] = (root / builder).read_text() + '''
#[path = "next_actual.rs"]
mod next_actual;
pub(super) fn next_actual_digest(graph: &str) -> String { next_actual::digest(graph) }
pub(super) fn next_actual_drift_controls(graph: &str) { next_actual::drift_controls(graph); }
'''
    custody = prefix + 'browser_custody.rs'
    change(custody, 'pub(super) fn current67b_input(source: &str, current: &str) -> String {',
           'pub(super) fn current67b_input(source: &str, current: &str) -> String {\n'
           '    let restored = next_actual_input(source, current);\n    let current = restored.as_str();')
    change(custody, 'pub(super) fn current67b_runner75b(current: &str) -> String {',
           'pub(super) fn current67b_runner75b(current: &str) -> String {\n'
           f'    let restored = next_actual_input("{runner}", current);\n    let current = restored.as_str();')
    sources[custody] += f'''
pub(super) fn next_actual_input(source: &str, current: &str) -> String {{
    pin_project(include_str!("../../../docs/evidence/{args.evidence_name}/current-to-prior.json"), source, current)
}}
'''
    actual = prefix + 'actual_67b.rs'
    change(actual, f'    let runner = include_str!("../../../{runner}");',
           f'    let runner = include_str!("../../../{runner}");\n'
           f'    let runner = super::custody::next_actual_input("{runner}", runner);')
    # Reuse the reviewed full-wire adapter; its downstream receiver is exact67b.
    template = (root / actual).read_text()
    start = template.index('const MAIN75B:')
    end = template.index('fn restore(', start)
    template = template[:start] + template[end:]
    start = template.index('    let actual83ca = restore(')
    end = template.index('\n    result\n}', start)
    template = template[:start] + '''    let previous = restore(graph, ACTUAL83CA[index].strip_suffix('\\n').unwrap());
    let result = super::current67b_digest(&previous);
    super::current67b_drift_controls(&previous);''' + template[end:]
    template = template.replace('actual83ca-pin-successor-20261016', old_evidence)
    template = template.replace(old_evidence, args.evidence_name, 2)
    template = template.replace('3468381', 'NEXT_ACTUAL_BILL').replace('3468382', 'NEXT_DRIFT_BILL')
    template = template.replace('NEXT_ACTUAL_BILL', str(bill)).replace('NEXT_DRIFT_BILL', str(bill + 1))
    template = template.replace('3_468_381', f'{bill:_}')
    template = template.replace('actual67b', 'next_actual').replace('Actual67b', 'Next actual')
    template = template.replace(f'{old_evidence}/current-to-prior.json',
                                f'{args.evidence_name}/current-to-prior.json')
    template = template.replace('current67b_input(source, &current)', 'next_actual_input(source, &current)')
    # Fresh outer receiver uses live pins; retained67b uses its explicit preimage.
    output[prefix + 'next_actual.rs'] = template
    inverse = []
    for path, after in sources.items():
        before = (root / path).read_text()
        inverse.append({'source': path, 'current_sha256': sha(after.encode()),
                        'original_sha256': sha(before.encode()),
                        'edits': [{'new': after, 'old': before}]})
        output[path] = after
        output[f'docs/evidence/{args.evidence_name}/{Path(path).name}.before.txt'] = before
    for name, raw in [('binding.json', binding_raw), ('results.json', results_raw),
                      ('owning-build-receipt.json', receipt_raw)]:
        output[f'docs/evidence/{args.evidence_name}/{name}'] = raw.decode()
    output[f'docs/evidence/{args.evidence_name}/current-to-prior.json'] = json.dumps(inverse, indent=2) + '\n'
    proof = {'scope': 'static only; capture is historical producer evidence, not owner-test or browser pass',
             'head': args.head, 'tree': args.tree, 'capture': str(capture),
             'binding_sha256': args.binding_sha256, 'receipt_sha256': args.receipt_sha256,
             'results_sha256': sha(results_raw), 'wires': facts,
             'source_inverses': [{k: r[k] for k in ('source', 'current_sha256', 'original_sha256')} for r in inverse]}
    output[f'docs/evidence/{args.evidence_name}/static-proof.json'] = json.dumps(proof, indent=2) + '\n'
    print(json.dumps({'apply': args.apply, 'proof': proof, 'files': list(output)}, indent=2))
    if args.apply:
        for path, text in output.items():
            destination = root / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(text)


if __name__ == '__main__':
    main()
