"""Pure archived-wire/source controls. Never invokes producers or public main."""
from pathlib import Path
import ast, copy, hashlib, importlib.util, json
R = Path(__file__).resolve().parents[3]
D = Path(__file__).resolve().parent
p = R / 'scripts/prelude_forecast_committed_generation.py'
spec = importlib.util.spec_from_file_location('public_generation_controls', p)
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
old = m.original('original-committed67b-generation.py')
for name, expected in m.PINS.items():
    raw = (D / name).read_bytes(); assert hashlib.sha256(raw).hexdigest() == expected
    ast.parse(raw)
entry = (R / 'scripts/update-prelude-forecast-pins.py').read_text()
proof = json.loads((D / 'public-entry-finite-inverse.json').read_bytes())
assert hashlib.sha256(entry.encode()).hexdigest() == proof['current_sha256']
restored = entry
for edit in reversed(proof['edits']):
    assert restored.count(edit['after']) == 1
    restored = restored.replace(edit['after'], edit['before'], 1)
original = (D / 'original-update-prelude-forecast-pins.py').read_bytes()
assert restored.encode() == original
assert len(original.decode().splitlines()) == 227
assert hashlib.sha256(original).hexdigest() == proof['original_sha256']
def requires(source):
    return sum(isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == 'require' for node in ast.walk(ast.parse(source)))
assert requires(original) == 42 and requires(entry) == 44
assert hashlib.sha256(p.read_bytes()).hexdigest() in entry
prior, hashes = m.baseline(R)
assert len(m.validate_capture(prior, prior)) == 10
# Existing frozen actual c874 bytes are read-only observations, not produced by
# these controls. Their complete independent file binding is authenticated.
controls = Path('/Volumes/Semaprax/provenance/managed-wire-derived-length-inverse-c874-20261017')
binding_raw = (controls / 'inputs.json').read_bytes()
assert hashlib.sha256(binding_raw).hexdigest() == 'c6a726dc0e7f9f8411e1b05fed9f1f14c97553179bae7003a9c88c54b759a711'
binding = json.loads(binding_raw)
for name, expected in binding['files'].items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == expected
capture = Path('/Volumes/Semaprax/provenance/actual-managed-c8740045e-20261017')
rows = m.validate_capture(capture, prior)
assert len(rows) == 10
try:
    old.validate_whole_capture(capture, prior)
except AssertionError:
    old_refusal_retained = True
else:
    raise AssertionError('original stale-size validator unexpectedly admitted capture')
_, public = old.document(capture / 'wires/public-graph.json')
_, previous_public = old.document(prior / 'wires/public-graph.json')
restored_public = m.inverse(public, previous_public, 'graph', public, previous_public)
negative = 0
for row in rows:
    name = row['name']; _, value = old.document(capture / 'wires' / name)
    _, previous = old.document(prior / 'wires' / name); role = m.kind(name)
    for mode in ('semantic', 'count'):
        bad = copy.deepcopy(value)
        if mode == 'semantic':
            bad['workspace_revision'] += '!'; m.fix_size(bad, role)
        else:
            budget = bad['budget'] if role == 'graph' else bad['budget']['analysis']
            budget['used_output_bytes'] += 1
        key = 'graph_digest' if role == 'graph' else 'artifact_digest'
        bad[key] = old.domain_digest(bad, role)
        try:
            m.inverse(bad, previous, role, bad if role == 'graph' else public, restored_public)
        except AssertionError:
            negative += 1
        else:
            raise AssertionError('validly resealed semantic/count drift admitted')
assert negative == 20
# Replay all107 original edits against immutable Git source domains. This is a
# pure source composition, not actual current capture/build qualification.
source = m.original('original-source-inverse-composition.py')
actual_root = Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang')
actual_head = 'c8740045e33126a4375aadd1dc524a25be9245fc'
paths = ['tests/workspace/semantic_graph.rs', 'src/workspace_graph/type_proof_tests.rs']
before = {name: source.blob(actual_root, actual_head, name) for name in paths}
projection = m.source_projection(actual_root, actual_head, before, before)
assert len(projection['rows']) == 2
assert sum(row['legacy_inverse_count'] for row in projection['rows']) == 107
print(json.dumps({'state': 'PASS_PURE_SOURCE_ARCHIVED_WIRE_CONTROLS_ONLY', 'original_public_lines': 227, 'original_public_require_guards': 42, 'current_public_require_guards': 44, 'baseline_wires': 10, 'independently_bound_existing_actual_wires': 10, 'validly_resealed_refusals': negative, 'original_legacy_refusal_retained': old_refusal_retained, 'complete_original_source_inverse_edits': 107, 'new_capture': False, 'public_main_executed': False, 'pins_applied': False}))
