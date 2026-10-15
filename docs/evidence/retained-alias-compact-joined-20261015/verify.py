"""Finite independent branch-domain replay, no Rust/toolchain execution."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
h=lambda b:hashlib.sha256(b).hexdigest();g=lambda *a:subprocess.check_output(['git',*a],cwd=R)
revs={'453':'45341c420645df90c8433253fd4a16128f1c0827','523':'523f03a260ac6672639a7344c1ad90fe06dca4ba','7f':'7faff0dcfba890bf2cc29b2d13c125f2c99062d9'}
views={};tables={}
for stage,rev in revs.items():
 rows=json.loads((D/f'final-to-{stage}.json').read_text());tables[stage]=rows;named={}
 for row in rows:
  raw=(R/row['source']).read_bytes();old=row['edits'][0]['old'].encode()
  assert raw==row['edits'][0]['new'].encode() and h(raw)==row['current_sha256']
  assert h(old)==row['original_sha256'] and old==(R/row['archive']).read_bytes()==g('show',rev+':'+row['source'])
  named[(R/row['source']).resolve()]=old
 views[stage]=named
# Both actual compile-repair branches restore the identical complete677 source.
packets={'523':'compact-old-receiver-compile-fix-844-20261015','7f':'field-update-hir-visitor-836-20261015'}
inputs=set(r['source'] for table in tables.values() for r in table)
def original(stage,path):return views[stage].get((R/path).resolve(),(R/path).read_bytes())
for path in inputs:
 results=[]
 for stage,packet in packets.items():
  text=original(stage,path).decode()
  for row in json.loads((R/'docs/evidence'/packet/'current-to-677.json').read_text()):
   if row['source']!=path:continue
   assert h(text.encode())==row['current_sha256']
   for e in row['edits']:assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
   assert h(text.encode())==row['original_sha256']
  results.append(text)
 assert results[0]==results[1],path
# Exact new847 production and independent old body; only the explicit test-loader adapter changes.
for row in json.loads((R/'docs/evidence/retained-function-alias-847/new-owners.json').read_text()):
 p=row['source'];raw=(R/p).read_bytes();before=g('show',revs['453']+':'+p)
 if p.endswith('function_alias/before_core.rs'):
  assert raw==before.replace(b'#[cfg(test)]\nmod tests;',b'#[cfg(test)]\n#[path = "before_core/tests.rs"]\nmod tests;').replace(b'                    cache.checked_module_source(&program.path, input),\n',b'                    cache.checked_module_source(&program.path, input),\n                expected_projection::borrowed_leaf::Input::Transient(_) =>\n                    unreachable!("complete old constructor cannot produce transient function view"),\n')
 else:assert raw==before,p
for p in ['src/ast/program_source.rs','src/ast/program_source/decl_slices.rs','src/hir/identity_resolution.rs','src/workspace_graph.rs','src/workspace_graph/checked_retention.rs','src/workspace_graph/constructor_failure/typed.rs','src/workspace_graph/expected_projection/borrowed_imports/typed.rs','src/workspace_graph/expected_projection/borrowed_leaf.rs']:
 assert (R/p).read_bytes()==g('show',revs['453']+':'+p),p
# Old factories remain old: only exhaustive enum compatibility is added to copies.
compat=b'                expected_projection::borrowed_leaf::Input::Transient(_) =>\n                    unreachable!("complete old constructor cannot produce transient function view"),\n'
for p in ['src/workspace_graph/constructor_failure/receiving/before.rs','src/workspace_graph/cache_key_original.rs']:
 old=g('show',revs['453']+':'+p);raw=(R/p).read_bytes();needle=b'                    cache.checked_module_source(&program.path, input),\n'
 assert old.count(needle)==1 and raw==old.replace(needle,needle+compat),p
assert (R/'src/workspace_graph/constructor_failure.rs').read_bytes()==g('show',revs['453']+':src/workspace_graph/constructor_failure.rs')
leaf=(R/'src/workspace_graph/expected_projection/borrowed_leaf.rs').read_text()
start=leaf.index('    pub(in crate::workspace_graph) fn construct_transient(');end=leaf.index('    // CORE_TRANSIENT_BEGIN',start)
assert 'Transient' not in leaf[start:end] and 'Self::Borrowed' in leaf[start:end] and 'Self::Owned' in leaf[start:end]
# Protected historical runtime data/code snapshots are never rewritten.
protected=0
for rev,packet in [(revs['453'],'retained-function-alias-847'),(revs['523'],packets['523']),(revs['7f'],packets['7f'])]:
 for path in g('ls-tree','-r','--name-only',rev,'--','docs/evidence/'+packet).decode().splitlines():
  assert (R/path).read_bytes()==g('show',rev+':'+path),path;protected+=1
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/p).read_text();assert len(text.splitlines())<=1500
 for row in json.loads((R/'docs/evidence/retained-function-alias-847/new-owners.json').read_text()):assert row['source'][4:]+'"' in text
# Unchanged branch verifiers see their complete named source tree, with exact missing paths.
results={};rb,rt,wt,co,glob,exists=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob,Path.exists
for stage,script in [('453','retained-function-alias-847'),('523',packets['523']),('7f',packets['7f'])]:
 named=views[stage];excluded={(R/p).resolve() for p in json.loads((D/f'absent-{stage}.json').read_text())};captured=[];out=io.StringIO();path=R/'docs/evidence'/script/'verify.py';code=path.read_text()
 def read_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
 def read_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
 def write(p,s,*a,**k):assert p.name=='source-proof.json';captured.append(json.loads(s));return len(s)
 def output(args,*a,**k):
  if isinstance(args,list) and args[:2]==['git','diff']:
   i=args.index('--') if '--' in args else len(args);revargs=[x for x in args[2:i] if not x.startswith('-')]
   if len(revargs)==1:args=args[:i]+[revs[stage]]+args[i:]
  return co(args,*a,**k)
 def rglob(p,*a,**k):
  for child in glob(p,*a,**k):
   if child.resolve() not in excluded:yield child
 try:
  Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob,Path.exists=read_bytes,read_text,write,output,rglob,lambda p:False if p.resolve() in excluded else exists(p)
  with contextlib.redirect_stdout(out):exec(compile(code,str(path),'exec'),{'__file__':str(path),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob,Path.exists=rb,rt,wt,co,glob,exists
 results[stage]=captured[-1] if captured else json.loads(out.getvalue())
proof={'status':'SOURCE_ONLY_NO_RUST','independent_branch_rows':{k:len(v) for k,v in tables.items()},'both_compile_branches_restore_identical_677':True,'protected_packet_files_byteexact':protected,'new_test_bodies':8,'original_branch_proofs':results,'gain_or_fit_claim':None}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS independent453/523/7f complete inputs and unchanged proofs; NO Rust')
