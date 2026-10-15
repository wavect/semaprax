"""Bounded source-only MAIN927/complete9b custody, never Rust execution."""
from pathlib import Path
import contextlib, hashlib, io, json, re, subprocess
R=Path.cwd();D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
meta=json.loads((D/'basis.json').read_text())
def get(rev,p):return subprocess.check_output(['git','show',rev+':'+p],cwd=R)
def apply(rows,p,b):
 found=[x for x in rows if x['source']==p];assert len(found)<=1
 if not found:return b
 row=found[0];assert sha(b)==row['current_sha256'],p
 for e in row['edits']:
  a=e['new'].encode();assert b.count(a)==1,p;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==row['original_sha256'],p
 return b
views={s:json.loads((D/('current-to-'+s+'.json')).read_text()) for s in ('9b','main927')}
negative=0
for stage,rows in views.items():
 rev=meta['source_basis'] if stage=='9b' else meta['actual_main']
 for row in rows:
  p=row['source'];raw=(R/p).read_bytes();old=apply(rows,p,raw)
  assert old==get(rev,p)==(R/row['archive']).read_bytes()
  for bad in (raw+b' ',old):
   try:apply(rows,p,bad)
   except AssertionError:negative+=1
   else:raise AssertionError('altered/stale input accepted: '+p)
old93=(R/meta['old93_runtime']).read_bytes()
assert sha(old93)==meta['old93_sha256'] and old93==get(meta['source_basis'],meta['old93_runtime'])
assert len(json.loads(old93))==93
actual=(D/'actual-main927-reflows.json').read_bytes()
assert actual==get(meta['actual_main'],'src/source_reflows.json')==(R/'src/source_reflows.json').read_bytes()
records=json.loads(actual);assert len(records)==87
for row in records:
 p=row['path'];raw=apply(views['main927'],p,(R/p).read_bytes());assert sha(raw)==row['after_sha256'],p
 for edit in row['inverse']:
  a=edit['after'].encode();assert raw.count(a)==1,p;raw=raw.replace(a,edit['before'].encode(),1)
 assert sha(raw)==row['before_sha256'],p
# Replay MAIN's exact semantic whole-root and whole-inventory assertions.
reference=get(meta['actual_main'],'src/workspace_graph/identity_resolution/base/cache_billing/source_reference.rs').decode()
def rawconst(name):return re.search(r'const '+name+r': &str = r###"(.*?)"###;',reference,re.S).group(1)
root=apply(views['main927'],'src/workspace_graph.rs',(R/'src/workspace_graph.rs').read_bytes()).decode()
assert sha(root.encode())=='3fcd39c7b442be79e1200493893a14f7ad9a00c242e54b40b090c102e3e44561'
for i in range(3):
 a=rawconst('AFTER_'+str(i));assert root.count(a)==1;root=root.replace(a,rawconst('BEFORE_'+str(i)),1)
assert sha(root.encode())=='de94863e39460dd9270b2feea2fc7a4986d25417e20ee2cfa4b38412e9a13f55'
a=rawconst('AFTER_3');assert root.count(a)==1;root=root.replace(a,rawconst('BEFORE_3'),1)
assert sha(root.encode())=='c82d3bb6176c847fd9ff171f02d9c5aedd113e72b1eb9b1739ffee062319c11b'
a=rawconst('CURRENT_CALL');assert root.count(a)==1;root=root.replace(a,rawconst('ORIGINAL_CALL'),1)
assert sha(root.encode())=='fe644b87c264e23b9afa0660d4bcd8149583f19438146734d258025c27087771'
p='tests/workspace/semantic_graph/source_inventory.rs';inventory=apply(views['main927'],p,(R/p).read_bytes()).decode()
assert sha(inventory.encode())=='e55bd484d5c38c005501cbb3b48493907cd7fc370ab083b810cc71a4b3b0ce0b'
for i in range(3):
 a=rawconst('ADDITIONS_'+str(i));assert inventory.count(a)==1;inventory=inventory.replace(a,'',1)
assert sha(inventory.encode())=='d4158fd988538e77c0f39a2f05cfc50aab15679744bba14ae1f005fc32c805fb'
new=[x['source'] for x in meta['new_owners']];assert len(new)==3
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 source=(R/p).read_text()
 for child in new:assert source.count(child+'"')==1,(p,child)
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
sizes={line.split('\t')[0]:int(line.split('\t')[1]) for line in (R/'tests/module-size-budget.tsv').read_text().splitlines() if line and not line.startswith('#')}
for p in meta['changed_physical_inputs']:
 if p.endswith('.rs'):assert len((R/p).read_text().splitlines())<=sizes.get(p,1500),p
# Full old source suite executes only against authenticated complete9b bytes.
# Its exact Git diff scope is9b, never the later working merge. Old code untouched.
old=R/'docs/evidence/final-ae91-mainf045-assembly-20261015/verify.py'
source=old.read_text();named={(R/x['source']).resolve():x['edits'][0]['old'].encode() for x in views['9b']}
rb0,rt0,wt0,co0=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output
captured=[]
def rb(p):return named[p.resolve()] if p.resolve() in named else rb0(p)
def rt(p,*args,**kw):return named[p.resolve()].decode() if p.resolve() in named else rt0(p,*args,**kw)
def wt(p,value,*args,**kw):
 assert p.resolve()==(old.parent/'source-proof.json').resolve(),p
 captured.append(json.loads(value));return len(value)
def co(args,*rest,**kw):
 if isinstance(args,list) and args[:3]==['git','diff','--name-only'] and '--' in args:
  split=args.index('--');revisions=[a for a in args[3:split] if not a.startswith('-')]
  if len(revisions)==1:args=args[:split]+[meta['source_basis']]+args[split:]
 return co0(args,*rest,**kw)
try:
 Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb,rt,wt,co
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(source,str(old),'exec'),{'__file__':str(old),'__name__':'__main__'})
finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb0,rt0,wt0,co0
assert captured
# Immutable old proof/runtime metadata is never rewritten by the source replay.
changes=subprocess.check_output(['git','diff','--name-only',meta['source_basis'],'--','docs/evidence'],cwd=R,text=True).splitlines()
assert all(p.startswith(str(D.relative_to(R))+'/') for p in changes),changes
result={'status':'SOURCE_ONLY_NO_RUST_GATES','source_basis':meta['source_basis'],'actual_main':meta['actual_main'],'current_to_9b':len(views['9b']),'current_to_main927':len(views['main927']),'old93_immutable':True,'new_main87_full_inverses':True,'main_semantic_root_and_inline_full_assertions':True,'three_new_owners_in_all_three_audits':True,'interop_inventory_lines':1500,'altered_and_stale_refusals':negative,'complete_original_9b_source_proof_replayed':captured[-1],'runtime_or_app_recovery_claim':False}
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='complete_original_9b_source_proof_replayed'}))
