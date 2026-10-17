"""Authenticated committed generation and exact derived-length inverse.

No compiler, producer, source mutation, capture synthesis or pin application.
"""
from pathlib import Path
import copy, hashlib, importlib.util, json, re, subprocess, tomllib
D = Path(__file__).resolve().parents[1] / 'docs/evidence/opt872-public-generation-20261017'
PINS = {
    'original-update-prelude-forecast-pins.py': '19936e3603f390f9c821e8baeb99a0ea9a629373152c91e2a04a3113bef6eb6c',
    'original-committed67b-generation.py': '495cf823bff1b1b2123ad89e69b5ba4e9d42da504dde59f2039e2a989e07f842',
    'original-five-build-admission.py': 'dd822599720b7ed964043daada833c6c802bfbd5ac1c0e3fadd66ea104a0bcb0',
    'original-source-inverse-composition.py': '7219e0924d6dc42222796c96414a11a7aa90c222073e3ee188bb1a0197308946',
}
BASELINE_SHA = '9dad4670060347e50751307680c897dc9af9e2072623d35cab8bcf4924ec4373'
SOURCE_TABLE = 'docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json'
CHECKPOINT = '39130d9237121b61e40b47f95c072c09a456554e'
sha = lambda b: hashlib.sha256(b).hexdigest()

