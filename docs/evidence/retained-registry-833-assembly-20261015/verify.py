"""Independent833/inherited whole-source composition only; no Rust execution."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;B='3817411671f83c4437432dae72b44b83369ac349';C='c3ecc303705334619b48aa64b79302f706afc44e';sha=lambda b:hashlib.sha256(b).hexdigest()
def apply(rows,p,b):
 matching=[r for r in rows if r['source']==p];assert len(matching)<=1
 if not matching:return b
 r=matching[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  new=e['new'].encode();assert b.count(new)==1,p;b=b.replace(new,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
views={n:json.loads((D/(n+'.json')).read_text()) for n in ['current-to-381','current-to-c3','checkpoint-c3']};negative=0
for rows in views.values():
 for r in rows:
  p=r['source'];raw=(R/p).read_bytes();old=apply(rows,p,raw)
  assert old==(R/r['archive']).read_bytes()==subprocess.check_output(['git','show',r['checkpoint_revision']+':'+p])
  for bad in [raw+b' ',b' '+raw]:
   try:apply(rows,p,bad)
   except AssertionError:negative+=1
   else:raise AssertionError('altered input accepted')
def replay(script,rows,scope):
 path=R/script;code=path.read_text();named={(R/r['source']).resolve():apply(rows,r['source'],(R/r['source']).read_bytes()) for r in rows}
 rb,rt,wt,co,glob=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob;proofs=[]
 def bounded_glob(p,*a,**k):
  for child in glob(p,*a,**k):
   if scope==C and child.resolve() in {(R/x).resolve() for x in json.loads((D/'absent-at-c3.json').read_text())}:continue
   if scope==B and child.as_posix().endswith(tuple("/private_layout/registry/"+n for n in ["storage.rs","before_retained.rs","retained_tests.rs"])):continue
   yield child
 def bound_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
 def bound_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
 def write(p,s,*a,**k):assert p.name=='source-proof.json';proofs.append(json.loads(s));return len(s)
 def output(args,*a,**k):
  if isinstance(args,list) and args[:3]==['git','diff','--name-only'] and '--' in args:
   i=args.index('--');revs=[x for x in args[3:i] if not x.startswith('-')]
   if len(revs)==1:args=args[:i]+[scope]+args[i:]
  return co(args,*a,**k)
 try:
  Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob=bound_bytes,bound_text,write,output,bounded_glob
  with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(code,str(path),'exec'),{'__file__':str(path),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob=rb,rt,wt,co,glob
 return proofs[-1] if proofs else json.loads(out.getvalue())

prior=replay('docs/evidence/compressed-fixture-841-assembly-20261015/verify.py',views['current-to-381'],B)
registry=replay('docs/evidence/retained-registry-storage-833/verify.py',views['checkpoint-c3'],C)
assert prior['original_assertion_rows']==41 and registry['whole_original_inputs']==11
for rev in [B,C]:
 changes=subprocess.check_output(['git','diff','--name-status',rev,'--','docs/evidence'],text=True).splitlines();assert all(x.startswith('A\t') for x in changes),changes
# New production bodies and all3 new owning modules are exact reviewed checkpoint.
paths=[r['source'] for r in views['current-to-c3'] if '/private_layout' in r['source'] or r['source']=='src/bounded_output/classifier_string.rs']
paths += ['src/workspace_graph/expected_projection/private_layout/registry/'+n for n in ['before_retained.rs','storage.rs','retained_tests.rs']]
for p in paths:assert (R/p).read_bytes()==subprocess.check_output(['git','show',C+':'+p]),p
helper=(R/'src/source_reflows_merge.rs').read_text();assert helper.count('let previous = before_retained833(source, current);')==2
assert 'fn retained833_previous(' not in helper,'late serial0e projection would mix domains'
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/p).read_text()
 for n in ['storage.rs','before_retained.rs','retained_tests.rs']:
  assert text.count('private_layout/registry/'+n+'"')==1
 assert len(text.splitlines())<=1500
result=dict(status='SOURCE_ONLY_NO_RUST_GATES',inherited_checkpoint=B,retained_checkpoint=C,full_views={n:len(v) for n,v in views.items()},modified_inputs_rejected=negative,production_and_owning_modules_exact_reviewed_c3=True,all_old_evidence_runtime_JSON_unchanged=True,full_inherited_source_proof=prior,full_registry_source_proof=registry,new_behavior_tests=6,new_source_tests=1,compiler_or_runtime_execution=False,measured_savings=None)
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if not k.startswith('full_')},indent=2))
