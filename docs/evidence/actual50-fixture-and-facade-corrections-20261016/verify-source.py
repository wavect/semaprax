from pathlib import Path
import json,hashlib,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='1830464c78a18b5eaf29e4684e0c891f7e9894a7'
sha=lambda x:hashlib.sha256(x).hexdigest()
def git(*x):return subprocess.check_output(['git',*x],cwd=R)
def restore(row,raw):
 assert sha(raw)==row['current_sha256'];s=raw.decode()
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==row['original_sha256'];return s.encode()
rows=json.loads((D/'current-to-183.json').read_text());assert rows==json.loads((D/'source-inverses.json').read_text())and len(rows)==5
for row in rows:assert restore(row,(R/row['source']).read_bytes())==git('show',B+':'+row['source'])==(R/row['archive']).read_bytes()
changed=git('diff',B,'--name-only','--diff-filter=M').decode().splitlines();assert set(changed)=={r['source']for r in rows};assert not [p for p in changed if p.startswith('docs/evidence/')]
# Original test body/assertions remain exact once legitimate caller setup and
# the added consumed-work assertion are removed.
p='src/cleanup_plan/replay/branch_clone/tests.rs';s=(R/p).read_text();old=git('show',B+':'+p).decode()
for text in ['                reserve_program_skeleton_work(&program, std::iter::once(function), &mut old).unwrap();\n','                reserve_program_skeleton_work(&program, std::iter::once(function), &mut current).unwrap();\n','                            // One actual clone and exactly this many pushes; lend\n                            // their work from the unchanged global caller budget.\n                            budget.reserve_skeleton(function, 1 + pushes)?;\n','                            assert_eq!(budget.skeleton_remaining, 0);\n']:
 assert s.count(text)==1;s=s.replace(text,'',1)
assert s==old
# Funding uses the unchanged real preflight: no fixture cap mutation, artificial
# baseline padding, oldcallee redirection, or skipped observed acquisition.
root=(R/'src/cleanup_plan/replay.rs').read_text();assert 'budget.reserve_skeleton(function, total)?;' in root and 'const MAX_REPLAY_WORK_UNITS: usize = 32_000_000;'in root
assert 'self.remaining = self.remaining.checked_sub(units)'in root
assert (R/'src/cleanup_plan/replay/branch_clone/before.rs').read_bytes()==git('show',B+':src/cleanup_plan/replay/branch_clone/before.rs')
# The actual complete facade union and each source/ separator are authenticated.
parts=[]
for item in json.loads((D/'facade-complete-inputs.json').read_text()):
 if 'literal'in item:parts.append(item['literal'])
 else:
  b=(R/item['source']).read_bytes();assert sha(b)==item['sha256'];parts.append(b.decode())
raw=''.join(parts).encode();assert raw==(D/'current-facade.before.rs.txt').read_bytes();row=json.loads((D/'current-facade-input.json').read_text());prior=restore(row,raw);assert prior==(D/'previous-facade.rs.txt').read_bytes()
name=row['current_decl_source'];current=(R/name).read_bytes();assert sha(current)==row['current_decl_sha256'] and current.decode()==row['edits'][0]['new'];olddecl=git('show',row['original_decl_revision']+':'+name);assert olddecl==(D/'pre847-decl-slices.rs.txt').read_bytes()==row['edits'][0]['old'].encode();assert sha(olddecl)==row['original_decl_sha256'];assert current.decode().count(row['actual_projected_method'])==1
# Preserve BOTH entire original expected-list blocks. The new input for the
# second list equals it exactly while raw current purity still uses actual data.
p='tests/workspace/semantic_graph.rs';s=(R/p).read_text();old=git('show',B+':'+p).decode();a=s.index('let exact_facade = [');z=s.index('    ];',a);oa=old.index('let exact_facade = [');oz=old.index('    ];',oa);assert s[a:z]==old[oa:oz]
a=s.index('    let mut current_facade =');z=s.index('    assert_eq!(',a);oa=old.index('    let mut current_facade =');oz=old.index('    assert_eq!(',oa);assert s[a:z].replace('    let current_facade_input = current_projection_facade_input(current_projection_root);\n','')==old[oa:oz]
expected=[json.loads('"'+v+'"')for v in re.findall(r'^\s*"((?:\\.|[^"\\])*)",?\s*$',s[s.index('let exact_facade = ['):s.index('    ];',s.index('let exact_facade = ['))],re.M)];at=expected.index('pub(super) fn materialize_receiving(')+1;expected[at:at]=['pub(super) fn pay_type(declaration: &TypeDeclaration, owner: &mut Receiver) -> Result<(), Setup> {','pub(super) fn pay_signature(function: &Function, owner: &mut Receiver) -> Result<(), Setup> {','pub(super) fn teardown_type(','pub(super) fn teardown_function(','pub(super) fn pay_default_teardown(','pub(super) fn pay_teardown_minimum(owner: &mut Receiver) -> Result<(), Setup> {'];expected.append('pub(super) fn verify(')
assert [x.strip()for x in prior.decode().splitlines()if 'pub(super)'in x]==expected
assert 'let production_projection = production_source_inventory::production('in s and 'assert_eq!(actual.matches(method).count(), 1' in s
# The four blank canonical manifest separators are the separately reviewed fix.
p='src/workspace_graph/function_alias/before_core/tests.rs';assert (R/p).read_bytes()==git('show','e2200ac21a2c032f686ef2a6ab68bda26e60f608:'+p)
for p in ['src/source_reflows_merge/runtime_json.rs','src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','src/source_reflows_merge/runtime_json/tests.rs']:assert (R/p).read_bytes()==git('show',B+':'+p)
for row in rows:
 p=row['source']
 if p.endswith('.rs'):assert len((R/p).read_bytes().splitlines())<=1500,p
assert 'let fixed = actual50_fixture_input(source, current);'in (R/'src/source_reflows_merge.rs').read_text()
for row in json.loads((D/'actual-failure-bindings.json').read_text())['logs']:assert sha((R/row['archive']).read_bytes())==row['sha256']
proof={'status':'SOURCE_ONLY_PASS','predecessor':B,'whole_prior_inputs':5,'compact_source_input_bytes':(D/'current-to-183.json').stat().st_size,'full_facade_input_bytes':len(raw),'full_old_facade_input_bytes':len(prior),'facade_runtime_JSON_bytes':(D/'current-facade-input.json').stat().st_size,'unchanged_exact_current_facade_entries':len(expected),'old_assertions_preserved':True,'actual_caller_preflight_in_observation':True,'global_caps_unchanged':True,'production_unchanged':True,'old90_and_all_prior_JSONs_exact':True,'Rust_executed':False};(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