def original(name):
    path = D / name
    raw = path.read_bytes()
    assert sha(raw) == PINS[name], 'complete original program drift'
    spec = importlib.util.spec_from_file_location(name.replace('.', '_'), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def baseline(root):
    root = Path(root).resolve()
    path = root / 'docs/evidence/managed67b-semantic-history-20261016/transition-binding.json'
    raw = path.read_bytes()
    assert sha(raw) == BASELINE_SHA, 'committed baseline binding drift'
    old = original('original-committed67b-generation.py')
    binding = json.loads(raw, object_pairs_hook=old.unique)
    assert binding['actual_head'] == '67b1b02141f19040fdd1b45bc9bea16dcff5d74e'
    hashes = binding['actual_wire_sha256']
    capture = path.parent / 'actual67b'
    assert len(hashes) == 10
    assert sorted(p.name for p in (capture / 'wires').glob('*.json')) == sorted(hashes)
    for name, digest in hashes.items():
        assert Path(name).name == name
        raw, _ = old.document(capture / 'wires' / name)
        assert sha(raw) == digest, 'complete committed wire drift'
    return capture, hashes

def preflight(args, expected, build):
    """Fresh descriptor/source/five-command binding before generation admission."""
    record_path = args.workflow_record.resolve()
    raw = record_path.read_bytes()
    assert re.fullmatch('[0-9a-f]{64}', args.expected_workflow_record_sha256)
    assert sha(raw) == args.expected_workflow_record_sha256, 'reviewed workflow descriptor drift'
    old = original('original-committed67b-generation.py')
    record = json.loads(raw, object_pairs_hook=old.unique)
    root = Path(record['repository']).resolve()
    assert root == args.repo.resolve() == Path(expected['draft_repository']).resolve()
    assert record['frozen_source_head'] == args.expected_capture_head
    assert re.fullmatch('[0-9a-f]{40}', record['frozen_source_head'])
    assert Path(record['required_round']).resolve() == args.round.resolve()
    assert record['strict_checker_full_head_locked'] is True
    for name, digest in record['trusted_tool_sha256'].items():
        assert Path(name).name == name
        assert sha((record_path.parent / name).read_bytes()) == digest, 'reviewed external tool drift'
    required = {SOURCE_TABLE, 'tests/workspace/semantic_graph.rs', 'src/workspace_graph/type_proof_tests.rs'}
    assert set(record['old_source_sha256']) == required
    source = original('original-source-inverse-composition.py')
    for name, digest in record['old_source_sha256'].items():
        raw = (root / name).read_bytes()
        assert sha(raw) == digest, 'complete current source drift'
        assert raw == (record_path.parent / 'frozen-source-before' / name).read_bytes()
        assert raw == source.blob(root, args.expected_capture_head, name)
    table = json.loads((root / SOURCE_TABLE).read_bytes(), object_pairs_hook=old.unique)
    assert len(table) == 2 and sum(len(row['edits']) for row in table) == 107
    admission = original('original-five-build-admission.py')
    admission.admit_actual_build(record, expected, build, args.round)
    capture, hashes = baseline(root)
    assert Path(record['old_actual_capture']).resolve() == capture
    assert record['old_actual_capture_head'] == '67b1b02141f19040fdd1b45bc9bea16dcff5d74e'
    assert record['old_actual_all10_wire_sha256'] == hashes
    # The original strict external target/profile/artifact checks cover all four
    # producer executable roles, in addition to the unchanged public two roles.
    built = {str(Path(row['executable']).resolve()): row['executable_sha256'] for row in build['artifacts']}
    found, cli = {}, {}
    with (args.round / 'owning-build.jsonl').open() as stream:
        for line in stream:
            try:
                row = json.loads(line)
            except ValueError:
                continue
            if row.get('reason') != 'compiler-artifact' or not row.get('executable'):
                continue
            manifest = Path(row['manifest_path']).resolve()
            package = tomllib.loads(manifest.read_text())['package']['name']
            name, target_kind = row['target']['name'], row['target']['kind']
            is_test = row.get('profile', {}).get('test')
            executable = Path(row['executable']).resolve()
            role = name if package == 'semaprax' and is_test is True and ((name == 'semaprax' and target_kind == ['lib']) or (name == 'workspace' and target_kind == ['test'])) else None
            binrole = name if is_test is False and target_kind == ['bin'] and ((package == 'semaprax' and name == 'semaprax') or (package == 'semaprax-toolchain' and name == 'semaprax-full')) else None
            if role or binrole:
                assert manifest.is_relative_to(root), 'foreign producer package manifest'
                assert executable.is_relative_to(Path(record['current_five_build']['private_target']).resolve()) and executable.is_file()
                digest = admission.sha(executable)
                value = {'package': package, 'target': row['target'], 'profile_test': is_test, 'executable': str(executable), 'executable_sha256': digest, 'source_head': record['frozen_source_head'], 'raw_Cargo_log_sha256': build['owning_build_metadata_sha256'], 'completed_build_receipt_sha256': args.expected_build_receipt_sha256}
                dest, key = (found, role) if role else (cli, binrole)
                if role:
                    assert built[str(executable)] == digest
                if key in dest:
                    assert dest[key] == value
                dest[key] = value
    admission.admit_artifact_roles(found, cli, record, build, args.expected_build_receipt_sha256, build['owning_build_metadata_sha256'])
    return capture, hashes

def kind(name):
    return 'graph' if name in ('public-graph.json', 'projection-wire.json') else 'review' if name.endswith('review.json') else 'impact' if name.endswith('impact.json') else 'context'

def check_sizes(value, role, public):
    old = original('original-committed67b-generation.py')
    budget = value['budget'] if role == 'graph' else value['budget']['analysis']
    assert type(budget['used_output_bytes']) is int and budget['used_output_bytes'] == len(old.enc(value)), 'own serialized output count'
    if role != 'graph':
        assert type(value['budget']['workspace']['used_output_bytes']) is int
        assert value['budget']['workspace']['used_output_bytes'] == len(old.enc(public)), 'public serialized output count'
    if role == 'review':
        for key in ('context', 'impact'):
            assert type(budget['used_' + key + '_bytes']) is int
            assert budget['used_' + key + '_bytes'] == len(old.enc(value[key])), 'child serialized output count'
            check_sizes(value[key], key, public)

def fix_size(value, role):
    old = original('original-committed67b-generation.py')
    budget = value['budget'] if role == 'graph' else value['budget']['analysis']
    for _ in range(8):
        actual = len(old.enc(value))
        if budget['used_output_bytes'] == actual:
            return
        budget['used_output_bytes'] = actual
    raise AssertionError('output size fixed point did not settle')

def inverse(value, previous, role, public, restored_public):
    old = original('original-committed67b-generation.py')
    old.validate_bindings(value, role, public['graph_digest'])
    check_sizes(value, role, public)
    old.validate_bindings(previous, role, restored_public['graph_digest'])
    check_sizes(previous, role, restored_public)
    value = copy.deepcopy(value)
    if role == 'graph':
        value['budget']['used_builder_bytes'] = previous['budget']['used_builder_bytes']
    else:
        value['workspace_graph_digest'] = restored_public['graph_digest']
        value['budget']['workspace']['used_builder_bytes'] = restored_public['budget']['used_builder_bytes']
        value['budget']['workspace']['used_output_bytes'] = len(old.enc(restored_public))
        if role == 'review':
            for key in ('context', 'impact'):
                value[key] = inverse(value[key], previous[key], key, public, restored_public)
                value['budget']['analysis']['used_' + key + '_bytes'] = len(old.enc(value[key]))
    fix_size(value, role)
    key = 'graph_digest' if role == 'graph' else 'artifact_digest'
    value[key] = old.domain_digest(value, role)
    check_sizes(value, role, value if role == 'graph' else restored_public)
    old.validate_bindings(value, role, value['graph_digest'] if role == 'graph' else restored_public['graph_digest'])
    assert old.enc(value) == old.enc(previous), 'complete committed wire differs'
    # Run the complete unchanged old validator on its exact authenticated domain.
    assert old.enc(old.restore(value, previous, role, restored_public['graph_digest'], restored_public['graph_digest'])) == old.enc(previous)
    return value

def validate_capture(capture, prior):
    old = original('original-committed67b-generation.py')
    names = sorted(p.name for p in (prior / 'wires').glob('*.json'))
    assert len(names) == 10 and sorted(p.name for p in (capture / 'wires').glob('*.json')) == names
    _, public = old.document(capture / 'wires/public-graph.json')
    _, previous_public = old.document(prior / 'wires/public-graph.json')
    restored_public = inverse(public, previous_public, 'graph', public, previous_public)
    rows = []
    for name in names:
        before, previous = old.document(prior / 'wires' / name)
        actual, value = old.document(capture / 'wires' / name)
        restored = inverse(value, previous, kind(name), public, restored_public)
        assert old.enc(restored) == before
        rows.append({'name': name, 'prior_sha256': sha(before), 'actual_sha256': sha(actual), 'complete_prior_restored': True})
    return rows

def source_projection(root, head, before, after):
    source = original('original-source-inverse-composition.py')
    for name, raw in before.items():
        assert raw == source.blob(root, head, name), 'complete captured source-before drift'
    table = (Path(root) / SOURCE_TABLE).read_bytes()
    assert table == source.blob(root, head, SOURCE_TABLE)
    projection, retained, archives = source.compose(root, head, CHECKPOINT, SOURCE_TABLE, after)
    assert retained == table
    return projection
