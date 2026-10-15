"""Complete authenticated successors only; no Rust gates or application run."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest();meta=json.loads((D/'basis.json').read_text())
def get(rev,p):return subprocess.check_output(['git','show',rev+':'+p])
def apply(rows,p,b):
 rs=[r for r in rows if r['source']==p];assert len(rs)<=1
 if not rs:return b
 r=rs[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  a=e['new'].encode();assert b.count(a)==1,p;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'],p
 return b
views={s:json.loads((D/('view-'+s+'.json')).read_text()) for s in ['d4','825','typed825']};negative=0
for stage,rows in views.items():
 for row in rows:
  p=row['source'];raw=(R/p).read_bytes();old=apply(rows,p,raw);assert old==(R/row['archive']).read_bytes()
  if stage!='typed825':assert old==get(meta['branches'][stage],p)
  for bad in (raw+b' ',old):
   try:apply(rows,p,bad)
   except AssertionError:negative+=1
   else:raise AssertionError('stale/altered accepted:'+p)
# Replay all21 original typed owner assertions at the same immutable domain.
packet=(R/meta['typed_original_manifest']).read_bytes();assert sha(packet)==meta['typed_original_manifest_sha256']
records=json.loads(packet);assert len(records)==21
for row in records:
 p=row['source'];b=apply(views['typed825'],p,(R/p).read_bytes());assert sha(b)==row['current_sha256'],p
 for edit in row['edits']:
  a=edit['new'].encode();assert b.count(a)==1,p;b=b.replace(a,edit['before'].encode(),1)
 assert sha(b)==row['original_sha256'] and b==(R/row['archive']).read_bytes(),p
# Only a preprojection is added to the prior typed owning test: all assertions,
# loop cases and post-loop live-caller predicates remain complete and exact.
p='src/workspace_graph/constructor_failure/receiving/typed/tests.rs';current=(R/p).read_text();prefix='        let current = crate::source_reflows_merge::setup_typed_input(path, current);\n        let current = current.as_str();\n';assert current.count(prefix)==1;assert current.replace(prefix,'',1)==get(meta['source_basis'],p).decode()
# Three full physical audit unions include every new implementation/test owner.
newowners=['src/workspace_graph/constructor_failure/'+x for x in ['before_publication.rs','before_publication/attempt.rs','before_publication/report.rs','publication_tests.rs']]
audits=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
for p in audits:
 for child in newowners:assert (R/p).read_text().count(child+'"')==1,(p,child)
assert len((R/audits[2]).read_text().splitlines())==1500
sizes={line.split('\t')[0]:int(line.split('\t')[1]) for line in (R/'tests/module-size-budget.tsv').read_text().splitlines() if line and not line.startswith('#')}
for p in {r['source'] for rs in views.values() for r in rs}|set(newowners):
 if p.endswith('.rs'):assert len((R/p).read_text().splitlines())<=sizes.get(p,1500),p
# The complete prior source proof and isolated825 proof execute only against
# their own authenticated full-source inputs, with closed original Git scope.
def replay(stage,script):
 old=R/script;src=old.read_text();named={(R/r['source']).resolve():r['edits'][0]['old'].encode() for r in views[stage]};rb0,rt0,wt0,co0=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output;capture=[]
 def rb(p):return named[p.resolve()] if p.resolve() in named else rb0(p)
 def rt(p,*a,**kw):return named[p.resolve()].decode() if p.resolve() in named else rt0(p,*a,**kw)
 def wt(p,s,*a,**kw):assert p.resolve()==(old.parent/'source-proof.json').resolve();capture.append(json.loads(s));return len(s)
 def co(args,*a,**kw):
  if isinstance(args,list) and args[:3]==['git','diff','--name-only'] and '--' in args:
   i=args.index('--');revs=[x for x in args[3:i] if not x.startswith('-')]
   if len(revs)==1:args=args[:i]+[meta['branches'][stage]]+args[i:]
  return co0(args,*a,**kw)
 try:
  Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb,rt,wt,co
  with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(src,str(old),'exec'),{'__file__':str(old),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb0,rt0,wt0,co0
 return capture[-1] if capture else json.loads(out.getvalue())
prior=replay('d4','docs/evidence/main927-hex-starter-assembly-20261015/verify.py')
partial=replay('825','docs/evidence/preowned-setup-publication-825/verify.py')
assert partial['issue825']=='OPEN_PARTIAL'
# Authored production relative reader and three unchanged input-channel sources.
app='crates/semaprax-toolchain/src/new_project/application.rs';s=(R/app).read_text();assert s==get(meta['relative_fix'],app).decode()
old=get(meta['source_basis'],app).decode();start='fn read_bound_with_hook(';end='pub(super) fn run(';assert s[s.index(start):s.index(end)]==old[old.index(start):old.index(end)]
for p in ['crates/semaprax-toolchain/tests/cli_new_project_v1/application_starter.rs','docs/APPLICATION-STARTER-V1.md','docs/DEVELOPMENT.md']:assert (R/p).read_bytes()==get(meta['relative_fix'],p)
assert s.index('let plan_path = selected_input_path(&plan_path)?;')<s.index('let plan = read_bound_with_hook(')<s.index('selected_input_path(&file)')<s.index('let command_bytes =')<s.index('let manifest =')
# No old evidence/JSON/proof/script/archive is mutated by this assembly.
changes=subprocess.check_output(['git','diff','--name-status',meta['source_basis'],'--','docs/evidence'],text=True).splitlines();assert all(x.startswith('A\t') for x in changes),changes
for folder,rev in [('preowned-setup-publication-825',meta['branches']['825']),('starter-relative-inputs-837-20261015',meta['relative_fix'])]:assert not subprocess.check_output(['git','diff','--name-only',rev,'--','docs/evidence/'+folder],text=True).strip()
proof={'status':'SOURCE_ONLY_NO_RUST_GATES','source_basis':meta['source_basis'],'complete_branch_rows':{s:len(v) for s,v in views.items()},'typed825_all21_original_full_assertions_replayed':True,'three_audit_owners_each_include_all_four_new_sources':True,'interop_inventory_lines':1500,'stale_and_altered_refusals':negative,'complete_prior_d4_source_proof':prior,'isolated_825_source_proof':partial,'relative_held_reader_byte_exact':True,'relative_resolution_read_order_preserved':True,'all_existing_evidence_unchanged':True,'issue825':'OPEN_PARTIAL','runtime_test_bodies_executed':0}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({k:v for k,v in proof.items() if k not in ['complete_prior_d4_source_proof','isolated_825_source_proof']}))
