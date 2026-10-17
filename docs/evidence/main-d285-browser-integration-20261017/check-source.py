#!/usr/bin/env python3
"""Read-only source/captured-artifact custody proof, never runtime qualification."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

BASE = '3d77c9566508782b607bed64735a9efe81e15c88'
MAIN = 'd285dc5375ba78e63db8ad2b77ebad9069db3bcb'
OWNER = 'tests/wasm/scalar_browser_ci_contract/historical_builder.rs'
RUNNER = 'platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs'
BEFORE = 'tests/wasm/scalar_browser_ci_contract/before_7d34.rs'
D = 'docs/evidence/main-d285-browser-integration-20261017'


def digest(graph):
    value = json.loads(graph)
    field = ',"graph_digest":' + json.dumps(value['graph_digest'], separators=(',', ':'))
    assert graph.count(field) == 1
    payload = graph.replace(field, '', 1).encode()
    return 'sha256:' + hashlib.sha256(
        b'semaprax.project-semantic-graph.artifact-digest.v1\0' + len(payload).to_bytes(8, 'little') + payload).hexdigest()


def restore(graph, prior):
    actual, previous = json.loads(graph), json.loads(prior)
    assert digest(graph) == actual['graph_digest']
    assert digest(prior) == previous['graph_digest']
    bill = actual['budget']['used_builder_bytes']
    assert type(bill) is int and 0 < bill <= 67108864
    field = '"used_builder_bytes":' + str(bill)
    assert graph.count(field) == 1
    result = graph.replace(field, '"used_builder_bytes":' + str(previous['budget']['used_builder_bytes']), 1)
    field = '"graph_digest":' + json.dumps(actual['graph_digest'])
    assert result.count(field) == 1
    result = result.replace(field, '"graph_digest":' + json.dumps(previous['graph_digest']), 1)
    assert result == prior
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--incoming-inputs', type=Path)
    args = parser.parse_args()
    git = lambda ref, path: subprocess.check_output(['git', 'show', ref+':'+path], cwd=args.repo)
    ours, incoming = git(BASE, OWNER), git(MAIN, OWNER)
    current = (args.repo / OWNER).read_bytes()
    old_entry = b'''pub(super) fn current_browser_digest(graph: &str, role: &str) -> String {
    current_browser_with_runner(graph, role, include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs"))
}
'''
    wrapped = b'mod retained_current_entry {\nuse super::current_browser_with_runner;\n' + old_entry + b'}\n'
    marker = b'\n// Independent complete incoming MAIN7d34 receiver.'
    assert current.count(marker) == 1
    old_part, extra = current.split(marker, 1)
    assert old_part.count(wrapped) == 1 and old_part.replace(wrapped, old_entry, 1) == ours
    assert (args.repo / D / 'ours-historical-builder.before.rs.txt').read_bytes() == ours
    assert (args.repo / D / 'main7d34-receiver.before.rs.txt').read_bytes() == incoming
    header = b'mod incoming_main7d34 {\n'
    end = b'}\n\nfn current_browser_all_with_runner'
    assert extra.count(header) == 1 and extra.count(end) == 1
    body = extra.split(header, 1)[1].split(end, 1)[0]
    binding = b'use super::before_7d34;'
    original = b'#[path = "before_7d34.rs"]\nmod before_7d34;'
    archived = b'include_str!("../../../docs/evidence/main-d285-browser-integration-20261017/main7d34-runner.before.mjs")'
    raw = b'include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs")'
    assert body.count(binding) == body.count(archived) == 1
    assert body.replace(binding, original, 1).replace(archived, raw, 1) == incoming
    runner = git(MAIN, RUNNER)
    assert (args.repo / D / 'main7d34-runner.before.mjs').read_bytes() == runner
    before = git(MAIN, BEFORE)
    assert len(before.splitlines()) == 129
    physical = False
    if args.incoming_inputs:
        assert (args.incoming_inputs / RUNNER).read_bytes() == runner
        assert (args.incoming_inputs / BEFORE).read_bytes() == before
        physical = True
    # Original raw3 domain/selector/cap carriers and all existing browser
    # custody readers remain EXACT base bytes in this private source patch.
    for path in ['tests/wasm/scalar_browser_ci_contract/browser_custody.rs',
                 'tests/wasm/scalar_browser_ci_contract/next_actual.rs',
                 'tests/wasm/scalar_browser_ci_contract/before_671.rs']:
        assert (args.repo / path).read_bytes() == git(BASE, path)
    artifacts = 0
    for role in ['baseline', 'renamed']:
        current_graph = git(MAIN, 'docs/evidence/browser-graph-pins-7d34f1269/current-hosted-calculator-'+role+'.json').decode().removesuffix('\n')
        row = json.loads(current_graph)
        assert row['budget']['used_builder_bytes'] == 2696104
        marker = 'name: "project-'+role+'",'
        text = runner.decode()
        assert text.count(marker) == 1
        record = text.split(marker)[1].split('\n  },')[0]
        for name, field in [('project_revision', 'project_revision'), ('workspace_revision', 'workspace_revision'), ('project_graph_digest', 'graph_digest')]:
            assert record.count(name+': "'+row[field]+'"') == record.count(name+':') == 1
        for prior_path in [
            'docs/evidence/browser-graph-pins-67107c7c5/current-hosted-calculator-'+role+'.json',
            'docs/evidence/browser-next-actual-218a9-20261016/actual-browser/calculator-'+role+'.json',
            'docs/evidence/browser-graph-pins-7d34f1269/current-hosted-calculator-'+role+'.json']:
            prior_ref = MAIN if '7d34f1269' in prior_path else BASE
            prior = git(prior_ref, prior_path).decode().removesuffix('\n')
            restore(current_graph, prior)
            artifacts += 1
            changed = current_graph.replace('"used_call_sites":10', '"used_call_sites":11', 1)
            changed = changed.replace('"graph_digest":'+json.dumps(row['graph_digest']),
                                      '"graph_digest":'+json.dumps(digest(changed)), 1)
            try:
                restore(changed, prior)
            except AssertionError:
                pass
            else:
                raise AssertionError('rehashed metadata mutation accepted')
    print(json.dumps({'scope':'SOURCE_AND_RETAINED_CAPTURE_BYTES_ONLY_NOT_RUST_OR_RUNTIME',
        'complete_ours_restored':True,'complete_main_program_restored':True,
        'whole_stored_wire_restorations':artifacts,'rehashed_metadata_refusals':artifacts,
        'physical_incoming_runner_and_new_owner_checked':physical,'old_custody_readers_unchanged':True}))


if __name__ == '__main__':
    main()
