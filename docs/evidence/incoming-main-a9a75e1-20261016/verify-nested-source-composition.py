#!/usr/bin/env python3
"""Complete immutable source/custody composition only; never executes Rust."""
from pathlib import Path
import argparse,collections,hashlib,json,re,subprocess
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);a=p.parse_args();r=a.repo.resolve();assert r.is_relative_to('/Volumes/Semaprax');d=r/'docs/evidence/incoming-main-a9a75e1-20261016';path='src/hir/validation/byte_traversal/nested/tests.rs';helper='src/hir/validation/byte_traversal/nested/tests/source_composition.rs'
def sha(s):return hashlib.sha256(s.encode()).hexdigest()
def git(ref,path):return subprocess.check_output(['git','show',ref+':'+path],cwd=r).decode()
j=json.loads((d/'nested-source-composition.json').read_text());joined=(r/path).read_text();assert sha(joined)==j['joined_sha256'];assert sha((r/helper).read_text())==j['helper_sha256']
assert not any(marker in joined for marker in ['<<<<<<<','=======','>>>>>>>'])
parents=json.loads((d/'complete-inputs.json').read_text());row=next(x for x in parents['rows'] if x['source']==path);old={}
for role in ['ours','incoming','base']:
 v=row[role];old[role]=(d/v['archive']).read_text();assert old[role]==git(v['ref'],path) and sha(old[role])==v['sha256']
def restore(s,row):
 assert sha(s)==row['after_sha256'];b=s.encode();prior=len(b)
 for e in row['inverse']:
  lo,hi=e['start'],e['end'];assert 0<=lo<=hi<=prior;assert hashlib.sha256(b[lo:hi]).hexdigest()==e['after_sha256'];b=b[:lo]+e['before'].encode()+b[hi:];prior=lo
 s=b.decode();assert sha(s)==row['before_sha256'];return s
assert restore(joined,j['joined_to_incoming'])==old['incoming'];assert restore(joined,j['joined_to_ours'])==old['ours']
ci_path='docs/evidence/nested-byte-traversal-780/ci-scope-original.json';archive='docs/evidence/nested-byte-traversal-780/ci-scope-tests.before.txt'
assert (r/ci_path).read_text()==git(row['incoming']['ref'],ci_path);assert (r/archive).read_text()==git(row['incoming']['ref'],archive)
ci=json.loads((r/ci_path).read_text());source=old['incoming']
for edit in reversed(ci['edits']):assert source.count(edit['new'])==1;source=source.replace(edit['new'],edit['old'],1)
assert source==(r/archive).read_text() and sha(source)==ci['sha256']
packet='docs/evidence/nested-byte-actual-fixture-correction-940/source-inverses.json';assert (r/packet).read_text()==git(row['ours']['ref'],packet)
record=next(x for x in json.loads((r/packet).read_text()) if x['source']==path);ours940=restore(old['ours'],j['ours_to_940']);assert sha(ours940)==record['current_sha256']
source=ours940
for edit in record['edits']:assert source.count(edit['new'])==1;source=source.replace(edit['new'],edit['before'],1)
assert source==(r/record['archive']).read_text() and source==git(row['ours']['ref'],record['archive']) and sha(source)==record['original_sha256']
# Trusted exact original source-only parser supplies lexical masking only.
ns={};exec(git(row['ours']['ref'],'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py').split('# Source only:')[0],ns);mask=ns['mask']
def body(s,name):
 m=mask(s);match=re.search(r'\bfn\s+'+re.escape(name)+r'\s*\(',m);assert match;lo=m.index('{',match.end());level=1;end=lo+1
 while level:level+=(m[end]=='{')-(m[end]=='}');end+=1
 return s[lo:end]
def predicates(s):
 m=mask(s);out=[]
 for match in re.finditer(r'\bassert(?:_eq|_ne)?!\s*\(',m):
  level=1;end=match.end()
  while level:level+=(m[end]=='(')-(m[end]==')');end+=1
  out.append(re.sub(r'\s+','',s[match.start():end]).replace('before_metrics','before'))
 return collections.Counter(out)
ancestor='nested_byte_ancestor_freezes_cover_every_inner_subtree_and_same_root_after_inner_exit';scope='nested_byte_census_uses_exact_owner_and_first_refusal_on_foreign_then_restored_scope'
for role in ['ours','incoming']:
 for name in [ancestor,scope]:
  before=body(old[role],name);current=body(joined,name)
  if name==ancestor:current+=body(joined,'same_root_loan_endpoints')
  assert not predicates(before)-predicates(current),(role,name,'original assertion lost')
# The incoming original guard changes only its source input binding. Every
# original reverse-loop, full-source equality and hostile predicate is exact.
name='nested_scope_correction_retains_complete_original_hostile_and_loan_controls';before=body(old['incoming'],name);after=body(joined,name)
new='''        let raw_current =
            std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
        let current = super::source_composition::incoming_current(&raw_current)
            .expect("complete actual joined source and both immutable parent inputs");'''
original='''        let current = std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();'''
assert after.count(new)==1 and after.replace(new,original)==before
for fixture in ['const SOURCE:','const ROOTED:']:
 for role in ['ours','incoming']:
  s=old[role];start=s.index(fixture);end=s.index(';\n',start)+2;assert s[start:end] in joined
# Both incoming independent calibration/storage controls stay entirely exact.
for name in ['empty_nested_budget_metrics','nested_empty_budget_scope_records_only_its_real_ledger_allocation','nested_storage_foreign_restore_has_no_allocation_beyond_test_scope']:
 assert body(joined,name)==body(old['incoming'],name),name
negative=['after_inner.live_after.contains','assert_eq!(metrics, scope_metrics);']
for oldtext in negative:
 assert oldtext in joined
 changed=joined.replace(oldtext,oldtext+' ',1);assert sha(changed)!=j['joined_sha256']
for changed in [old['ours'],old['incoming'],joined+'\n',joined[:-1]]:assert sha(changed)!=j['joined_sha256']
assert len(joined.splitlines())<1500 and len((r/helper).read_text().splitlines())<1500
print(json.dumps(dict(status='SOURCE_ONLY_JOINED_NESTED_BOTH_PARENT_COMPOSITION_PASS',joined_sha256=sha(joined),helper_sha256=j['helper_sha256'],all_three_arm_preimages_exact=True,both_original_recipes_and_archives_immutable=True,all_original_assertions_retained=True,incoming_complete_guard_body_predicates_exact=True,independent_calibration_and_storage_controls_exact=True,all_complete_inverse_replays=True,raw_drift_negative_source_checks=True,rust_executed=False)))
