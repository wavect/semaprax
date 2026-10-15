"""Bounded full-source fee-correction successor, without Rust execution."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='9cbdb5520a0ded38c5aece9f79e99c03e73e93fd';FIX='96c5548088f189c836a7e66081a26269dbf82cec';sha=lambda b:hashlib.sha256(b).hexdigest();rows=json.loads((D/'current-to-9cb.json').read_text())
def get(rev,p):return subprocess.check_output(['git','show',rev+':'+p])
def apply(rs,p,b):
 hits=[x for x in rs if x['source']==p];assert len(hits)<=1
 if not hits:return b
 row=hits[0];assert sha(b)==row['current_sha256'],p
 for e in row['edits']:
  a=e['new'].encode();assert b.count(a)==1,p;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==row['original_sha256'],p
 return b
negative=0
for row in rows:
 p=row['source'];raw=(R/p).read_bytes();old=apply(rows,p,raw);assert old==get(BASE,p)==(R/row['archive']).read_bytes()
 for bad in (raw+b' ',old):
  try:apply(rows,p,bad)
  except AssertionError:negative+=1
  else:raise AssertionError('stale/altered accepted')
assert len(rows)==2
attempt='src/workspace_graph/constructor_failure/attempt.rs';assert (R/attempt).read_bytes()==get(FIX,attempt)
# The new source-helper delta is exactly two actual entry prefixes plus its
# single full-source authenticated helper; old routing/assertions remain whole.
s=(R/'src/source_reflows_merge.rs').read_text();prefix='    let previous = before_setup_reference_controls(source, current);\n    let current = previous.as_str();\n';assert s.count(prefix)==2
marker='\n// Corrected physical receiving fees precede the immutable initial setup views.';prior=s[:s.index(marker)].replace(prefix,'');assert prior==get(BASE,'src/source_reflows_merge.rs').decode()
def replay(script,named,scope):
 old=R/script;source=old.read_text();rb0,rt0,wt0,co0=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output;capture=[]
 def rb(p):return named[p.resolve()] if p.resolve() in named else rb0(p)
 def rt(p,*a,**kw):return named[p.resolve()].decode() if p.resolve() in named else rt0(p,*a,**kw)
 def wt(p,s,*a,**kw):assert p.resolve()==(old.parent/'source-proof.json').resolve();capture.append(json.loads(s));return len(s)
 def co(args,*a,**kw):
  if isinstance(args,list) and args[:3]==['git','diff','--name-only'] and '--' in args:
   i=args.index('--');revs=[x for x in args[3:i] if not x.startswith('-')]
   if len(revs)==1:args=args[:i]+[scope]+args[i:]
  return co0(args,*a,**kw)
 try:
  Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb,rt,wt,co
  with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(source,str(old),'exec'),{'__file__':str(old),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb0,rt0,wt0,co0
 return capture[-1] if capture else json.loads(out.getvalue())
named={(R/r['source']).resolve():r['edits'][0]['old'].encode() for r in rows}
oldproof=replay('docs/evidence/setup825-relative-starter-assembly-20261015/verify.py',named,BASE)
# The fee author's proof receives complete corrected isolated96c bytes. The
# existing full9cb→1935 branch supplies every unrelated source; attempt alone
# is exactly the independently authenticated96c production correction above.
view=json.loads((R/'docs/evidence/setup825-relative-starter-assembly-20261015/view-825.json').read_text());namedfix={}
for r in view:
 p=r['source'];b=(R/p).read_bytes()
 if p!=attempt:b=apply(view,p,apply(rows,p,b))
 assert b==get(FIX,p),p
 namedfix[(R/p).resolve()]=b
corrected=replay('docs/evidence/preowned-setup-publication-825-reference-controls/verify.py',namedfix,FIX)
assert corrected['whole_current_inverse'] and corrected['original_complete_bindings_replayed']
# Existing immutable packets, all assertions and previous diagnostics unchanged.
assert all(x.startswith('A\t') for x in subprocess.check_output(['git','diff','--name-status',BASE,'--','docs/evidence'],text=True).splitlines())
changed=subprocess.check_output(['git','diff','--name-only',BASE,'--','src','tests','crates'],text=True).splitlines();assert sorted(changed)==sorted(r['source'] for r in rows)
selection=(R/'docs/evidence/setup825-relative-starter-assembly-20261015/pending-selection.json').read_bytes();assert selection==get(BASE,'docs/evidence/setup825-relative-starter-assembly-20261015/pending-selection.json');assert json.loads(selection)['total']==152
proof={'status':'SOURCE_ONLY_NO_RUST_GATES','source_basis':BASE,'fee_author_commit':FIX,'complete_successor_rows':len(rows),'stale_and_altered_refusals':negative,'all_old_9cb_source_proofs_replayed':oldproof,'fee_author_complete_source_proof_replayed':corrected,'all_existing_evidence_unchanged':True,'future_selection_unchanged':152,'new_test_bodies':0,'production_correction_exact_author_bytes':True,'runtime_bodies_executed':0,'issue825':'OPEN_PARTIAL'}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({k:v for k,v in proof.items() if k not in ['all_old_9cb_source_proofs_replayed','fee_author_complete_source_proof_replayed']}))
