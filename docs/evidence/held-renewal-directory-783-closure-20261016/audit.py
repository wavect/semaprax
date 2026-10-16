from pathlib import Path
import hashlib,json,subprocess,re
r=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang');p=Path(__file__).parent;sha=lambda b:hashlib.sha256(b).hexdigest();current='b3a53367d4def76240d14dca25ee414785bed9d6';base='8bb9f5b7b99e1e71a797460c46180daf716b234b'
def git(head,s):return subprocess.check_output(['git','show',head+':'+s],cwd=r)
paths=[]
for prefix in ['src/cleanup_plan/renewal_profile/ordinary','src/loop_calls/byte_writer/module_index','src/cleanup_plan/build/helper_index']:
 paths+=subprocess.check_output(['git','ls-tree','-r','--name-only',current,'--',prefix],cwd=r,text=True).splitlines()
paths+=['src/cleanup_plan/renewal_profile.rs','src/loop_calls/byte_writer/module_index.rs','src/cleanup_plan/build/helper_index.rs','src/cleanup_plan/replay/ordinary_renewals.rs','src/cleanup_plan/replay/phase_measurement.rs','src/cleanup_plan/replay/skeleton_work.rs']
cut=[];changed=[]
for s in paths:
 a=git(base,s);b=git(current,s)
 if a!=b:changed.append(s);continue
 cut.append({'source':s,'sha256':sha(b),'bytes':len(b)})
assert changed==['src/cleanup_plan/renewal_profile/ordinary/consuming/tests.rs'],changed
records=[]
root=r/'docs/evidence/byte-slice-traversal-780-closure-20261016/raw/receipts'
for f in ['f71f15b8e0043a65-owning-selected-results.json','7ad94b4a4c33b3ee-owning-selected-results.json']:
 records.append(root/f)
records += [r/'docs/evidence/qualification-8bb9-76-resume-20261016/raw/owning-selected-results.json',r/'docs/evidence/qualification-364d-60-20261016/raw/owning-selected-results.json',Path('/Volumes/Semaprax/lane-prompts/final-repair-rebind-171cece20-20261010/repair-fd2ac721b-final/owning-selected-results.json')]
rows=[]
for receipt in records:
 j=json.loads(receipt.read_text())
 for i,x in enumerate(j['results']):
  s=x.get('selector','')
  selected=(s.startswith('cleanup_plan::renewal_profile::ordinary::tests::') or s.startswith('cleanup_plan::build::helper_index::tests::') or s.startswith('cleanup_plan::renewal_profile::ordinary::record_presence::tests::') or s.endswith('::phase_presence_keeps_full_current_directories_and_every_consuming_entry') or s.endswith('::full_consuming_collector_matches_unchanged_original'))
  if not selected:continue
  if receipt.parent.name=='raw' and '364d' in str(receipt) and not s.endswith('::phase_presence_keeps_full_current_directories_and_every_consuming_entry'):continue
  log=receipt.parent/Path(x['log']).name
  if not log.exists():log=Path(x['log'])
  # The780 archive stores its full original log inventory under raw/logs.
  if not log.exists():
   found=list((r/'docs/evidence/byte-slice-traversal-780-closure-20261016/raw').rglob('*'+Path(x['log']).name));found=[f for f in found if sha(f.read_bytes())==x['log_sha256']];assert len(found)==1;(log,)=found
  assert sha(log.read_bytes())==x['log_sha256'];text=log.read_text();assert x['exit_code'] in [0,101]
  if x['exit_code']==0:
   summary=x.get('summary');assert 'test result: ok.' in text and '0 failed; 0 ignored;' in text
  rows.append({'receipt':str(receipt),'receipt_sha256':sha(receipt.read_bytes()),'subject':j['source_subject'],'row':i,'selector':s,'exit_code':x['exit_code'],'log':str(log),'log_sha256':x['log_sha256'],'summary':x.get('summary'),'discovered_names':x.get('discovered_names')})
# Exact unchanged source-pointer/stale and constructor short/zero test bodies
# from the actual seven-owner prefix subject, not an invented current test run.
def item(s,n):
 m=re.search(r'(?m)^fn '+n+r'\(',s);assert m,n;end=re.search(r'(?m)^}$',s[m.end():]);assert end;return s[m.start():m.end()+end.end()]
source='src/cleanup_plan/build/helper_index/tests.rs';oldhead='fd2ac721b6e2764414239429c1c2657152a69e8a';exact=[]
for n in ['construction_rejects_copied_function_foreign_program_and_reproves_changed_provider','construction_exact_one_short_and_zero_budget_leave_attached_plans_unchanged']:
 a=item(git(oldhead,source).decode(),n);b=item(git(current,source).decode(),n);assert a==b;exact.append({'source':source,'item':n,'sha256':sha(b.encode()),'before':oldhead})
result={'current':current,'byte_exact_from':base,'byte_exact_cut':cut,'changed_test_only':changed,'exact_historical_negative_items':exact,'actual_rows':rows,'pass_rows':sum(x['exit_code']==0 for x in rows),'fail_rows':sum(x['exit_code']!=0 for x in rows),'new_execution':False,'current_global_pass_claim':False,'publication_dependency':'fd2ac original prefix7 receipt/log/build receipt are physically present but not located in pushed docs; publish originals before closure if using this negative provenance'}
(p/'audit-binding.json').write_text(json.dumps(result,indent=2)+'\n');print(len(cut),'exactfiles',result['pass_rows'],'historicalPASS',result['fail_rows'],'retainedFAIL')
