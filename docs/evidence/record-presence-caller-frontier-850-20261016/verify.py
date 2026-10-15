from pathlib import Path
import json,subprocess,hashlib
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest();rows=json.loads((D/'source-inverses.json').read_text());prior=R/'docs/evidence/record-presence-controls-850-20261016';oldrows=json.loads((prior/'source-inverses.json').read_text());runtime=json.loads((D/'runtime-inputs.json').read_text())
for p,row in rows.items():
 assert (R/p).read_text()==row['current'] and sha(row['current'].encode())==row['current_sha256']
 assert subprocess.check_output(['git','show','bdff54dfa:'+p],cwd=R).decode()==row['before'] and sha(row['before'].encode())==row['before_sha256']
 assert (D/'before'/p).read_text()==row['before']
for p,row in oldrows.items():
 virtual=rows[p]['before']if p in rows else (R/p).read_text();assert virtual==row['current'] and sha(virtual.encode())==row['current_sha256']
for p,row in runtime.items():assert row==rows[p] and not p.endswith('/tests.rs')
assert len(runtime)==3 and len(rows)==4
# The former independently passed namespace and complete MAIN inputs remain exact.
for p in ['runtime-inputs.json','source-proof.json','verify.py']:
 assert (prior/p).read_bytes()==subprocess.check_output(['git','show','bdff54dfa:docs/evidence/record-presence-controls-850-20261016/'+p],cwd=R)
caller=(R/'src/loop_calls/byte_writer/module_index/ordinary.rs').read_text();assert caller.index('.reserve(RecordPresence::shared_caller_controls())')<caller.index('let directory = Directory::collect_current_with_presence(')<caller.index('&record_presence,')
mod=(R/'src/cleanup_plan/renewal_profile/ordinary/mod.rs').read_text();a=mod.index('pub(crate) fn collect_current_with_presence');b=mod.index('fn collect_impl',a);assert 'admission.reserve('not in mod[a:b]
proof={'mode':'SOURCE_ONLY_NO_RUST_EXECUTION','whole_immediate_inverses':4,'runtime_inputs':3,'complete_prior_namespace_restored':True,'old_runtime_proof_script_byte_exact':True,'some_payment_before_caller_borrow':True,'no_new_selector':True};(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
