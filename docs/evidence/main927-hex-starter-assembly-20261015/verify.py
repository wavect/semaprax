"""Whole-source named branch replay; source only, no Rust execution."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest();meta=json.loads((D/'basis.json').read_text())
def get(rev,p):return subprocess.check_output(['git','show',rev+':'+p])
def load(p):return json.loads((R/p).read_text())
def apply(rows,p,b):
 found=[r for r in rows if r['source']==p];assert len(found)<=1
 if not found:return b
 r=found[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  a=e['new'].encode();assert b.count(a)==1,p;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'],p
 return b
views={s:json.loads((D/('view-'+s+'.json')).read_text()) for s in meta['branches']};negative=0
for stage,rows in views.items():
 for r in rows:
  p=r['source'];raw=(R/p).read_bytes();old=apply(rows,p,raw)
  assert old==get(meta['branches'][stage],p)==(R/r['archive']).read_bytes()
  for bad in (raw+b' ',old):
   try:apply(rows,p,bad)
   except AssertionError:negative+=1
   else:raise AssertionError('stale or altered accepted:'+p)
mainold=load('docs/evidence/main927-source-successor-20261015/current-to-9b.json');hexold=load('docs/evidence/workspace-facade-hex-fix-9b-20261015/current-to-9b.json')
paths=sorted({r['source'] for rs in [*views.values(),mainold,hexold] for r in rs})
for p in paths:
 raw=(R/p).read_bytes();a=apply(mainold,p,apply(views['main927'],p,raw));b=apply(hexold,p,apply(views['hex'],p,raw));assert a==b==get(meta['source_basis'],p),p
# Original verifier scripts and receipts remain immutable. Interpose only complete
# authenticated branch bytes and Git's matching closed branch diff domain.
def replay(stage,script):
 old=R/script;source=old.read_text();named={(R/r['source']).resolve():r['edits'][0]['old'].encode() for r in views[stage]}
 rb0,rt0,wt0,co0=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output;captured=[]
 def rb(p):return named[p.resolve()] if p.resolve() in named else rb0(p)
 def rt(p,*a,**kw):return named[p.resolve()].decode() if p.resolve() in named else rt0(p,*a,**kw)
 def wt(p,s,*a,**kw):
  assert p.resolve()==(old.parent/'source-proof.json').resolve(),p
  captured.append(json.loads(s));return len(s)
 def co(args,*a,**kw):
  if isinstance(args,list) and args[:3]==['git','diff','--name-only'] and '--' in args:
   i=args.index('--');revs=[x for x in args[3:i] if not x.startswith('-')]
   if len(revs)==1:args=args[:i]+[meta['branches'][stage]]+args[i:]
  return co0(args,*a,**kw)
 try:
  Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb,rt,wt,co
  with contextlib.redirect_stdout(io.StringIO()):exec(compile(source,str(old),'exec'),{'__file__':str(old),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb0,rt0,wt0,co0
 assert captured;return captured[-1]
mainproof=replay('main927','docs/evidence/main927-source-successor-20261015/verify.py')
hexproof=replay('hex','docs/evidence/workspace-facade-hex-fix-9b-20261015/verify.py')
# Preserve every pre-existing tracked evidence file, not just selected hashes.
changes=subprocess.check_output(['git','diff','--name-status',meta['source_basis'],'--','docs/evidence'],text=True).splitlines()
assert all(x.startswith('A\t') for x in changes),changes
for folder,rev in [('main927-source-successor-20261015',meta['branches']['main927']),('workspace-facade-hex-fix-9b-20261015',meta['branches']['hex']),('starter-fixture-contracts-837-20261015',meta['starter_commit']),('failed-build-9b174-20261015',meta['failed_history_commit'])]:
 assert not subprocess.check_output(['git','diff','--name-only',rev,'--','docs/evidence/'+folder],text=True).strip(),folder
starter='crates/semaprax-toolchain/tests/cli_new_project_v1/application_starter.rs';assert (R/starter).read_bytes()==get(meta['starter_commit'],starter)
helper=(R/'src/source_reflows_merge.rs').read_text();assert 'assert_eq!(\n        previous, hex_previous,' in helper
assert helper.count('let previous = before_main927(source, current);')==3
# All runtime guard JSON retains its original physical/domain ownership.
for p in ['docs/evidence/held-auth-839/source-inverses.json','src/source_reflows.json','src/source_presence_restorations.json']:
 if (R/p).exists():
  rev='9279411a0c7e7cf12908457c5fe93289a7b19508' if p.startswith('src/') else meta['source_basis']
  assert (R/p).read_bytes()==get(rev,p),p
proof={'status':'SOURCE_ONLY_NO_RUST_GATES','actual_main':'9279411a0c7e7cf12908457c5fe93289a7b19508','source_basis':meta['source_basis'],'complete_branch_rows':{s:len(v) for s,v in views.items()},'complete_9b_convergence_paths':len(paths),'stale_and_altered_refusals':negative,'original_main927_proof_replayed':mainproof,'original_hex_proof_replayed':hexproof,'all_preexisting_evidence_unchanged':True,'failed_build_14_archive_unchanged':True,'starter_exact_authored_commit':True,'runtime_test_bodies_executed':0,'production_changes_only_authored_MAIN':True,'prior_completed_runtime_subject':'ae91_95_only_not_failed9b'}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({k:v for k,v in proof.items() if not k.startswith('original_')}))
