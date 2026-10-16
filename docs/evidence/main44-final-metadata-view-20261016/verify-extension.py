#!/opt/homebrew/bin/python3
"""Prove only the finite Python source extension; never execute Rust."""
from pathlib import Path
import hashlib,json,runpy,subprocess
E=Path(__file__).resolve().parent;R=E.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(commit,path):return subprocess.check_output(['git','show',commit+':'+path],cwd=R)
for p,h in json.loads((E/'finite-bindings.json').read_text()).items():assert sha((E/p).read_bytes())==h
m=json.loads((E/'script-extensions.json').read_text());HEAD=m['head']
for row in m['changes']:
 before=(E/row['archive']).read_bytes();assert sha(before)==row['before_sha256'] and before==git(HEAD,row['path'])
 after=(R/row['path']).read_bytes();assert sha(after)==row['after_sha256'];text=after.decode()
 for edit in reversed(row['replacements']):assert text.count(edit['after'])==1;text=text.replace(edit['after'],edit['before'],1)
 assert text.encode()==before,('complete prior verifier lost',row['path'])
for row in json.loads((E/'output-bindings.json').read_text()):
 before=(E/row['archive']).read_bytes();assert sha(before)==row['before_sha256'] and before==git(HEAD,row['path']);assert sha((R/row['path']).read_bytes())==row['current_sha256']
compose=runpy.run_path(str(E/'finite.py'))['compose'];inputs=json.loads((E/'extensions.json').read_text());assert inputs['head']==HEAD
negatives=0
for row in inputs['rows']:
 raw=(R/row['path']).read_bytes();assert raw==git(HEAD,row['path']);assert sha(raw)==row['current_sha256'];target=(E/row['target_archive']).read_bytes();assert sha(target)==row['target_sha256']
 assert compose(row['domain'],row['path'],raw)==target
 assert compose(row['domain'],row['path'],target)==target
 for bad in [raw[:-1],raw+b' ',raw+raw,(b'!'if raw[:1]!=b'!'else b'?')+raw[1:]]:
  try:compose(row['domain'],row['path'],bad)
  except AssertionError:negatives+=1
  else:raise AssertionError(('whole endpoint drift accepted',row['path']))
# Each old capsule and every original accepted source choice stays completely unchanged.
for packet in ['ordinary-schema-summary-20261016','opt859-record-shape-recovery-20261016']:
 p='docs/evidence/'+packet+'/source-capsule.json';assert (R/p).read_bytes()==git(HEAD,p)
p='docs/evidence/next831858852-source-composition-20261016/composition.json';assert(R/p).read_bytes()==git(HEAD,p)
private='96a49042bf7b204635f8ddef360254948fb63b49';packet='docs/evidence/skeleton-constructor-funding-856-20261016';promoted=json.loads((R/packet/'promoted-modules.json').read_text());changes=[]
for path in promoted:
 raw=(R/path).read_bytes();old=git(private,path)
 assert compose('856-promoted',path,raw)==old
 if raw!=old:changes.append(path)
assert changes==['src/literal_format/funded.rs','src/loop_calls/byte_writer/resolved/funded.rs']
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_bytes().splitlines())==1500
proof=dict(status='PASS_SOURCE_ONLY_EB2_FINITE_PREDECESSORS',head=HEAD,exact_input_views=len(inputs['rows']),new_whole_endpoint_negatives=negatives,complete_prior_scripts=4,original_capsules_unchanged=True,actual856_promoted_modules=len(promoted),private856_byte_exact_modules=11,exact_import_only_promoted_views=changes,Rust_execution=False)
print(json.dumps(proof,sort_keys=True))
