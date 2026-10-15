"""Finite source-only successor proof in explicit physical/ae91/ff views."""
from pathlib import Path
import subprocess,json,hashlib,contextlib,io
R=Path.cwd();D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest()
def load(p):return json.loads((R/p).read_text())
def apply(rows,p,b):
 rs=[r for r in rows if r['source']==p];assert len(rs)<=1
 if not rs:return b
 r=rs[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  a=e['new'].encode();assert b.count(a)==1,p;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'],p
 return b
prior=load(D.relative_to(R)/'current-to-ae91.json');main=load(D.relative_to(R)/'current-to-mainff.json');meta=load(D.relative_to(R)/'successor.json');ae=meta['frozen_prior_domain'];ff=meta['actual_main_baseline'];named={};negative=0
for r in prior:
 p=r['source'];raw=(R/p).read_bytes();before=apply(prior,p,raw);assert before==subprocess.check_output(['git','show',ae+':'+p]);named[(R/p).resolve()]=before
 try:apply(prior,p,before)
 except AssertionError:negative+=1
 else:raise AssertionError(('stale raw admitted',p))
# Replay the byte-exact previous consumer proof against authenticated ae91 inputs.
# Its original script, runtime manifests and outputs are never rewritten.
old=R/'docs/evidence/source-consumer-closure-fixes-20261015/verify.py';oldsource=old.read_text();original_read_bytes=Path.read_bytes;original_read_text=Path.read_text;original_write_text=Path.write_text;replay={}
def rb(p):return named.get(p.resolve(),original_read_bytes(p))
def rt(p,*a,**kw):return named[p.resolve()].decode() if p.resolve() in named else original_read_text(p,*a,**kw)
def wt(p,data,*a,**kw):
 assert p.resolve()==(old.parent/'source-only-proof.json').resolve(),p
 replay['ae91_consumer_proof']=json.loads(data);return len(data)
try:
 Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt
 old_ns={'__file__':str(old),'__name__':'__main__'}
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(oldsource,str(old),'exec'),old_ns)
finally:Path.read_bytes=original_read_bytes;Path.read_text=original_read_text;Path.write_text=original_write_text
assert replay['ae91_consumer_proof']['historical_consumer_rows_replayed']==57
records=load(D.relative_to(R)/'live-mainff-ci92.json');old87=load('src/source_reflows.json');assert len(records)==92 and records[:87]==old87
for r in records:
 p=r['path'];raw=(R/p).read_bytes()
 # .main also executes the historical branch/CI authentication on all 92 inputs.
 route=old_ns['route'];historical=apply(prior,p,raw).decode();historical=route(old_ns['corrective'],p,route(old_ns['consumer'],p,historical));branch=route(load(old_ns['M']+'current-to-9365.json'),p,historical)
 branches=[route(old_ns['hooks'][v],p,route(old_ns['views'][v],p,branch)) for v in old_ns['views']];assert branches[0]==branches[1]==branches[2]
 route(load('docs/evidence/source-composition-f813-held-20261015/main-projections.json'),p,branches[0]);route(load(old_ns['M']+'current-to-main7f.json'),p,historical)
 b=apply(main,p,raw);assert b==subprocess.check_output(['git','show',ff+':'+p]);assert sha(b)==r['after_sha256']
 for e in r['inverse']:
  a=e['after'].encode();assert b.count(a)==1;b=b.replace(a,e['before'].encode(),1)
 assert sha(b)==r['before_sha256'],p
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/p).read_text()
 for owner in ['atomic_controls.rs','atomic_controls/tests.rs']:
  assert text.count('src/cleanup_plan/replay/path_join/storage/'+owner+'"')==1,(p,owner)
 assert len(text.splitlines())<=1500
# Authenticate every merged input and every pre-assembly owner without changing production.
inputs=load(D.relative_to(R)/'complete-merge-inputs.json')
for r in inputs:
 x=subprocess.run(['git','show',r['revision']+':'+r['source']],capture_output=True)
 assert (x.returncode==0)==r['exists']
 if r['exists']:assert x.stdout.decode()==r['complete_source'] and sha(x.stdout)==r['sha256']
assembly=load(D.relative_to(R)/'assembly-inverses.json')
for r in assembly:assert apply(assembly,r['source'],(R/r['source']).read_bytes())==subprocess.check_output(['git','show',meta['isolated_merge']+':'+r['source']])
# Every previously tracked evidence packet, history and original runtime JSON stays exact.
changed=subprocess.check_output(['git','diff','--name-only',ae,'--','docs/evidence'],text=True).splitlines();assert all(p.startswith(str(D.relative_to(R))+'/') for p in changed)
assert (R/'src/source_reflows.json').read_bytes()==subprocess.check_output(['git','show',ae+':src/source_reflows.json'])
proof={'status':'SOURCE_ONLY_NOT_RUNTIME_QUALIFIED','frozen_prior':ae,'actual_main':ff,'strict_prior_inputs':len(prior),'strict_stale_rejections':negative,'full_merge_snapshot_rows':len(inputs),'active_main_ci_records':len(records),'old87_records_retained_exact':True,'new_doc_inverse_owners':meta['ci_changed_main_inputs'],'all_three_audits_join_both_new_atomic_owners':True,'all_older_evidence_packets_unchanged':True,'assembly_owners':len(assembly),'previous_source_only_proof_replayed_in_authenticated_ae91_view':replay['ae91_consumer_proof'],'runtime_bodies_executed':0}
(D/'source-only-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS strictae91',len(prior),'fullinputs; CI',len(records),'records; old57consumer/87CI replay; atomic2×3includes')
