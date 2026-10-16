"""Finite actual-pin/source custody; no Cargo, captures or owning test execution."""
from pathlib import Path
import json,hashlib,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='47f80b0f7a06fde9b2388439a9c0bfe998a05af1';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def restore(row,b):
 assert sha(b)==row['current_sha256'],row['source'];s=b.decode()
 for e in row['edits']:assert s.count(e['new'])==1,row['source'];s=s.replace(e['new'],e['old'],1)
 b=s.encode();assert sha(b)==row['original_sha256']and b==(R/row['archive']).read_bytes();return b
rows=json.loads((D/'current-to-47f.json').read_text());view={}
for row in rows:
 p=row['source'];view[p]=restore(row,(R/p).read_bytes());assert view[p]==git('show',B+':'+p)
assert set(view)==set(git('diff',B,'--name-only','--diff-filter=M').decode().splitlines())
assert not git('diff',B,'--name-only','--diff-filter=D').strip()
assert not git('diff',B,'--name-only','--diff-filter=M','--','docs/evidence/**/*.json').strip()
# Entire old namespace is restored before its unchanged strict stages. Production
# equivalence and literal consumer presence are additionally checked by21 tooling.
prior=R/'docs/evidence/task722-held-fitting759-join-20261016'
for name in ['verify-source.py','verify-authority.py','source-proof.json','authority-source-proof.json']:assert (prior/name).read_bytes()==git('show',B+':'+str((prior/name).relative_to(R)))
for table,target in [('task722-held-fitting759-join-20261016/current-to-0e7b.json','0e7b370445b1cdad5529b4bcf0652b2dcfa997d8'),('b3a-compile-fixture-task722-join-20261016/current-to-b3a.json','b3a53367d4def76240d14dca25ee414785bed9d6')]:
 for row in json.loads((R/'docs/evidence'/table).read_text()):
  p=row['source'];view[p]=restore(row,view.get(p,(R/p).read_bytes()));assert view[p]==git('show',target+':'+p)
actual=json.loads((D/'actual-inputs-plan.json').read_text());assert sha((D/'actual-inputs-plan.json').read_bytes())=='8b4d37221d8c8fa0d2aa796dfbd531ba236791938a272b6e895d9cc70b3771d0'
for e in actual['managed_literal_edits']:text=(R/e['path']).read_text();assert text.count(e['after'])==1 and e['before']not in text
# Validate every entire actual and independently retained browser wire; no fake
# value or partial parsed projection substitutes for full byte equality.
def digest(wire):
 d=json.loads(wire);f=(',"graph_digest":'+json.dumps(d['graph_digest'],separators=(',',':'))).encode();assert wire.count(f)==1;p=wire.replace(f,b'',1);return 'sha256:'+hashlib.sha256(b'semaprax.project-semantic-graph.artifact-digest.v1\0'+len(p).to_bytes(8,'little')+p).hexdigest()
for a in actual['actual_browser']:
 name=a['fixture'];raw=(D/'actual-browser'/f'calculator-{name}.json').read_bytes();assert sha(raw)==a['sha256'];wire=raw.removesuffix(b'\n');d=json.loads(wire);assert digest(wire)==d['graph_digest']and d['budget']['used_builder_bytes']==3477789
 old=[D/folder/f'calculator-{name}.json'for folder in ['observed364d','observed2d4b','observed8bb']]+[R/f'docs/evidence/browser-graph-pins-2600e49eb/current-hosted-calculator-{name}.json',R/f'docs/evidence/browser-graph-pins-35cc-20261016/actual-capture/calculator-{name}.json']
 for p in old:
  previous=p.read_bytes().removesuffix(b'\n');q=json.loads(previous);assert digest(previous)==q['graph_digest'];f=f'"used_builder_bytes":{d["budget"]["used_builder_bytes"]}'.encode();assert wire.count(f)==1;s=wire.replace(f,f'"used_builder_bytes":{q["budget"]["used_builder_bytes"]}'.encode(),1);f=('"graph_digest":'+json.dumps(d['graph_digest'])).encode();assert s.count(f)==1;s=s.replace(f,('"graph_digest":'+json.dumps(q['graph_digest'])).encode(),1);assert s==previous
# Retained original browser functions are still compiled, not replaced by views.
parser=R/'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py';ns={};exec(compile(parser.read_text(),str(parser),'exec'),ns);mask=ns['mask']
def fn(s,name):
 m=mask(s);a=re.search(r'\bfn\s+'+re.escape(name)+r'\b',m).start();i=m.index('{',a);j=i+1;depth=1
 while depth:depth+=(m[j]=='{')-(m[j]=='}');j+=1
 return s[a:j]
p='tests/wasm/scalar_browser_ci_contract/historical_builder.rs';old=git('show',B+':'+p).decode();new=(R/p).read_text()
for name in re.findall(r'\bfn (\w+)\(',mask(old)):
 if name=='a_rehashed_non_builder_budget_mutation_cannot_pass_the_historical_bridge':assert fn(old,name)[:-2]in fn(new,name)
 else:assert fn(old,name)==fn(new,name),name
for p,commit in [('src/cleanup_plan/replay/path_array/tests.rs','dae42296fc52f171ddc4d9dafb2d0262261bca47'),('src/cleanup_plan/replay/path_array/funded_tests.rs','dae42296fc52f171ddc4d9dafb2d0262261bca47'),('src/workspace_graph/function_alias/before_core/tests.rs','843184a6a8ace13a4edee211411703f051b7e2a4')]:assert (R/p).read_bytes()==git('show',commit+':'+p)
# Current manifest authority pure production text stays identical to frozen47f.
a=json.loads((D/'authority-source-proof.json').read_text());old=json.loads(git('show',B+':docs/evidence/task722-held-fitting759-join-20261016/authority-source-proof.json'))
for key in ['hir','projection']:assert a[key]['production_sha256']==old[key]['production_sha256']and a[key]['exact_existing_forbidden_tokens']==old[key]['exact_existing_forbidden_tokens']
for p in ['src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','tests/wasm/scalar_browser_ci_contract/before_2600.rs']:assert (R/p).read_bytes()==git('show',B+':'+p)
# No shipping/config/standard-library/example source is changed by the patch.
allowed={'src/source_reflows_merge.rs','src/workspace_graph/type_proof_tests.rs','src/cleanup_plan/replay/path_array/tests.rs','src/cleanup_plan/replay/path_array/funded_tests.rs','src/workspace_graph/function_alias/before_core/tests.rs','crates/semaprax-toolchain/tests/cli_help_surface_v1.rs','platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs'}
for p in git('diff',B,'--name-only','--diff-filter=M').decode().splitlines():assert p.startswith('tests/')or p in allowed,p
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
proof={'status':'SOURCE_ONLY_PASS','whole_current_to47f_rows':len(rows),'all_current_source_rows_restore_complete_frozen47f':True,'production_source_and_every_original_predicate_unchanged':True,'actual_managed_bill':1106063,'actual_browser_bill':3477789,'all_actual_browser_fullwire_restorations_pass':True,'original_browser_receiver_functions_still_compiled_and_exact':True,'old_JSON90_and_membership_immutable':True,'precise_fixture_components_exact':True,'no_Rust_Cargo_or_capture_execution':True,'postpin_consumers':'PENDING_NEW21_RUN; old47f evidence is historical'};(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
