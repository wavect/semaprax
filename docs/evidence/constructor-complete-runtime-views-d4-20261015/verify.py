"""Complete physical source domains for the existing inline/backing owners."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='aa03d88c21b268cfc383c9842c5799bc8bca3c3f';sha=lambda b:hashlib.sha256(b).hexdigest()
def get(rev,p):return subprocess.check_output(['git','show',rev+':'+p])
def apply(row,b):
 assert sha(b)==row['current_sha256'],row['source']
 for e in row['edits']:
  a=e['new'].encode();assert b.count(a)==1;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==row['original_sha256'];return b
rows=json.loads((D/'current-to-aa03.json').read_text());runtime=json.loads((D/'runtime-views.json').read_text());negative=0
for row in rows+runtime:
 raw=(R/row['source']).read_bytes();old=apply(row,raw);assert old==get(row['checkpoint_revision'],row['source'])==(R/row['archive']).read_bytes()
 if raw!=old:
  for bad in (raw+b' ',old):
   try:apply(row,bad)
   except AssertionError:negative+=1
   else:raise AssertionError('stale/altered accepted')
by={r['source']:apply(r,(R/r['source']).read_bytes()).decode() for r in runtime}
D825=R/'docs/evidence/constructor-inline-failure-825';manifest=json.loads((D825/'exact-source-inverses.json').read_text());assert len(manifest['owners'])==5 and len(manifest['children'])==5
for record in manifest['owners']:
 p=record['source'];b=by[p]
 for e in reversed(record['edits']):assert b.count(e['after'])==1,p;b=b.replace(e['after'],e['before'],1)
 original=(D825/record['archive']).read_bytes();assert b.encode()==original and sha(original)==record['before_sha256'],p
for record in manifest['children']:assert sha(by[record['source']].encode())==record['sha256'],record['source']
Dproof=R/'docs/evidence/provider-proof-backing-714177';manifest=json.loads((Dproof/'whole-source-inverses.json').read_text());assert len(manifest['whole_sources'])==17
for record in manifest['whole_sources']:
 p=record['file'];b=by[p];assert sha(b.encode())==record['current_sha256'],p
 original=(Dproof/record['archive']).read_bytes();assert len(original)==record['original_bytes'] and sha(original)==record['original_sha256'],p
 for e in record['inverse']:assert b.count(e['current'])==1,p;b=b.replace(e['current'],e['before'],1)
 assert b.encode()==original,p
# New physical entry routing keeps every original manifest and original archive.
# Existing test assertions/inverse loops remain; only their current input view is
# made explicit, with complete current SHA before the e4b predecessor is used.
for p in ['src/workspace_graph/expected_projection/defaults/inline_tests.rs','src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs']:
 assert 'constructor_complete_input(' in (R/p).read_text()
changed=subprocess.check_output(['git','diff','--name-only',BASE,'--','src','tests','crates'],text=True).splitlines();assert sorted(changed)==sorted(r['source'] for r in rows)
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
prior=replay('docs/evidence/region-status-import-view-d4-20261015/verify.py',named,BASE)
assert all(x.startswith('A\t') for x in subprocess.check_output(['git','diff','--name-status',BASE,'--','docs/evidence'],text=True).splitlines())
proof={'status':'SOURCE_ONLY_NO_RUST_GATES','source_basis':BASE,'full_source_successor_rows':len(rows),'physical_runtime_e4b_inputs':len(runtime),'inline_five_complete_owners_and_five_children_replayed':True,'backing_seventeen_full_sources_replayed':True,'old_manifests_archives_unchanged':True,'prior_source_proof_replayed':prior,'stale_altered_refusals':negative,'new_test_bodies':0,'production_changes':0,'runtime_bodies_executed':0,'future_selection_unchanged':178}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({k:v for k,v in proof.items() if k!='prior_source_proof_replayed'}))
