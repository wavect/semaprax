"""Finite source-only observer successor; no compiler/test/producers."""
from pathlib import Path
import hashlib,json,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;H='7a87b33115c0cd45b14cac64d7b37e5742503159';sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-7a87.json').read_text())
def apply(table,path,raw):
 matches=[r for r in table if r['source']==path];assert len(matches)<=1
 if not matches:return raw
 row=matches[0];assert sha(raw)==row['current_sha256'],path
 for e in row['edits']:
  new=e['new'].encode();assert raw.count(new)==1;raw=raw.replace(new,e['old'].encode(),1)
 assert sha(raw)==row['original_sha256'];return raw
neg=0
for row in rows:
 path=row['source'];raw=(R/path).read_bytes();before=apply(rows,path,raw)
 assert before==(R/row['archive']).read_bytes()==subprocess.check_output(['git','show',H+':'+path])
 for bad in [raw+b' ',b' '+raw]:
  try:apply(rows,path,bad)
  except AssertionError:neg+=1
  else:raise AssertionError('changed complete source accepted')
cache='src/workspace_graph/identity_resolution/base/cache_billing.rs';old=apply(rows,cache,(R/cache).read_bytes()).decode();new=(R/cache).read_text();hook='    #[cfg(test)]\n    observe_acquired(&owner);\n';assert new.count(hook)==1 and new.replace(hook,'',1)==old
ordinary=(R/'src/workspace_graph/identity_resolution/base.rs').read_text();assert hook in ordinary
# Every existing executable assertion owner stays complete and byte-exact.
unchanged=['src/workspace_graph/identity_resolution/base/cache_billing/tests.rs','src/workspace_graph/identity_resolution/base/cache_billing/source_reference.rs','src/workspace_graph/identity_resolution/base/affinity_tests.rs','src/workspace_graph/expected_projection/import_stub/core_proofs.rs','src/hir/prelude_bundle/invocation.rs']
for p in unchanged:assert (R/p).read_bytes()==subprocess.check_output(['git','show',H+':'+p])
helper='src/source_reflows_merge.rs';s=(R/helper).read_text();prior=apply(rows,helper,s.encode()).decode();prefix='    let observed = before_cached_acquisition_observer(source, current);\n    let current = observed.as_str();\n';assert s.count(prefix)==1
start='fn before_direct_carriers831(source: &str, current: &str) -> String {\n';assert start+prefix in s
newfn=s[s.index('\n// Test-only observation successor;'):];assert s.replace(prefix,'',1).replace(newfn,'',1)==prior
# Execute the unchanged831 complete-input checks only after the new full domain.
legacy=json.loads((R/'docs/evidence/direct-canonical-carriers-831-20261015/current-to-297.json').read_text());legacy_rows=0
for row in rows:
 p=row['source'];before=apply(rows,p,(R/p).read_bytes());oldrows=[x for x in legacy if x['source']==p]
 if oldrows:
  assert len(oldrows)==1;restored=apply(legacy,p,before);assert restored==(R/oldrows[0]['archive']).read_bytes();legacy_rows+=1
assert all((R/p).read_bytes()==subprocess.check_output(['git','show',H+':'+p]) for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs'])
assert max(len((R/p).read_text().splitlines()) for p in [cache,helper])<=1500
# Old evidence files are not touched; this packet contains additions only.
changes=subprocess.check_output(['git','diff','--name-status',H,'--','docs/evidence'],text=True).splitlines()
assert all(line.startswith('A\tdocs/evidence/cached-prelude-acquisition-observer-7a87/') for line in changes),changes
result={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','base':H,'complete_source_rows':len(rows),'tamper_refusals':neg,'old831_rows_replayed':legacy_rows,'unchanged_full_assertion_and_authority_owners':unchanged,'production_cache_body_byte_exact_after_cfg_hook_removal':True,'all_old_audit_inventory_bytes_unchanged':True,'new_test_bodies':0,'actual_failed_log_sha256':sha((D/'actual-7a87-failure.log').read_bytes()),'runtime_claim':False}
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
