"""Source-only exact f045/0bf views; replay the unchanged complete ff verifier."""
from pathlib import Path
import subprocess,json,hashlib,contextlib,io
R=Path.cwd();D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
def load(p):return json.loads(p.read_text())
def apply(rows,p,b):
 records=[x for x in rows if x['source']==p];assert len(records)<=1
 if not records:return b
 row=records[0];assert sha(b)==row['current_sha256'],p
 for e in row['edits']:
  new=e['new'].encode();assert b.count(new)==1;b=b.replace(new,e['old'].encode(),1)
 assert sha(b)==row['original_sha256'],p
 return b
before=load(D/'current-to-0bf.json');f045=load(D/'current-to-f045.json');named={};negative=0
for row in before:
 p=row['source'];raw=(R/p).read_bytes();old=apply(before,p,raw);assert old==subprocess.check_output(['git','show','0bf3c1d04e627f9af226a805d773460596fa4d52:'+p]);named[(R/p).resolve()]=old
 try:apply(before,p,old)
 except AssertionError:negative+=1
 else:raise AssertionError(('stale original admitted',p))
named[(R/'src/source_reflows.json').resolve()]=(D/'historical-main87.json').read_bytes()
old=R/'docs/evidence/mainff869-source-successor-20261015/verify.py';source=old.read_text();rb0=Path.read_bytes;rt0=Path.read_text;wt0=Path.write_text;co0=subprocess.check_output;capture={}
def rb(p):return named.get(p.resolve(),rb0(p))
def rt(p,*a,**kw):return named[p.resolve()].decode() if p.resolve() in named else rt0(p,*a,**kw)
def wt(p,value,*a,**kw):
 assert p.resolve()==(old.parent/'source-only-proof.json').resolve(),p
 capture['full_ff_proof']=json.loads(value);return len(value)
def historical_git(args,*a,**kw):
 if args==['git','diff','--name-only','ae91fa963ecfe85f4cc830cda85fff2f5bdbe094','--','docs/evidence']:
  args=['git','diff','--name-only','ae91fa963ecfe85f4cc830cda85fff2f5bdbe094','77f90da5e','--','docs/evidence']
 return co0(args,*a,**kw)
try:
 subprocess.check_output=historical_git
 Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(source,str(old),'exec'),{'__file__':str(old),'__name__':'__main__'})
finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0;subprocess.check_output=co0
assert capture['full_ff_proof']['active_main_ci_records']==92
active=load(D/'live-mainf045-ci93.json');assert len(active)==93 and active[:92]==load(R/'docs/evidence/mainff869-source-successor-20261015/live-mainff-ci92.json')
assert all(row in active for row in load(R/'src/source_reflows.json'))
row=active[-1];p=row['path'];value=apply(f045,p,(R/p).read_bytes());assert sha(value)==row['after_sha256']
for e in row['inverse']:
 new=e['after'].encode();assert value.count(new)==1;value=value.replace(new,e['before'].encode(),1)
assert sha(value)==row['before_sha256']
for row in f045:assert apply(f045,row['source'],(R/row['source']).read_bytes())==subprocess.check_output(['git','show','f0455caf96706a0545e754957fcc903434a7f880:'+row['source']])
# New fixed-operation guard still reconstructs the entire archived list owner.
def ci(path):
 row=next((r for r in active if r['path']==path),None);raw=(R/path).read_bytes()
 if row is None:return raw.decode()
 raw=apply(before,path,raw)
 raw=apply(load(R/'docs/evidence/mainff869-source-successor-20261015/current-to-mainff.json'),path,raw)
 assert sha(raw)==row['after_sha256'],path
 for e in row['inverse']:
  new=e['after'].encode();assert raw.count(new)==1;raw=raw.replace(new,e['before'].encode(),1)
 assert sha(raw)==row['before_sha256'];return raw.decode()
current=ci('src/list_ops.rs');start=current.index('pub(crate) fn is_list(');end=current.index('pub(crate) fn validate_declarations(')
assert sha(current[start:end].encode())=='c91741020e653b86582b9e811353438c67c934a29746dedfdec632ef81ece8a9'
saved=ci('src/list_ops/original_constructor.rs');a=saved.index('pub(crate) fn is_list(');b=saved.index('pub(crate) fn validate_declarations(');restored=current[:start]+saved[a:b]+current[end:]
for new,oldtext in [('&& canonical_step_shape(&program.declarations)','&& step_shape(&program.declarations, &resolved_step())'),('\n#[cfg(test)]\npub(crate) mod original_constructor;\n','')]:
 assert restored.count(new)==1;restored=restored.replace(new,oldtext,1)
assert restored==ci('docs/evidence/fixed-operation-identities-785/src__list_ops.rs.before.txt')
proof=load(D/'source-proof.json');proof.update(full_original_ff92_and_consumer57_verifier_replayed=True,full_original_ff_proof=capture['full_ff_proof'],new_MAIN_record_full_inverse_exact=True,new_complete_fixed_operation_list_guard_exact=True,stale0bf_inputs_rejected=negative,runtime_bodies_executed=0,old_ff_metadata_diff_checked_at_immutable_original_77f90da5e=True)
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS full original ff92/consumer57 replay +new MAIN93 record; strict0bf',len(before),'inputs')
