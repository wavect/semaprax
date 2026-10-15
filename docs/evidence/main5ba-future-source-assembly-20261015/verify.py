"""Full current, previous-private, parent and MAIN source domains; no Rust."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess,re
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='1c7697809';PARENT='85142274c2f3e73bc1a4e5150672150c4d17d95f';MAIN='5ba0c4daf1c4eacfc2004de2da676b71493f8e28';sha=lambda b:hashlib.sha256(b).hexdigest()
def get(rev,p):return subprocess.check_output(['git','show',rev+':'+p])
def apply(rows,p,b):
 rs=[r for r in rows if r['source']==p];assert len(rs)<=1
 if not rs:return b
 r=rs[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  a=e['new'].encode();assert b.count(a)==1,p;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
views={s:json.loads((D/('current-to-'+s+'.json')).read_text()) for s in ['1c','parent851','main5ba']};fixture=json.loads((D/'fixture-view.json').read_text());negative=0
for stage,rows in [*views.items(),('fixture',fixture)]:
 for row in rows:
  p=row['source'];raw=(R/p).read_bytes();old=apply(rows,p,raw);assert old==get(row['checkpoint_revision'],p)==(R/row['archive']).read_bytes()
  for bad in (raw+b' ',old):
   try:apply(rows,p,bad)
   except AssertionError:negative+=1
   else:raise AssertionError('altered/stale accepted')
assert len(views['main5ba'])==2
# MAIN's exact whole-root proof, including diagnostic-site inverse, stays real.
reference=get(MAIN,'src/workspace_graph/identity_resolution/base/cache_billing/source_reference.rs').decode()
def rawconst(name):return re.search(r'const '+name+r': &str = r###"(.*?)"###;',reference,re.S).group(1)
root=apply(views['main5ba'],'src/workspace_graph.rs',(R/'src/workspace_graph.rs').read_bytes()).decode();assert sha(root.encode())=='811b1dae18de462dedf10c6b683ef6f3ea1c835cc0d92964baa39476c97936e4'
for i in range(2):
 a=rawconst('SITE_AFTER_'+str(i));assert root.count(a)==1;root=root.replace(a,rawconst('SITE_BEFORE_'+str(i)),1)
assert sha(root.encode())=='3fcd39c7b442be79e1200493893a14f7ad9a00c242e54b40b090c102e3e44561'
for i in range(3):
 a=rawconst('AFTER_'+str(i));assert root.count(a)==1;root=root.replace(a,rawconst('BEFORE_'+str(i)),1)
assert sha(root.encode())=='de94863e39460dd9270b2feea2fc7a4986d25417e20ee2cfa4b38412e9a13f55'
a=rawconst('AFTER_3');assert root.count(a)==1;root=root.replace(a,rawconst('BEFORE_3'),1);assert sha(root.encode())=='c82d3bb6176c847fd9ff171f02d9c5aedd113e72b1eb9b1739ffee062319c11b'
a=rawconst('CURRENT_CALL');assert root.count(a)==1;root=root.replace(a,rawconst('ORIGINAL_CALL'),1);assert sha(root.encode())=='fe644b87c264e23b9afa0660d4bcd8149583f19438146734d258025c27087771'
p='tests/workspace/semantic_graph/source_inventory.rs';inventory=apply(views['main5ba'],p,(R/p).read_bytes()).decode();assert sha(inventory.encode())=='e55bd484d5c38c005501cbb3b48493907cd7fc370ab083b810cc71a4b3b0ce0b'
for i in range(3):
 a=rawconst('ADDITIONS_'+str(i));assert inventory.count(a)==1;inventory=inventory.replace(a,'',1)
assert sha(inventory.encode())=='d4158fd988538e77c0f39a2f05cfc50aab15679744bba14ae1f005fc32c805fb'
# Only the two previously-authored source-wrapper names differ from parent's
# reviewed complete reference; MAIN source assertions and all literals unchanged.
p='src/workspace_graph/identity_resolution/base/cache_billing/source_reference.rs';source=(R/p).read_text();assert source.count('source_reflows_merge::main_current_input')==2;assert source.replace('source_reflows_merge::main_current_input','source_reflows_merge::main927_input')==get(PARENT,p).decode()
# MAIN's current initialization repair and actual parent merge source retained.
for p in ['src/bounded_output.rs','src/bounded_output/source_owner_scope.rs','src/bounded_output/refusal_tests.rs','src/workspace_graph.rs','src/workspace_graph/diagnostics.rs','src/workspace_graph/diagnostics/refusal_tests.rs']:
 assert (R/p).read_bytes()==get(PARENT,p),p
assert 'first_site: Cell::new(None)' in (R/'src/bounded_output/source_owner_scope.rs').read_text()
# Physical source-producer v12 expectation plus independent v11 rejection remain
# exact focused assertions inside existing selected test bodies, not new gates.
p='tests/project/semantic_cache.rs';s=(R/p).read_text();assert s.replace('semaprax::project::PROJECT_SEMANTIC_CACHE_COMPATIBILITY','"semaprax.project-checked-module-hir.v11"')==get(BASE,p).decode()
s=(R/'src/project/incremental/snapshot/field_update/tests.rs').read_text()
for text in ['semaprax.project-checked-module-hir.v12','semaprax.project-checked-module-hir.v11','super::super::manifest(&context).unwrap()','super::super::manifest(&legacy).unwrap_err()','SPX-G255','semantic snapshot compiler context is incompatible']:assert re.sub(r"\s+","",text) in re.sub(r"\s+","",s)
# The ordinary fresh path is the complete pre-e484 receiver, byteexact apart
# from the additive module declaration. Its original strong affinity gates stay.
ordinary=json.loads((D/'ordinary-receiver-binding.json').read_text())
for binding in ordinary:
 p=binding['source'];old=(R/binding['archive']).read_bytes();assert old==get(binding['revision'],p) and sha(old)==binding['sha256']
 current=(R/p).read_bytes()
 if p.endswith('/base.rs'):
  registration=b'\npub(in crate::workspace_graph) mod cache_billing;\n';assert current.count(registration)==1;assert current.replace(registration,b'',1)==old
 else:assert current==old
s=(R/'src/workspace_graph/identity_resolution/base/cache_billing/tests.rs').read_text()
for text in ['for built in [false, true]','let ordinary = observe(false);','let cached = observe(true);','assert_eq!(ordinary, cached);','ModuleFailure::Report(report)','ModuleFailure::Setup(Refused::Budget(restored))','assert_eq!(restored, expected_parent);','assert_eq!(first, Some(outcome.1));','assert_eq!(physical.allocated, 0);','assert_eq!(acquisitions(), acquired);','ProducerFailure::Detached','detachment cannot retry acquisition','report[0].severity','report[0].path','report[0].span','report[0].help']:assert text in s,text
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

named={(R/r['source']).resolve():r['edits'][0]['old'].encode() for r in views['1c']}
prior=replay('docs/evidence/constructor-complete-runtime-views-d4-20261015/verify.py',named,BASE)
named={(R/r['source']).resolve():r['edits'][0]['old'].encode() for r in fixture}
fixtureproof=replay('docs/evidence/starter-shiftsim-canonical-fixture-20261015/verify.py',named,'f6b4c2d64ddffc7fd1b42e18ff2b7a4c88aeb7b2')
assert fixtureproof['full_raw_fixture_files']==13
for rev in [BASE,PARENT]:
 changes=subprocess.check_output(['git','diff','--name-status',rev,'--','docs/evidence'],text=True).splitlines();assert all(x.startswith('A\t') for x in changes),changes
for p in ['docs/evidence/held-auth-839/source-inverses.json','src/source_reflows.json','src/source_presence_restorations.json']:
 if (R/p).exists():assert (R/p).read_bytes()==get(PARENT,p)
proof={'status':'SOURCE_ONLY_NO_RUST_GATES','actual_parent':PARENT,'actual_MAIN':MAIN,'previous_private_full_source_proof':prior,'fixture_author_proof':fixtureproof,'complete_views':{s:len(v) for s,v in views.items()},'fresh_MAIN_full_root_and_inventory_inverses':True,'v12_positive_v11_negative_authored_unexecuted':True,'all_existing_evidence_unchanged':True,'new_test_bodies_by_assembly':0,'stale_altered_refusals':negative,'source_subject_results_not_currentPASS':True,'owner130_matched_context_contract_authored':'ordinary vs cached fullreport parity, sticky restored SetupBudget, zeroallocation and retainedDetached; no production change'}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({k:v for k,v in proof.items() if k not in ['previous_private_full_source_proof','fixture_author_proof']}))
