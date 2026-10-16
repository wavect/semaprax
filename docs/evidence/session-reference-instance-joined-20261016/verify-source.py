from pathlib import Path
import subprocess,json,hashlib,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='17b12c909916ce8671b43f208b59f3432144709a';F='eefb5f967b1a98ec12261b68b29f328c2d940ff5';V='b92f8b120618172fc1f3bcffc613091df8b60161'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def restore(row,raw):
 assert sha(raw)==row['current_sha256'],row['source'];text=raw.decode()
 for edit in row['edits']:
  assert text.count(edit['new'])==1,row['source'];text=text.replace(edit['new'],edit['old'],1)
 assert sha(text.encode())==row['original_sha256'],row['source'];return text.encode()
tables={};counts={}
for label,rev in [('17b',B),('reference',F),('853',V)]:
 rows=json.loads((D/('current-to-'+label+'.json')).read_text());tables[label]={x['source']:x for x in rows};counts[label]=len(rows)
 assert set(tables[label])==set(git('diff',rev,'--name-only','--diff-filter=M').decode().splitlines())
 assert not git('diff',rev,'--name-only','--diff-filter=D').strip()
 for row in rows:
  p=row['source'];result=restore(row,(R/p).read_bytes())
  assert result==git('show',rev+':'+p)==(R/row['archive']).read_bytes()
def view(label,p):
 raw=(R/p).read_bytes();return restore(tables[label][p],raw)if p in tables[label]else raw
# Reuse old finite proofs only after their entire original namespace is Git-exact.
component_rows={}
for name,label,rev in [('reference-coverage-paid-receiving-781-next-20261016','reference',F),('streaming-instance-comparison-853-next-20261016','853',V)]:
 packet=R/'docs/evidence'/name;rows=json.loads((packet/'source-inverses.json').read_text())
 if isinstance(rows,dict):rows=rows.get('rows',rows.get('inputs'))
 for row in rows:
  # The853 packet has its own compact field vocabulary; fullnamespace above
  # already authenticates all old files independently against its exact Git.
  p=row.get('source',row.get('path'))
  if 'current_sha256' in row:assert sha(view(label,p))==row['current_sha256'],p
 component_rows[name]=len(rows)
# No oldJSON, fixed90fixture bytes, or loader inventory/membership changed.
changed=git('diff',B,'--name-only','--diff-filter=M').decode().splitlines()
assert not [p for p in changed if p.endswith('.json')]
for p in ['src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs']:
 assert (R/p).read_bytes()==git('show',B+':'+p)
new=json.loads((D/'new-owners.json').read_text());actual={p for p in git('diff',B,'--name-only','--diff-filter=A').decode().splitlines()if p.endswith('.rs')and p.split('/')[0]in ['src','tests','crates']};assert actual=={x['source']for x in new}
for row in new:assert sha((R/row['source']).read_bytes())==row['sha256']and row['lines']<=1500
helper=(R/'src/source_reflows_merge.rs').read_text();old=git('show',B+':src/source_reflows_merge.rs').decode()
def project(s):a=s.index('fn project(');return s[a:s.index('fn before_build_fix',a)]
assert project(helper)==project(old)
for f in ['main_b80_input','main_b80_prior_input']:
 assert ('fn '+f+'(source: &str, current: &str) -> String {\n    let latest = session_reference_input(source, current);')in helper
assert 'fn reference781_input(source: &str, current: &str) -> String {\n    let latest = reference_family_input(source, current);'in helper
for p in ['src/cleanup_plan/replay/selected_entry/paid.rs','src/cleanup_plan/replay/physical_budget.rs','src/cleanup_plan/replay/semantic_refusal.rs']:
 assert (R/p).read_bytes()==git('show',B+':'+p)
# New runtime transport is only these two compact outer tables; the853 view is metadata-only.
assert 'session-reference-instance-joined-20261016/current-to-853.json'not in helper
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
proof={'status':'SOURCE_ONLY_PASS_NO_RUST_EXECUTION','whole_namespace_inverse_rows':counts,'component_whole_namespace_proof_reuse':component_rows,'old_JSON_and_90runtime_fixtures_unchanged':True,'project_body_exact':True,'new_source_owners':len(new),'current_report_setup_and_receipt_authorities_byteexact':True,'whole781':'OPEN_remaining_dynamic_producers','execution':'NOT_EXECUTED'}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
