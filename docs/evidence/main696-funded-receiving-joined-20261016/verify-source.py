"""Current joined custody entrypoint. No Rust execution or behavioral PASS."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='ff7e1afd945de67b0e51125168a273e6f93fc73c';C='364d824194cd1df76b5c273aa0fb9cbafb602e29';M='6966765c965c8a5ad85ae3c2827653599826fb88';P='d201b9cdab9d45a7b0b5e3a401316408c401507b';U='3ae2fbb5710b9e85aef2d2a91d42a7a782216d76'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def restore(row,b):
 assert sha(b)==row['current_sha256'],row['source'];s=b.decode()
 for e in row['edits']:
  assert s.count(e['new'])==1,row['source'];s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==row['original_sha256'],row['source'];return s.encode()
rows=json.loads((D/'current-to-ff7e.json').read_text());v={}
for row in rows:
 p=row['source'];v[p]=restore(row,(R/p).read_bytes());assert v[p]==git('show',B+':'+p)==(R/row['archive']).read_bytes()
assert set(v)==set(git('diff',B,'--name-only','--diff-filter=M').decode().splitlines())
oldD=R/'docs/evidence/main696-source-reconciliation-20261016';restored={}
for domain,target in [('364d',C),('main696',M)]:
 values={}
 for row in json.loads((oldD/('current-to-'+domain+'.json')).read_text()):
  p=row['source'];values[p]=restore(row,v.get(p,(R/p).read_bytes()));assert values[p]==git('show',target+':'+p)==(R/row['archive']).read_bytes()
 restored[domain]=values
# All four original Main semantic inputs then their original formatting inverses.
formatting={x['path']:x for x in json.loads((R/'docs/evidence/mainf045-source-successor-20261015/live-mainf045-ci93.json').read_text())}
for row in json.loads((R/'docs/evidence/receiver-source-custody-v1/inverse.json').read_text()):
 p=row['path'];b=restored['main696'].get(p,v.get(p,(R/p).read_bytes()));assert b==git('show',M+':'+p) and sha(b)==row['after_sha256'];s=b.decode()
 for e in row['inverse']:
  assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
 assert sha(s.encode())==row['before_sha256'] and s.encode()==(R/'docs/evidence/receiver-source-custody-v1'/row['archive']).read_bytes()
 if p in formatting:
  f=formatting[p];assert sha(s.encode())==f['after_sha256']
  for e in f['inverse']:
   assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
  assert sha(s.encode())==f['before_sha256']
# Main corrected component files are unchanged, so its own bounded exact-body
# proof runs in its actual owning namespace. The831 global-scope predecessor
# verifier is historical; replay its two complete rows instead of weakening it.
for p in ['src/cleanup_plan/replay.rs','src/cleanup_plan/replay/path_array.rs','src/cleanup_plan/replay/path_array/before.rs','src/cleanup_plan/replay/path_array/tests.rs','src/cleanup_plan/replay/path_array/main696.rs','src/cleanup_plan/replay/path_array/funded_tests.rs']:assert (R/p).read_bytes()==git('show',P+':'+p)
for row in json.loads((R/'docs/evidence/lexical-scratch-receiving-831-next-20261016/current-to-364d.json').read_text()):
 p=row['source'];assert (R/p).read_bytes()==git('show',U+':'+p);assert restore(row,(R/p).read_bytes())==git('show',C+':'+p)
for script in ['docs/evidence/path-array-first-output-funding-20261016/verify-source.py','docs/evidence/alias-project-provider-fixture-847-20261016/verify-source.py']:
 subprocess.run(['python3',str(R/script)],cwd=R,check=True,stdout=subprocess.DEVNULL)
assert not git('diff',B,'--name-only','--diff-filter=M','--','docs/evidence/**/*.json').strip()
for p in ['src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','src/cleanup_plan/replay/branch_clone.rs','src/cleanup_plan/replay/branch_clone/inline_tests.rs']:assert (R/p).read_bytes()==git('show',C+':'+p)
helper=(R/'src/source_reflows_merge.rs').read_text();old=git('show',B+':src/source_reflows_merge.rs').decode();assert helper[helper.index('fn project('):helper.index('// Exact post-build')]==old[old.index('fn project('):old.index('// Exact post-build')]
assert helper.count('let corrected = main696_corrected_input(source, current);')==2
new=['src/cleanup_plan/replay/path_array/main696.rs','src/cleanup_plan/replay/path_array/funded_tests.rs']
for owner in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 for p in new:assert (R/owner).read_text().count('/'+p+'"')==1
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
caps={}
for line in (R/'tests/module-size-budget.tsv').read_text().splitlines():
 if line and not line.startswith('#'):
  p,n=line.split();caps[p]=int(n)
for p in git('diff',B,'--name-only','--','src','tests','crates').decode().splitlines():
 if p.endswith('.rs') and (R/p).is_file():assert len((R/p).read_text().splitlines())<=caps.get(p,1500),p
parser=R/'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py';ns={};exec(compile(parser.read_text(),str(parser),'exec'),ns);ns['R']=R;original=ns['parse'];loaded=[]
def capture(path,*a,**kw):
 path=Path(path).resolve();assert path.is_file(),str(path);loaded.append(str(path.relative_to(R)));return original(path,*a,**kw)
ns['parse']=capture
capture(R/'src/cleanup_plan/replay/path_array.rs',['cleanup_plan','replay','path_array'],[],'lib','semaprax')
capture(R/'src/source_reflows.rs',['source_reflows'],[],'lib','semaprax')
assert set(new)<=set(loaded)
checks=[{'package':k[0],'kind':k[1],'selector':k[2],**value}for k,value in ns['IDX'].items()];assert len([x for x in checks if x['selector'].startswith('cleanup_plan::replay::path_array::tests::')])==4;assert len([x for x in checks if x['selector'].startswith('cleanup_plan::replay::path_array::funded_tests::')])==2
(D/'recursive-module-binding.json').write_text(json.dumps({'mode':'SOURCE_ONLY_NO_RUST_DISCOVERY','parser_sha256':sha(parser.read_bytes()),'loaded_sources':sorted(set(loaded)),'checks':checks},indent=2)+'\n')
proof={'status':'SOURCE_ONLY_PASS','whole_current_to_ff7e_rows':len(rows),'compact_bytes':(D/'current-to-ff7e.json').stat().st_size,'restored_original_ff7e_namespace':True,'original_27_cutoff_and_3_Main_rows_replayed':True,'four_Main_semantic_and_formatting_inverses_pass':True,'reviewed_Main_correction_exact_d201':True,'reviewed_831_shipping_exact_3ae':True,'original_90_fixture_identities_and_all_old_JSON_unchanged':True,'original_project_function_exact':True,'two_new_closed_children_all3_raw_inventory':True,'recursive_loaded_files':len(set(loaded)),'Main_original_four_names_preserved':True,'module_caps_pass':True,'Rust_executed':False,'prior_proof_reuse':'Complete restored namespace equality; the original831 global-scope verifier refused joined raw source and remains unchanged. Its two exact whole rows are replayed here.'};(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
