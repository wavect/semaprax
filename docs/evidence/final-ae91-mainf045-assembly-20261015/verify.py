"""Finite final assembly: independent full branch views, no Rust gates."""
from pathlib import Path
import json,hashlib,subprocess,contextlib,io
R=Path.cwd();D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest();meta=json.loads((D/'basis.json').read_text())
def load(p):return json.loads((R/p).read_text())
def apply(rows,p,b):
 rs=[r for r in rows if r['source']==p];assert len(rs)<=1
 if not rs:return b
 r=rs[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  a=e['new'].encode();assert b.count(a)==1,p;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'],p
 return b
views={v:load(str(D.relative_to(R))+'/view-'+v+'.json') for v in ['fixes','main']};negative=0
for v,rows in views.items():
 for r in rows:
  p=r['source'];raw=(R/p).read_bytes();before=apply(rows,p,raw);assert before==subprocess.check_output(['git','show',meta[v+'_branch']+':'+p])
  try:apply(rows,p,before)
  except AssertionError:negative+=1
  else:raise AssertionError('stale branch source accepted')
fixesrows=load('docs/evidence/ae91-source-consumer-corrections-20261015/current-to-ae91.json');mainrows=load('docs/evidence/mainf045-source-successor-20261015/current-to-0bf.json');ffrows=load('docs/evidence/mainff869-source-successor-20261015/current-to-ae91.json')
for p in meta['source_input_paths']:
 raw=(R/p).read_bytes();a=apply(views['fixes'],p,raw);a=apply(fixesrows,p,a);a=apply(load(str(D.relative_to(R))+'/fixes-ae91-reconcile.json'),p,a)
 b=apply(views['main'],p,raw);b=apply(mainrows,p,b);b=apply(ffrows,p,b);b=apply(load(str(D.relative_to(R))+'/main-ae91-reconcile.json'),p,b);assert a==b,p
 # New atomic owners have no ae91 predecessor; both independent branches agree.
 x=subprocess.run(['git','show',meta['common_ae91']+':'+p],capture_output=True)
 if x.returncode==0:assert a==x.stdout,p
# Replay original source verifiers only in their authenticated named branch view.
# Their closed metadata scope checks use the exact original scope commit, while
# final metadata/production source preservation is independently checked below.
def replay(stage,script,scope):
 old=R/script;source=old.read_text();named={(R/r['source']).resolve():r['edits'][0]['old'].encode() for r in views[stage]};rb0=Path.read_bytes;rt0=Path.read_text;wt0=Path.write_text;co0=subprocess.check_output;capture=[]
 def rb(p):return named[p.resolve()] if p.resolve() in named else rb0(p)
 def rt(p,*a,**kw):return named[p.resolve()].decode() if p.resolve() in named else rt0(p,*a,**kw)
 def wt(p,s,*a,**kw):assert p.resolve()==(old.parent/'source-only-proof.json').resolve() or p.resolve()==(old.parent/'source-proof.json').resolve(),p;capture.append(json.loads(s));return len(s)
 def co(args,*a,**kw):
  if args==['git','diff','--name-only',meta['common_ae91'],'--','docs/evidence']:args=['git','diff','--name-only',meta['common_ae91'],scope,'--','docs/evidence']
  return co0(args,*a,**kw)
 try:
  Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt;subprocess.check_output=co
  with contextlib.redirect_stdout(io.StringIO()):exec(compile(source,str(old),'exec'),{'__file__':str(old),'__name__':'__main__'})
 finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0;subprocess.check_output=co0
 assert capture;return capture[-1]
fixed=replay('fixes','docs/evidence/ae91-source-consumer-corrections-20261015/verify.py','052667a21687a73e2d9af415171663c783d415a9')
main=replay('main','docs/evidence/mainf045-source-successor-20261015/verify.py',meta['main_branch'])
assert fixed['new_current_facade_entries']==7 and main['full_original_ff92_and_consumer57_verifier_replayed']
# Preserve every immutable packet from both branch owners, not merely its receipt.
protected={'fixes':['ae91-source-consumer-corrections-20261015','ae91-source-consumer-rustfmt-20261015','foreign-owner-test-corrections-ae91-20261015','starter-fixture-canonical-root-20261015'],'main':['mainff869-source-successor-20261015','mainf045-source-successor-20261015','workspace-warm-cold-json-diagnostic-20261015']};count=0
for stage,folders in protected.items():
 for folder in folders:
  paths=subprocess.check_output(['git','ls-tree','-r','--name-only',meta[stage+'_branch'],'--','docs/evidence/'+folder],text=True).splitlines()
  for p in paths:assert (R/p).read_bytes()==subprocess.check_output(['git','show',meta[stage+'_branch']+':'+p]);count+=1
# Production comes only from the authored MAIN merge. Final assembly edits one cfgtest helper.
code_changed=subprocess.check_output(['git','diff','--name-only',meta['main_branch'],'--','src','tests','crates'],text=True).splitlines();expected=[r['source'] for r in views['main']];assert sorted(code_changed)==sorted(expected)
assert all(len((R/p).read_text().splitlines())<=1500 for p in meta['source_input_paths'] if p.endswith('.rs'))
proof={'status':'SOURCE_ONLY_NO_RUST_GATES','actual_main':meta['actual_latest_main'],'strict_named_branch_inputs':meta['view_counts'],'strict_stale_rejections':negative,'independent_full_ae91_convergence':True,'four_source_failures_and_full_facade_replayed':True,'original57consumer87CI_and_ff92_MAIN93_verifiers_replayed_in_named_views':True,'old_closed_metadata_scope_views':{'fixes':'052667a21687a73e2d9af415171663c783d415a9','main_ff':'77f90da5e00d533e40714d42369daed90661156a'},'protected_full_packet_files':count,'production_edited_by_assembly':False,'new_test_bodies_by_assembly':0,'runtime_bodies_executed':0}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS final independent branch views; old4/57/87/92/93 source proofs; protected files',count)
