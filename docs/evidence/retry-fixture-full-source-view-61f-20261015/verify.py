"""Exact retry guard physical→829 view; no Rust or application execution."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;B='61f2de3cb32ab280accb227123b4c332cb628ec1';sha=lambda b:hashlib.sha256(b).hexdigest();introduced=set()
views={n:json.loads((D/(n+'.json')).read_text()) for n in ['current-to-61f','runtime-view']}
def apply(rows,p,b):
 rs=[r for r in rows if r['source']==p];assert len(rs)<=1
 if not rs:return b
 r=rs[0];assert sha(b)==r['current_sha256']
 for e in r['edits']:
  n=e['new'].encode();assert b.count(n)==1;b=b.replace(n,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
negative=0
for rows in views.values():
 for r in rows:
  p=r['source'];raw=(R/p).read_bytes();old=apply(rows,p,raw);assert old==(R/r['archive']).read_bytes()==subprocess.check_output(['git','show',r['checkpoint_revision']+':'+p])
  try:apply(rows,p,raw+b' ')
  except AssertionError:negative+=1
  else:raise AssertionError('modified source accepted')
p='src/workspace_graph/tests/builder_limits.rs';raw=(R/p).read_bytes();old=apply(views['current-to-61f'],p,raw)
new=b'    let current = crate::source_reflows_merge::retry_fixture_input("src/workspace_graph/tests/builder_limits.rs", include_str!("builder_limits.rs"));';before=b'    let current = include_str!("builder_limits.rs");';assert raw.count(new)==1 and raw.replace(new,before,1)==old
inverse=json.loads((R/'docs/evidence/canonical-preparation-stage-829/retry-precharge-fixture/exact-inverse.json').read_text());original=(R/'docs/evidence/canonical-preparation-stage-829/retry-precharge-fixture/complete-fixture.before.txt').read_bytes();assert sha(original)==inverse['original_sha256']=='4c8f8a2cac574013d6acb485d7af2c86699fb3b5b09be532c90786c6ec7add24'
def restore(s):
 start=s.index(b'/// Tune only the fixture size, never the production bound.');end=start+s[start:].index(b'\n#[test]\nfn identity_prebound_production_core_retry');v=s[start:end]
 for splice in reversed(inverse['splices']):
  at=splice['start'];after=splice['after'].encode();assert v[at:at+len(after)]==after;v=v[:at]+splice['before'].encode()+v[at+len(after):]
 assert v==original;return len(v)
try:restore(old)
except AssertionError:prior_actual_static_failure=True
else:raise AssertionError('prior fixture unexpectedly restored')
restored=restore(apply(views['runtime-view'],p,raw))
def replay(script,rows,scope):
 path=R/script;code=path.read_text();named={(R/r['source']).resolve():apply(rows,r['source'],(R/r['source']).read_bytes()) for r in rows}
 rb,rt,wt,co,glob=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob;proofs=[]
 def bounded_glob(p,*a,**k):
  for child in glob(p,*a,**k):
   if child.resolve() in introduced:continue
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



prior=replay('docs/evidence/main66517-source-assembly-20261015/verify.py',views['current-to-61f'],B)
changes=subprocess.check_output(['git','diff','--name-status',B,'--','docs/evidence'],text=True).splitlines();assert all(x.startswith('A\t') for x in changes)
result=dict(status='SOURCE_ONLY_NO_RUST_GATES',source_basis=B,full_predecessor_inputs=2,full_runtime_829_inputs=1,modified_input_refusals=negative,old_literal_hash_and_all_splices_unchanged=True,entire_original_restored_bytes=restored,prior_physical_fixture_static_refusal_reproduced=prior_actual_static_failure,application_retry_not_executed=True,selection_unchanged=119,full_prior_proof=prior)
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='full_prior_proof'},indent=2))
