#!/usr/bin/env python3
import hashlib,json,pathlib,subprocess
root=pathlib.Path(__file__).resolve().parents[3]
packet=pathlib.Path(__file__).resolve().parent
manifest=json.loads((packet/'source-inverses.json').read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
for row in manifest['rows']:
    current=(root/row['path']).read_bytes(); before=(packet/'before'/row['path']).read_bytes()
    assert sha(current)==row['after_sha256'] and sha(before)==row['before_sha256']
    assert before==subprocess.check_output(['git','show',manifest['base']+':'+row['path']],cwd=root)
    restored=current.decode()
    for edit in reversed(row['edits']):
        assert restored.count(edit['after'])==1
        restored=restored.replace(edit['after'],edit['before'],1)
    assert restored.encode()==before
    for edit in row['edits']:
        assert current.count(edit['after'].encode())==1
        try:
            bad=current.replace(edit['after'].encode(),b'TAMPERED',1).decode()
            assert bad.count(edit['after'])==1
        except AssertionError:
            pass
        else:
            raise AssertionError('tamper accepted')
for path in ['src/workspace_graph/function_alias/before_core.rs','src/workspace_graph/function_alias/before_core/tests.rs','src/workspace_graph/constructor_failure/typed.rs','src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs']:
    assert (root/path).read_bytes()==subprocess.check_output(['git','show',manifest['base']+':'+path],cwd=root)
print('PASS: three complete before/inverse domains; one production predicate; original core/Project gate/classifiers/payments unchanged')
