"""Bounded complete source-domain composition, no Rust execution or RSS claim."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;B='13d48f0b5d0d6fdc1ce83a717d1729f734eb5392';P='5f60eecf8d9f7febfc515b9f34e2d02a8327b31a';sha=lambda b:hashlib.sha256(b).hexdigest()
def get(rev,p):return subprocess.check_output(['git','show',rev+':'+p])
def apply(rows,p,b):
 matching=[r for r in rows if r['source']==p];assert len(matching)<=1
 if not matching:return b
 r=matching[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  new=e['new'].encode();assert b.count(new)==1,p;b=b.replace(new,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
allviews={n:json.loads((D/(n+'.json')).read_text()) for n in ['current-to-13d','current-to-pilot','pilot-runtime-views','source-inverses']};negative=0
for name,rows in allviews.items():
 for r in rows:
  p=r['source'];raw=(R/p).read_bytes();old=apply(rows,p,raw)
  assert old==get(r['checkpoint_revision'],p)==(R/r['archive']).read_bytes(),p
  for bad in [raw+b' ',b' '+raw]:
   try:apply(rows,p,bad)
   except AssertionError:negative+=1
   else:raise AssertionError('modified input accepted')
# Runtime stages are independent, with all5 complete fixed pilot inputs retained.
pilot=json.loads((R/'docs/evidence/compressed-selected-child-fixture-841/complete-source-views.json').read_text())['views'];assert len(pilot)==5
for v in pilot:assert apply(allviews['pilot-runtime-views'],v['path'],(R/v['path']).read_bytes())==v['current_bytes'].encode()
helper='src/cleanup_plan/replay/selected_context/child/tests/source/compressed_fixture.rs';s=(R/helper).read_text();assert s.count('source_reflows_merge::compressed_fixture_input(path, current)')==1
assert s.index('_ => return current')<s.index('source_reflows_merge::compressed_fixture_input')<s.index('digest(current.as_bytes())')
assert s.count('/docs/evidence/compressed-selected-child-fixture-841/exact-source-inverses.json.gz')==1
h=(R/'src/source_reflows_merge.rs').read_text();assert h.count('let previous = before_fixture841(source, current);')==2
# Run unchanged prior scripts only in their complete authenticated physical view.
def replay(script,rows,scope):
 path=R/script;code=path.read_text();named={(R/r['source']).resolve():apply(rows,r['source'],(R/r['source']).read_bytes()) for r in rows}
 rb,rt,wt,co=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output;proofs=[]
 def bound_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
 def bound_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
 def write(p,s,*a,**k):assert p.name=='source-proof.json';proofs.append(json.loads(s));return len(s)
 def output(args,*a,**k):
  if isinstance(args,list) and args[:3]==['git','diff','--name-only'] and '--' in args:
   i=args.index('--');revs=[x for x in args[3:i] if not x.startswith('-')]
   if len(revs)==1:args=args[:i]+[scope]+args[i:]
  return co(args,*a,**k)
 try:
  Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=bound_bytes,bound_text,write,output
  with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(code,str(path),'exec'),{'__file__':str(path),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb,rt,wt,co
 return proofs[-1] if proofs else json.loads(out.getvalue())
prior=replay('docs/evidence/main5ba-future-source-assembly-20261015/verify.py',allviews['current-to-13d'],B)
compressed=replay('docs/evidence/compressed-selected-child-fixture-841-paired-transport/verify.py',allviews['current-to-pilot'],P)
assert compressed['original_full_assertion_rows']==41
# Every old artifact/runtime literal remains byteexact, both inherited and pilot.
for rev in [B,P]:
 changes=subprocess.check_output(['git','diff','--name-status',rev,'--','docs/evidence'],text=True).splitlines();assert all(x.startswith('A\t') for x in changes),changes
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 assert (R/p).read_text().count(helper+'"')==1
 assert apply(allviews['current-to-13d'],p,(R/p).read_bytes())==get(B,p)
 assert apply(allviews['pilot-runtime-views'],p,(R/p).read_bytes())==get(P,p)
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
for p in ['src/source_reflows_merge.rs',helper]:assert len((R/p).read_text().splitlines())<=1500
result=dict(status='SOURCE_ONLY_NO_RUST_GATES',inherited_checkpoint=B,pilot_checkpoint=P,full_views={k:len(v) for k,v in allviews.items()},altered_inputs_rejected=negative,full_inherited_proof=prior,full_paired_transport_source_proof=compressed,all_old_evidence_and_runtime_JSON_unchanged=True,original_assertion_rows=41,independent_inherited_and_pilot_domains=True,paired_build_or_RSS_measurement=False,issue841='OPEN_PENDING_RUNTIME_AND_MEANINGFUL_PAIRED_MEASUREMENT')
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if not k.startswith('full_')},indent=2))
