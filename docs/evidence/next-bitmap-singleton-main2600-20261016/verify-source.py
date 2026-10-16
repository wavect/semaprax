"""Bounded complete namespace proof; no compiler or runtime execution."""
from pathlib import Path
import json,hashlib,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='4fba2ee58ab595eb6ee1dc9aaa4bfa975358a7a4';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*x):return subprocess.check_output(['git',*x],cwd=R)
def restore(table,path,current):
 matches=[r for r in table if r['source']==path];assert len(matches)<=1
 for row in matches:
  assert sha(current)==row['current_sha256'];text=current.decode()
  for e in row['edits']:assert e['new'] and text.count(e['new'])==1; text=text.replace(e['new'],e['old'],1)
  current=text.encode();assert sha(current)==row['original_sha256']
 return current
rows=json.loads((D/'current-to-4fba.json').read_text());assert rows==json.loads((D/'source-inverses.json').read_text())
for row in rows:
 p=row['source'];old=git('show',B+':'+p);assert restore(rows,p,(R/p).read_bytes())==old==(R/row['archive']).read_bytes()
modified=git('diff',B,'--name-only','--diff-filter=M').decode().splitlines();oldpaths=set(git('ls-tree','-r','--name-only',B).decode().splitlines())
assert not [p for p in modified if p.startswith('docs/evidence/')], 'previous immutable evidence changed'
expected={p for p in modified if not p.startswith('docs/evidence/')};assert expected=={r['source']for r in rows}
# The fixed90 transport specifications and each fixture remain exact. The Git
# diff above authenticates the complete immutable old evidence namespace.
for p in ['src/source_reflows_merge/runtime_json.rs','src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','src/source_reflows_merge/runtime_json/tests.rs']:assert (R/p).read_bytes()==git('show',B+':'+p)
helper=(R/'src/source_reflows_merge.rs').read_text();oldhelper=git('show',B+':src/source_reflows_merge.rs').decode();a=helper.index('fn project(');b=helper.index('// Exact post-build',a);c=oldhelper.index('fn project(');e=oldhelper.index('// Exact post-build',c);assert helper[a:b]==oldhelper[c:e]
# New live Rust owners are closed in all3 unions; old include order/multiplicity
# is independently checked by verify-authority.py.
newowners=git('diff',B,'--name-only','--diff-filter=A').decode().splitlines();newowners=[p for p in newowners if p.endswith('.rs') and not p.startswith('docs/evidence/')]
for p in newowners:
 assert len((R/p).read_bytes().splitlines())<=1500
 for owner,prefix in [('tests/hir_module_boundaries.rs','../'),('tests/workspace/semantic_graph/source_inventory.rs','../../'),('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','../../../../../')]:assert (R/owner).read_text().count('include_str!("'+prefix+p+'")')==1,(owner,p)
# Independent bitmap namespace before BOTH immutable predecessor tables.
target='22fb5577da4e095c1d11ae2df6e513ade6167022';direct=json.loads((D/'current-to-22fb.json').read_text());part=R/'docs/evidence/bitmap-direct-reads-831-next-20261016';component=json.loads((part/'source-inverses.json').read_text());prior=json.loads((R/'docs/evidence/empty-closure-bitmap-831-next-20261016/source-inverses.json').read_text());views={}
for row in component['rows']:
 p=row['source'];current=restore(direct,p,(R/p).read_bytes());assert current==git('show',target+':'+p)==row['after'].encode();assert sha(current)==row['after_sha256'];views[p]=row['before'].encode();assert views[p]==git('show',component['predecessor']+':'+p)
for row in prior['rows']:
 p=row['source'];current=views.get(p,restore(direct,p,(R/p).read_bytes()));assert current==row['after'].encode();assert row['before'].encode()==git('show',prior['base']+':'+p)
