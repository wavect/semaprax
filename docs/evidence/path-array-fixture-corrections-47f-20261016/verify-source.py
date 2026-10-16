#!/usr/bin/env python3
"""Finite fixture correction custody. Does not execute Rust or qualify source."""
import argparse,hashlib,json,pathlib,re,subprocess
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);r=pathlib.Path(ap.parse_args().repo).resolve();base='47f80b0f7a06fde9b2388439a9c0bfe998a05af1';prefix='docs/evidence/path-array-fixture-corrections-47f-20261016/';p=r/prefix;sha=lambda b:hashlib.sha256(b).hexdigest();rows=json.loads((p/'current-to-47f.json').read_text());assert len(rows)==2
olds={};currents={}
for row in rows:
 s=row['source'];current=(r/s).read_text();assert sha(current.encode())==row['current_sha256'];old=current
 for e in row['edits']:assert old.count(e['new'])==1;old=old.replace(e['new'],e['old'],1)
 assert old.encode()==subprocess.check_output(['git','show',base+':'+s],cwd=r)==(r/row['original_archive']).read_bytes();assert sha(old.encode())==row['original_sha256'];olds[s]=old;currents[s]=current;assert len(current.splitlines())<=1500
 source=currents[s];a=source.index('                        // The owning HIR enumerator');b=source.index('                        let before = skeleton_materializations();',a);bank=source[a:b]
 assert bank.count('reserve_program_skeleton_work(')==2
 assert bank.count('std::iter::once(function)')==2
 assert '&mut old,' in bank and '&mut current,' in bank
 source=source[:a]+source[b:]
 if s.endswith('/tests.rs'):assert source==old,'only exact shared old/current preflight added'
 else:
  # Whole second control, including first-output zero-work negative, is exact.
  marker='#[test]\nfn funded_capacity_exact_short_zero_and_first_output_work_frontier()'
  assert source[source.index(marker):]==old[old.index(marker):]
  # Whole original positive owner body exact after removing its common setup.
  marker='#[test]\nfn funded_capacity_preserves_complete_main_hir_work_and_drop()'
  assert source[source.index(marker):]==old[old.index(marker):]
  old_literal=re.search(r'crate::check\(("(?:\\.|[^"\\])*")',old).group(1)
  assert 'const ORIGINAL_SOURCE: &str = '+old_literal+';' in source
  assert 'crate::check(ORIGINAL_SOURCE, "funded-arrays.spx").unwrap_err()' in source
  for field in ['code: "SPX-T105"','severity: crate::diagnostic::Severity::Error','message: "executable module must define `fn main() -> i64`"','path: Some("funded-arrays.spx".to_owned())','start: 31, end: 167, line: 1, column: 32','help: Some("a module without `fn main() -> i64`']:
   assert field in source
  assert '{ORIGINAL_SOURCE} @id(\\"main\\") fn main()->i64 {{f(true,false,true,false)}}' in source
for f in json.loads((p/'actual-failures.json').read_text()):assert sha((r/f['archive']).read_bytes())==f['sha256']
selectors=json.loads((p/'pending-selectors.json').read_text());assert len(selectors)==3 and all(not s['new'] for s in selectors)
changed=subprocess.check_output(['git','diff','--name-only',base],cwd=r,text=True).splitlines();allowed=set(olds);assert all(s in allowed or s.startswith(prefix) for s in changed),changed
print(json.dumps({'status':'PASS_SOURCE_ONLY','base':base,'whole_current_to_git_inverse':2,'production_changes':0,'new_test_bodies':0,'existing_selectors':selectors,'original_second_funded_owner_byte_exact':True,'original_first_owner_assertions_byte_exact':True,'actual_failed_logs_retained':[2,30,31],'rust_execution':False,'qualification_claim':False,'inherited_global_views':'PENDING_CANONICAL_COMPOSITION'},indent=2))
