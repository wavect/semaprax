"""Pure offline controls for the actual unchanged refresh validate_dirty AST."""
from pathlib import Path
import ast, hashlib, json, types
D = Path(__file__).resolve().parent
raw = (D / 'refresh-entry.py').read_bytes()
assert hashlib.sha256(raw).hexdigest() == '2d945db9a3b940b4e992458a40375a9a2a193bc38ec46faaf6895469da2aee3b'
old_raw = (D / 'refresh-write-paths-before-dee6.json').read_bytes()
assert hashlib.sha256(old_raw).hexdigest() == '834da9c392dea7bc291ab8ba55254f94c86c9b0397c19a0138e218bac66d0f02'
old = json.loads(old_raw)
current = json.loads((D / 'refresh-write-paths.json').read_bytes())
ADDED = ['docs/evidence/a3-e47-final-custody-20261016/before/Previous16c/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs', 'docs/evidence/a3-e47-final-custody-20261016/before/Previous16c/docs/DEVELOPMENT.md', 'docs/evidence/a3-e47-final-custody-20261016/before/Previous16c/src/cleanup/schema_summary/tests.rs', 'docs/evidence/a3-e47-final-custody-20261016/before/Previous16c/src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs', 'docs/evidence/a3-e47-final-custody-20261016/before/Previous16c/src/cleanup_plan/replay/flat_records/tests/receiving_path_boundary.rs', 'docs/evidence/a3-e47-final-custody-20261016/before/Previous16c/src/cleanup_plan/replay/live_oracle/source_custody/current_view.rs', 'docs/evidence/a3-e47-final-custody-20261016/before/Previous16c/src/hir/resolve_program/tests/reservation_stages.rs', 'docs/evidence/main67107-fronts-6e45-20261017/front-binding-receipt.json']
assert current['paths'] == old['paths'] + ADDED
assert {k:v for k,v in current.items() if k != 'paths'} == {k:v for k,v in old.items() if k != 'paths'}
assert len(current['paths']) == len(set(current['paths'])) == 57
assert not any(any(c in p for c in '*?[]') for p in current['paths'])
function = next(n for n in ast.parse(raw).body if isinstance(n, ast.FunctionDef) and n.name == 'validate_dirty')
code = compile(ast.Module(body=[function], type_ignores=[]), str(D / 'refresh-entry.py') + ':actual-validate-dirty', 'exec')
commands = [
 ['git','diff','--name-only','-z'],
 ['git','diff','--cached','--name-only','-z'],
 ['git','ls-files','--others','--exclude-standard','-z'],
 ['git','status','--porcelain=v1','--untracked-files=all'],
]
def run(paths, allowed=None, nonregular=(), symlinks=()):
    groups = [paths[::3], paths[1::3], paths[2::3]]
    status = ''.join(' M ' + p + '\n' for p in paths)
    calls = []
    class File:
        def __init__(self, path): self.path = path
        def is_file(self): return self.path not in nonregular
        def is_symlink(self): return self.path in symlinks
    class Repo:
        def __truediv__(self, path): return File(path)
    repo = Repo()
    def check_output(command, cwd, text=False):
        i = len(calls)
        assert command == commands[i] and cwd is repo
        calls.append(command)
        if i == 3:
            assert text
            return status
        assert not text
        return ('\0'.join(groups[i]) + ('\0' if groups[i] else '')).encode()
    ns = {'repo':repo, 'allowed':set(current['paths']) if allowed is None else allowed,
          'subprocess':types.SimpleNamespace(check_output=check_output)}
    exec(code, ns)
    result = ns['validate_dirty']()
    assert result == status and len(calls) == 4
    return result
positive = 0
for paths in [[], old['paths'], current['paths'], *[[p] for p in ADDED]]:
    run(paths)
    positive += 1
negative = 0
def refuses(paths, **kwargs):
    global negative
    try: run(paths, **kwargs)
    except AssertionError as error:
        assert error.args and isinstance(error.args[0], tuple)
        assert error.args[0][0] in {'unexpected dirty refresh paths', 'not an ordinary refresh output'}
        negative += 1
    else: raise AssertionError('actual validate_dirty admitted hostile status')
refuses([ADDED[-1] + '.near'])
refuses([ADDED[0].replace('before/Previous16c/', 'before/Previous16c-neighbor/', 1)])
refuses([ADDED[-1].replace('docs/evidence/', 'docs/evidence-neighbor/', 1)])
refuses([old['paths'][0]], allowed=set(current['paths']) - {old['paths'][0]})
refuses([ADDED[0]], nonregular={ADDED[0]})
refuses([ADDED[0]], symlinks={ADDED[0]})
print(json.dumps({'status':'PASS_PURE_ACTUAL_VALIDATE_DIRTY_AST_CONTROLS', 'original_paths':49,
                  'literal_added_paths':8, 'positive':positive, 'negative':negative,
                  'refresh_or_runtime_executed':False, 'validator_unchanged':True}))
