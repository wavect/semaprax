from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
B='bac678f344df94fb4aa8de6190add06be5d7684f';U='7c1722091c59fe14c4e2fd5f13ee1e55e8c33e89';I='9e9416ab62065bfcd33148b9b371ec44338fad20';F='fac23f62fe1a931cf5f5c7ad9526534ed5cefad9';OLD='2d4b68d3a416cae5fba6ac08498cd56721878ba1'
def restore(row,b):
 assert sha(b)==row['current_sha256'],row['source'];s=b.decode()
 for e in row['edits']:
  assert s.count(e['new'])==1,row['source'];s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==row['original_sha256'],row['source'];return s.encode()
views={};proof={}
for name,rev in [('bac',B),('7c',U),('9e',I)]:
 rows=json.loads((D/('final-to-'+name+'.json')).read_text());v={}
 for row in rows:
  p=row['source'];v[p]=restore(row,(R/p).read_bytes());assert v[p]==git('show',rev+':'+p)==(R/row['archive']).read_bytes()
 changed=git('diff',rev,'--name-only','--diff-filter=M').decode().splitlines();assert set(changed)==set(v),(name,set(changed)^set(v))
 assert all(not p.startswith('docs/evidence/') or p=='docs/evidence/darwin-native-link-reference-v1/README.md' for p in changed)
 views[name]=v;proof[name]={'rows':len(rows),'compact_bytes':(D/('final-to-'+name+'.json')).stat().st_size,'complete_checkpoint_namespace_restored':True}
def current(name,p):return views[name].get(p,(R/p).read_bytes())
for file,rev in [('preflight-binding-successor/current-to-fac.json',F),('current-to-2d4b.json',OLD)]:
 rows=json.loads((R/'docs/evidence/closure-unit-receiving-831-next-20261016'/file).read_text())
 for row in rows:
  p=row['source'];b=restore(row,current('7c',p));assert b==git('show',rev+':'+p)==(R/row['original_archive']).read_bytes();views['7c'][p]=b
proof['unit_component_stages']={'successor_rows':3,'original_rows':9,'full_namespace_to_2d4b':True}
shipping_unit=['src/cleanup_plan/replay/factored/canonical_shared.rs','src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs','src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs']
shipping_inline=['src/cleanup_plan/replay.rs','src/cleanup_plan/replay/branch_clone.rs']
for p in shipping_unit:assert (R/p).read_bytes()==git('show',U+':'+p)
for p in shipping_inline:assert (R/p).read_bytes()==git('show',I+':'+p)
helper=(R/'src/source_reflows_merge.rs').read_text();oldhelper=git('show',B+':src/source_reflows_merge.rs').decode();assert helper[helper.index('fn project('):helper.index('// Exact post-build')]==oldhelper[oldhelper.index('fn project('):oldhelper.index('// Exact post-build')]
for p in ['src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs']:
 assert (R/p).read_bytes()==git('show',B+':'+p)
# Every previously tracked evidence JSON is unchanged; therefore all ninety fixed
# runtime fixture identities remain exact without re-reading their 93MB payload.
assert not git('diff',B,'--name-only','--diff-filter=M','--','docs/evidence/**/*.json').strip()
new=[p for p in git('diff',B,'--name-only','--diff-filter=A','--','src','tests').decode().splitlines() if p.endswith('.rs')];assert len(new)==7
for owner in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/owner).read_text()
 for p in new:assert text.count('/'+p+'"')==1,(owner,p)
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
parser=R/'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py';ns={};exec(compile(parser.read_text(),str(parser),'exec'),ns);ns['R']=R;original=ns['parse'];loaded=[]
def capture(path,*a,**kw):
 path=Path(path).resolve();assert path.is_file(),str(path);loaded.append(str(path.relative_to(R)));return original(path,*a,**kw)
ns['parse']=capture
for p,prefix,custom in [('src/cleanup_plan/replay/factored/canonical_shared.rs',['cleanup_plan','replay','factored','canonical_shared'],False),('src/cleanup_plan/replay/branch_clone.rs',['cleanup_plan','replay','branch_clone'],False),('src/cleanup_plan/replay/path_join/main_single_groups/mod.rs',['cleanup_plan','replay','path_join','main_single_groups'],True)]:capture(R/p,prefix,[],'lib','semaprax',custom_path=custom)
assert set(new)<=set(loaded)
binding={'mode':'SOURCE_ONLY_NO_RUST_DISCOVERY','parser_sha256':sha(parser.read_bytes()),'loaded_sources':sorted(set(loaded)),'checks':[{'package':k[0],'kind':k[1],'selector':k[2],**v}for k,v in ns['IDX'].items()]};(D/'recursive-module-binding.json').write_text(json.dumps(binding,indent=2)+'\n')
caps={}
for line in (R/'tests/module-size-budget.tsv').read_text().splitlines():
 if line and not line.startswith('#'):
  p,n=line.split();caps[p]=int(n)
changed=git('diff',B,'--name-only','--','src','tests','crates').decode().splitlines()
for p in changed:
 if p.endswith('.rs') and (R/p).is_file():assert len((R/p).read_text().splitlines())<=caps.get(p,1500),p
for p in ['docs/evidence/darwin-native-link-reference-v1/README.md','.github/workflows/ci.yml','tests/project_product_acceptance_ci_contract.rs']:
 assert (R/p).read_bytes()==git('show','531bde1ac6b835b3b4e44e0d0b15f3a9fc5e6d07:'+p)
proof.update(status='SOURCE_ONLY_PASS',shipping_unit_exact_7c=True,shipping_inline_exact_9e=True,original_project_body_exact=True,original_90_runtime_fixtures_unchanged=True,new_closed_Rust_owners=7,recursive_loaded_files=len(set(loaded)),Rust_executed=False,inherited_bac_proof_reuse='Complete restored namespace equality; predecessor scripts are historical and not executed against raw final source.')
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
