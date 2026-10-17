"""Read-only source and immutable-wire proof; does not execute Rust/capture."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3]
D=Path(__file__).resolve().parent
BASE='754f5c5c3c25facf8ea6e68ac14ce0492a3857ff'
sha=lambda b:hashlib.sha256(b).hexdigest()
proof=json.loads((D/'full-source-inverse.json').read_bytes())
for row in proof['rows']:
    current=(R/row['path']).read_bytes();before=(R/row['before_archive']).read_bytes()
    assert before==subprocess.check_output(['git','show',BASE+':'+row['path']],cwd=R)
    assert len(before)==row['before_len'] and sha(before)==row['before_sha256']
    assert len(current)==row['current_len'] and sha(current)==row['current_sha256']
    assert row['inverse']==[{'at':0,'remove_len':len(current),'remove_sha256':sha(current),'insert_archive':row['before_archive']}]
runner=(D/'main671-runner.before.mjs').read_bytes()
assert runner==subprocess.check_output(['git','show',BASE+':platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs'],cwd=R)
assert sha(runner)=='11cfaa78b2fb7df873e2d2f6b5b66c9a06ac33cdb5448cf12bb5ce6a1f216b47'
old=(D/'before/tests__wasm__scalar_browser_ci_contract__historical_builder.rs').read_bytes()
new=(R/'tests/wasm/scalar_browser_ci_contract/historical_builder.rs').read_bytes()
prefix=b'\n// Current raw-pin admission followed by BOTH complete retained browser trees.'
retained,added=new.split(prefix,1)
old_runner=b'    let runner = include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs");'
archive_runner=b'    let runner = include_str!("../../../docs/evidence/browser-current-both-retained-754f-20261017/main671-runner.before.mjs");'
head,tail=old.split(b'mod incoming_main671 {',1)
assert tail.count(old_runner)==1
assert retained==head+b'mod incoming_main671 {'+tail.replace(old_runner,archive_runner,1)
for path in ['tests/wasm/scalar_browser_ci_contract.rs','tests/wasm/scalar_browser_ci_contract/browser_custody.rs']:
    before=(D/'before'/path.replace('/','__')).read_bytes()
    if path.endswith('browser_custody.rs'):
        needle=b'        "tests/wasm/scalar_browser_ci_contract/historical_builder.rs" => include_str!("historical_builder.rs"),'
        after=before.replace(needle,b'        "tests/wasm/scalar_browser_ci_contract.rs" => include_str!("../scalar_browser_ci_contract.rs"),\n'+needle,1)
    else:
        after=before.replace(b'historical_builder::next_actual_digest(revision.semantic_graph())',b'historical_builder::current_browser_digest(revision.semantic_graph(), name)',1).replace(b'historical_builder::next_actual_drift_controls(revision.semantic_graph());',b'historical_builder::current_browser_drift_controls(revision.semantic_graph(), name);',1)
    assert after==(R/path).read_bytes()
def digest(wire):
    value=json.loads(wire);field=(',"graph_digest":'+json.dumps(value['graph_digest'])).encode()
    assert wire.count(field)==1
    payload=wire.replace(field,b'',1)
    return 'sha256:'+hashlib.sha256(b'semaprax.project-semantic-graph.artifact-digest.v1\0'+len(payload).to_bytes(8,'little')+payload).hexdigest()
wires=[]
for name in ['baseline','renamed']:
    paths=[f'docs/evidence/browser-graph-pins-67107c7c5/current-hosted-calculator-{name}.json',f'docs/evidence/browser-next-actual-218a9-20261016/actual-browser/calculator-{name}.json']
    a,b=[(R/path).read_bytes().removesuffix(b'\n')for path in paths]
    av,bv=json.loads(a),json.loads(b)
    assert digest(a)==av['graph_digest'] and digest(b)==bv['graph_digest']
    field=f'"used_builder_bytes":{av["budget"]["used_builder_bytes"]}'.encode();assert a.count(field)==1
    restored=a.replace(field,f'"used_builder_bytes":{bv["budget"]["used_builder_bytes"]}'.encode(),1).replace(('"graph_digest":'+json.dumps(av['graph_digest'])).encode(),('"graph_digest":'+json.dumps(bv['graph_digest'])).encode(),1)
    assert restored==b
    wires.extend({'path':path,'sha256':sha((R/path).read_bytes())}for path in paths)
table=R/'docs/evidence/browser-next-actual-218a9-20261016/current-to-prior.json'
assert sha(table.read_bytes())=='e472406e433dd61bd26a852637996981a53b64cf4db9feb054185fb6e752c74a'
assert len(json.loads(table.read_bytes()))==5
print(json.dumps({'status':'PASS_SOURCE_ONLY_NOT_RUST_EXECUTION','whole_original_programs':3,'retained_MAIN671_namespace':'byteexact except one historical runner acquisition','original218_table_unchanged':True,'complete_independent_wire_pairs':wires,'actual_capture_or_new_pin_value':None}))
