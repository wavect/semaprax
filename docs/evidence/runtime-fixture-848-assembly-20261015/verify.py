from pathlib import Path
import contextlib,hashlib,io,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;H='999fae8f3b82f1ff68408849b4684ea1dd49a1f9';B='8c974ceb05e0bbd63fda796054b00f68f90b5938';g=lambda *a:subprocess.check_output(['git',*a],cwd=R);h=lambda b:hashlib.sha256(b).hexdigest()
def table(name,rev):
 rows=json.loads((D/name).read_text());named={}
 for row in rows:
  p=(R/row['source']).resolve();assert p not in named;raw=p.read_bytes();assert h(raw)==row['current_sha256'];old=raw
  for e in row['edits']:assert old.count(e['new'].encode())==1;old=old.replace(e['new'].encode(),e['old'].encode(),1)
  assert h(old)==row['original_sha256'] and old==(R/row['archive']).read_bytes()==g('show',rev+':'+row['source']);named[p]=old
 return rows,named
immediate,old999=table('current-to-999fa.json',H);direct,old848direct=table('current-to-848.json',B);full,old848=table('complete-848-inputs.json',B);assert len(immediate)==len(direct)==2
for rev,named in [(H,old999),(B,old848)]:
 tracked=set(g('ls-tree','-r','--name-only',rev).decode().splitlines())
 for p in g('diff','--name-only',rev).decode().splitlines():
  if p in tracked:assert named.get((R/p).resolve(),(R/p).read_bytes())==g('show',rev+':'+p)
assert (D/'source-inverses.json').read_bytes()==(D/'current-to-999fa.json').read_bytes()
helper='src/cleanup_plan/replay/selected_context/child/tests/source/compressed_fixture.rs';assert (R/helper).read_bytes()==g('show',B+':'+helper)
# Production, original root41assertions, both fixtures, all decoder dependencies,
# three audit bodies, and all old packet bytes stay exact in their true domains.
for p in ['Cargo.toml','Cargo.lock','src/cleanup_plan/replay/selected_context/child/tests/source.rs','tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','docs/evidence/selected-child-refusal-781/exact-source-inverses.json','docs/evidence/compressed-selected-child-fixture-841/exact-source-inverses.json.gz']:
 assert (R/p).read_bytes()==g('show',H+':'+p),p
for p in g('diff','--name-only',H).decode().splitlines():
 if g('ls-tree',H,'--',p).strip():assert p in [helper,'src/source_reflows_merge.rs'],p
for p in g('ls-tree','-r','--name-only',B,'--','docs/evidence/runtime-selected-child-fixture-848').decode().splitlines():assert (R/p).read_bytes()==g('show',B+':'+p)
s=(R/'src/source_reflows_merge.rs').read_text();prefix='    let before_runtime848 = project(\n        include_str!(concat!(\n            env!("CARGO_MANIFEST_DIR"),\n            "/docs/evidence/runtime-fixture-848-assembly-20261015/current-to-999fa.json"\n        )),\n        source,\n        current,\n    );\n    let current = before_runtime848.as_str();\n';suffix='\n// Direct runtime-fixture custody stays independent of inherited source views.\npub(crate) fn runtime848_input(source: &str, current: &str) -> String {\n    project(\n        include_str!(concat!(\n            env!("CARGO_MANIFEST_DIR"),\n            "/docs/evidence/runtime-fixture-848-assembly-20261015/current-to-848.json"\n        )),\n        source,\n        current,\n    )\n}\n';assert s.count(prefix)==1 and s.endswith(suffix);assert s[:-len(suffix)].replace(prefix,'')==g('show',H+':src/source_reflows_merge.rs').decode();assert len(s.splitlines())<=1500
# Execute the UNCHANGED848 static verifier only in its whole exact Git namespace.
# New files are hidden there; no original script or proof file is written.
script=R/'docs/evidence/runtime-selected-child-fixture-848/verify.py';code=script.read_text();tracked=set(g('ls-tree','-r','--name-only',B).decode().splitlines());rb,rt,wt,co,glob,ex=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob,Path.exists;captured=[]
def relative(p):
 try:return str(p.resolve().relative_to(R))
 except ValueError:return None
def output(args,*a,**k):
 if isinstance(args,list) and args[:2]==['git','diff']:
  i=args.index('--') if '--' in args else len(args);revs=[v for v in args[2:i] if not v.startswith('-')]
  if len(revs)==1:args=args[:i]+[B]+args[i:]
 return co(args,*a,**k)
def write(p,text,*a,**k):assert p.name=='source-proof.json';captured.append(json.loads(text));return len(text)
try:
 Path.read_bytes=lambda p,*a,**k:old848[p.resolve()] if p.resolve() in old848 else rb(p,*a,**k)
 Path.read_text=lambda p,*a,**k:old848[p.resolve()].decode() if p.resolve() in old848 else rt(p,*a,**k)
 Path.write_text=write;subprocess.check_output=output
 Path.rglob=lambda p,pattern:(x for x in glob(p,pattern) if relative(x) is None or relative(x) in tracked or x.is_dir())
 Path.exists=lambda p:False if relative(p) is not None and p.suffix and relative(p) not in tracked else ex(p)
 with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(code,str(script),'exec'),{'__file__':str(script),'__name__':'__main__'})
 prior=json.loads(out.getvalue());assert prior['original_decoder_functions_byte_exact']==5 and prior['new_IO_test_bodies']==1
finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob,Path.exists=rb,rt,wt,co,glob,ex
proof={'status':'SOURCE_ONLY_RUNTIME_FIXTURE_JOIN_NOT_EXECUTABLY_QUALIFIED','parents':[H,B],'whole_immediate999fa_rows':len(immediate),'independent_direct848_rows':len(direct),'whole848_checkpoint_namespace_rows':len(full),'original848_static_verifier_replayed_unchanged':prior,'production_unchanged':True,'old_runtime_JSON_archives_scripts_immutable':True,'all_audits_raw_gzip_and_five_dependencies_unchanged':True,'runtime_repository_fixture_required':True,'RAM_or_speed_gain_claim':False,'Rust_Cargo_Node_IO_tests_executed':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({k:v for k,v in proof.items() if k!='original848_static_verifier_replayed_unchanged'},indent=2))
