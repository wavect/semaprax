from pathlib import Path
import ast,hashlib,json,re,subprocess
D=Path(__file__).resolve().parent;R=D.parents[2];BASE='35cc6325790e00f7bcbd0c0bedf51518939f9b47';h=lambda s:hashlib.sha256(s.encode()).hexdigest()
rows=json.loads((D/'current-to-35cc.json').read_text());assert rows==json.loads((D/'source-inverses.json').read_text()) and len(rows)==2
for row in rows:
 current=(R/row['source']).read_text();assert h(current)==row['current_sha256']
 for e in row['edits']:assert current.count(e['new'])==1;current=current.replace(e['new'],e['old'],1)
 assert current==(R/row['archive']).read_text() and h(current)==row['original_sha256'];assert current.encode()==subprocess.check_output(['git','show',BASE+':'+row['source']],cwd=R)
# Execute faithful source::apply translation, both directions, for the actual failing fullroot.
def apply(source,patch,reverse=False):
 source=source.splitlines(True);out=[];consumed=0;inside=False
 for line in patch.splitlines(True):
  if line.startswith('@@ '):
   start=int(line.split()[2 if reverse else 1][1:].split(',')[0]);out+=source[consumed:max(consumed,start-1)];consumed=max(consumed,start-1);inside=True;continue
  if not inside:continue
  prefix=line[0];contents=line[1:];remove=prefix==('+' if reverse else '-');add=prefix==('-' if reverse else '+')
  if remove or prefix==' ':assert source[consumed]==contents;consumed+=1
  if add or prefix==' ':out.append(contents)
  assert remove or add or prefix==' '
 return ''.join(out+source[consumed:])
root='src/cleanup_plan/replay/factored/canonical_shared/ordered_weights.rs';physical=(R/root).read_text();beforeRoot=physical
capsule=json.loads((R/'docs/evidence/branch-closed-receiving-831-20261015/current-to-e93f.json').read_text());row=next(r for r in capsule if r['source']==root);assert h(physical)==row['current_sha256']
for edit in row['edits']:assert physical.count(edit['new'])==1;physical=physical.replace(edit['new'],edit['old'],1)
assert h(physical)==row['original_sha256'] and physical==(R/row['archive']).read_text()
# Fullroot data has no second held-auth mutation after assembled reaches e93.
held=json.loads((R/'docs/evidence/held-auth-839/source-inverses.json').read_text());assert not any(r.get('path')==root for r in held)
orig=(R/'docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared__ordered_weights.rs.before.txt').read_text();patch=(R/'docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared__ordered_weights.rs.patch.txt').read_text();assert apply(orig,patch)==physical and apply(physical,patch,True)==orig
log=(D/'actual-failed-owning-selected-77.log').read_text();left,right=log.split('  left: ',1)[1].split('\n right: ',1);assert ast.literal_eval(left)==physical and ast.literal_eval(right.split('\nnote:',1)[0])==beforeRoot
helper=(R/'src/source_reflows_merge.rs').read_text();auth=(R/'src/cleanup_plan/replay/physical_budget/authentication/source.rs').read_text();owner=(R/rows[0]['source']).read_text()
assert 'if path == "../ordered_weights.rs"' in owner and 'authentication::previous(' in owner
assert 'let staged=crate::source_reflows_merge::held(path,current);' in auth
assert 'let predecessor = assembled_previous(source, current);' in helper
assert 'let prior = project(\n        &runtime_json::load(runtime_json::Fixture::F76),' in helper
inventory=(R/'src/source_reflows_merge/runtime_json/inventory.rs').read_text();assert 'Self::F76 => Spec {\n                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/branch-closed-receiving-831-20261015/current-to-e93f.json")' in inventory
# All old guard expressions, immediatepatch tables, runtimefixtures and algorithms are byteexact.
changed=subprocess.check_output(['git','diff',BASE,'--diff-filter=M','--name-only'],cwd=R,text=True).splitlines();assert set(changed)=={row['source'] for row in rows}
for row in rows:assert len((R/row['source']).read_text().splitlines())<=1500
proof={'status':'SOURCE_ONLY_INVERSE_AND_ACTUAL_FAILED_FULLROOT_REPLAY_PASS_NO_RUST','full_current_to_35cc_inputs':2,'runtime_added_compiled_JSON_bytes':(D/'current-to-35cc.json').stat().st_size,'actual_failed_first_bind_fullforward_and_inverse':True,'full_expected_source_equal_original_actual_failure_left':True,'no_old_assertions_hashes_tables_or_production_changed':True,'other_six_bind_routes_unchanged':True,'new_test_selectors':0,'Rust_Cargo_execution':False};(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