for row in json.loads((part/'active-new-owners.json').read_text()):assert sha((R/row['source']).read_bytes())==row['sha256']
# Named Main and singleton component cuts stay physical-exact: their strict
# runtime source readers require no speculative extra preprojection.
for rev,packet in [('9fdc5a74c9591307ba600e0f8bf321e23813cc70','branch-clone-funded-next-20261016'),('0d47dfe90c8033e783579c0d9cfbf40201c30bb9','singleton-path-group-next-20261016'),('6a7b4f02e5a63fb73973bdef8d28ac574ec34559','main-eb13-browser-join-20261016')]:
 paths=git('diff',rev+'^',rev,'--name-only','--','src','tests','platform-tests').decode().splitlines()
 for p in paths:
  if (R/p).is_file() and p not in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs']:assert (R/p).read_bytes()==git('show',rev+':'+p),(rev,p)
for p in ['.github/workflows/ci.yml','scripts/ci-msrv.py','scripts/ci-darwin-native-reference.py','tests/project_product_acceptance_ci_contract.rs','platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs','docs/QUALITY-GATES.md']:assert (R/p).read_bytes()==git('show','eb13ed02c3a3522f1ead1fd3bd53c58439a98da1:'+p)
# Every affected live Rust file respects its recorded ceiling or the1500 default.
budget={}
for line in (R/'tests/module-size-budget.tsv').read_text().splitlines():
 if line and not line.startswith('#'):
  fields=line.split('\t')
  if len(fields)>=2 and fields[1].isdigit():budget[fields[0]]=int(fields[1])
for p in set(modified+newowners):
 if p.endswith('.rs') and not p.startswith('docs/evidence/'):assert len((R/p).read_bytes().splitlines())<=budget.get(p,1500),(p,len((R/p).read_bytes().splitlines()),budget.get(p,1500))
# Actual failed SourceA classifier input and virtual facade retain exact old
# whole inputs; the active readers keep their new narrowly scoped first hops.
source='src/bounded_output/classifier_string.rs';current=(R/source).read_bytes()
for table in [rows]+[json.loads((R/'docs/evidence'/name).read_text()) for name in ['next-main-beb0-847-joined-20261016/current-to-8bb.json','private-175-actual-fixes-20261015/current-to-c159.json','ordered-amortized-root-domain-20261015/current-to-35cc.json','storage-oracle-compiler-fix-join-20261015/current-to-d719.json','sourceguard-input-domains-7a87/classifier-input.json']]:current=restore(table,source,current)
assert sha(current)=='ee3073593ac4c8ab657dc5daa7500108b60c4c64bc898e877208f60a10d56a34'
assert 'pub(crate) fn source36_classifier_input(source: &str, current: &str) -> String {\n    let staged = before_storage_oracle_join(source, current);' in helper
inventory=(R/'tests/workspace/semantic_graph/source_inventory.rs').read_text();block=inventory[inventory.index('macro_rules! semantic_graph_projection_root_source_inventory'):inventory.index('macro_rules! semantic_graph_projection_child_source_inventory')];parts=[]
for path,literal in re.findall(r'include_str!\(\s*"([^"]+)"\s*\)|((?:"(?:\\.|[^"\\])*"))',block):parts.append(json.loads(literal) if literal else (R/'tests/workspace'/path).read_text())
facade=json.loads((R/'docs/evidence/projection-facade-frontier-8bb-20261016/root-input.json').read_text());previous=restore([facade],'expected_projection::root_join',''.join(parts).encode());legacy=json.loads((R/'docs/evidence/ae91-source-consumer-corrections-20261015/facade.json').read_text());assert previous.decode()==legacy['edits'][0]['new'] and sha(previous)==legacy['current_sha256']
proof={'status':'SOURCE_ONLY_PASS','old_namespace':B,'whole_old_inputs':len(rows),'compact_input_bytes':(D/'current-to-4fba.json').stat().st_size,'old_evidence_files_unchanged':sum(p.startswith('docs/evidence/') for p in oldpaths),'new_live_Rust_owners':len(newowners),'bitmap_independent_rows':len(direct),'fixed90_runtime_transport_unchanged':True,'old_project_body_exact':True,'Main_barrier':'eb13ed02c3a3522f1ead1fd3bd53c58439a98da1','runtime_executed':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');(D/'new-owners.json').write_text(json.dumps([{'source':p,'sha256':sha((R/p).read_bytes()),'lines':len((R/p).read_bytes().splitlines())}for p in sorted(newowners)],indent=2)+'\n');print(json.dumps(proof,indent=2))
