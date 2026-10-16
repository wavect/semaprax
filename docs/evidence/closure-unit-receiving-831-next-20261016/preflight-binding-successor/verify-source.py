#!/usr/bin/env python3
"""Narrow source-only successor; old fac verifier/tables are immutable domains."""
import argparse,pathlib,json,hashlib,subprocess
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);a=ap.parse_args();r=pathlib.Path(a.repo).resolve();base='fac23f62fe1a931cf5f5c7ad9526534ed5cefad9';p=r/'docs/evidence/closure-unit-receiving-831-next-20261016';q=p/'preflight-binding-successor';sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((q/'current-to-fac.json').read_text());assert len(rows)==3
views={}
for row in rows:
 text=(r/row['source']).read_text();assert sha(text.encode())==row['current_sha256']
 for e in row['edits']:assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
 assert sha(text.encode())==row['original_sha256'];assert text.encode()==subprocess.check_output(['git','show',base+':'+row['source']],cwd=r);assert text==(r/row['original_archive']).read_text();views[row['source']]=text
# Compose immutable prior9 rows on the authenticated fac named input; no older
# verifier is falsely invoked against new physical source.
oldrows=json.loads((p/'current-to-2d4b.json').read_text());assert len(oldrows)==9
for row in oldrows:
 text=views.get(row['source'],(r/row['source']).read_text());assert sha(text.encode())==row['current_sha256']
 for e in row['edits']:assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
 assert sha(text.encode())==row['original_sha256'];assert text==(r/row['original_archive']).read_text();assert text.encode()==subprocess.check_output(['git','show','2d4b68d3a416cae5fba6ac08498cd56721878ba1:'+row['source']],cwd=r)
changed=subprocess.check_output(['git','diff','--name-only',base],cwd=r,text=True).splitlines();allowed={row['source'] for row in rows}
assert all(s in allowed or s.startswith('docs/evidence/closure-unit-receiving-831-next-20261016/preflight-binding-successor/') for s in changed),changed
for owner in json.loads((p/'new-owners.json').read_text()):assert sha((r/owner['source']).read_bytes())==owner['sha256']
root=(r/'src/cleanup_plan/replay/factored/canonical_shared.rs').read_text();memo=(r/'src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs').read_text()
assert 'closures.binding.is_none()' not in root
assert 'if $this.closures.preflight($function, $budget)' in root
assert 'fn preflight(&mut self, function: &ResolvedFunction, budget: &ReplayBudget) -> bool' in memo
assert root.count('if self.closures.preflight(function, budget)')==2
print(json.dumps({'status':'PASS_SOURCE_ONLY','three_whole_fac_inputs':len(rows),'composed_whole_2d4b_inputs':len(oldrows),'protected_fac_tables_and_test_owners':'UNCHANGED','extra_option_borrow':'ELIMINATED_DIRECT_BRANCH_PREDICATE','execution':False},indent=2))
