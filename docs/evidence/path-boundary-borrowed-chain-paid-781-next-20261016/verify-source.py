#!/usr/bin/env python3
"""Finite complete-input source proof only. Does not execute Rust."""
import argparse,hashlib,json,re,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path.cwd());a=p.parse_args();root=a.repo.resolve()
packet=root/'docs/evidence/path-boundary-borrowed-chain-paid-781-next-20261016';data=json.loads((packet/'source-inverses.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
def old(path):return subprocess.check_output(['git','show',data['base']+':'+path],cwd=root)
for row in data['rows']:
 before=(packet/row['before_archive']).read_bytes();current=(root/row['source']).read_bytes()
 assert before==old(row['source']);assert sha(before)==row['before_sha256'];assert sha(current)==row['after_sha256']
 forward=before.decode();reverse=current.decode()
 for e in reversed(row['edits']):
  i=e['old_offset'];assert forward[i:i+len(e['before'])]==e['before'];forward=forward[:i]+e['after']+forward[i+len(e['before']):]
  i=e['new_offset'];assert reverse[i:i+len(e['after'])]==e['after'];reverse=reverse[:i]+e['before']+reverse[i+len(e['after']):]
 assert forward.encode()==current and reverse.encode()==before
for row in data['new_sources']:
 b=(root/row['source']).read_bytes();assert sha(b)==row['sha256'];assert len(b.decode().splitlines())==row['lines']<=1500
path='src/cleanup_plan/replay/path_join.rs';before=old(path).decode()
expected=re.sub(r'(?m)(?:#\[[^\n]*\]\n)*((?:pub\([^\n]*?\) )?mod [A-Za-z_]+;)\n','',before)
expected=expected.replace('pub(super) fn validate_path_states','pub(in crate::cleanup_plan::replay) fn validate_path_states',1)
assert (root/'src/cleanup_plan/replay/path_join/before_path_boundary.rs').read_text()==expected
# All original algorithm text survives in the Diagnostic policy body. Only its
# module path qualification and indentation differ; literal bytes match too.
boundary='src/cleanup_plan/replay/lexical_boundary.rs';raw=old(boundary).decode();a=raw.index('pub(super) fn check(');a=raw.index('{',a)+1;b=raw.index('\n}',a);body=raw[a:b]
policy=(root/'src/cleanup_plan/replay/lexical_boundary/legacy_policy.rs').read_text();a=policy.index(') -> Result<(), Diagnostic> {')+len(') -> Result<(), Diagnostic> {');policy=policy[a:policy.rfind('\n    }')]
policy=policy.replace('crate::cleanup_plan::replay::lexical_cert::storage','super::lexical_cert::storage')
# rustfmt wrapped precisely this old single-expression closure after moving
# it one indentation level; restore that unique lexical adapter for comparison.
normalized=re.sub(r'\s+','',policy).replace('ok_or_else(||{replay_error(function,"executionboundaryvarianthasnoregion")})','ok_or_else(||replay_error(function,"executionboundaryvarianthasnoregion"))')
assert re.sub(r'\s+','',body)==normalized
assert re.findall(r'"(?:[^"\\]|\\.)*"',body)==re.findall(r'"(?:[^"\\]|\\.)*"',policy)
# Original public legacy checker and entire storage body are unmodified.
current=(root/boundary).read_text();assert raw.replace('#[allow(clippy::too_many_arguments)]\npub(super) fn check','#[cfg(test)]\n#[allow(clippy::too_many_arguments)]\npub(super) fn check') in current
storage='src/cleanup_plan/replay/lexical_cert/storage.rs';assert (root/storage).read_bytes()==old(storage)
labels=(root/'src/cleanup_plan/renewal_profile/path_boundary_refusal.rs').read_text();texts=re.findall(r'"([^"\n]+)"',labels)
assert len(texts)==11
for text in texts:assert text in raw+old(storage).decode()
join=(root/path).read_text();assert join.count('<E as lexical_boundary::Policy>::check(')==2
oldcall=join.replace('<E as lexical_boundary::Policy>::check(','lexical_boundary::check(');assert oldcall.startswith(before)
chains=(root/'src/cleanup_plan/replay/lexical_cert/paid/chains.rs').read_text();assert 'Permit::' not in chains and 'reserve_' not in chains and 'budget:' not in chains
assert 'checked_sub(1)' in chains and "None => break 'chain false" in chains
assert (root/'src/cleanup_plan/replay/selected_context/child.rs').read_bytes()==old('src/cleanup_plan/replay/selected_context/child.rs')
# No preceding protected proof or fixture is changed.
assert not subprocess.check_output(['git','diff',data['base'],'--','docs/evidence/structural-leaf-paid-receiving-781-next-20261016','docs/evidence/status-scalar-paid-receiving-781-next-20261016','docs/evidence/certificate-precision-paid-receiving-781-next-20261016','docs/evidence/boundary-chain-paid-receiving-781-next-20261016','docs/evidence/lexical-type-drop-paid-receiving-781-next-20261016'],cwd=root)
caps={}
for line in (root/'tests/module-size-budget.tsv').read_text().splitlines():
 cells=line.split('\t')
 if len(cells)>1 and cells[1].isdigit():caps[cells[0]]=int(cells[1])
for row in data['rows']:
 if row['source'].endswith('.rs'):assert len((root/row['source']).read_text().splitlines())<=caps.get(row['source'],1500)
assert len(json.loads((packet/'pending-selectors.json').read_text()))==3
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_before_inverse_rows':len(data['rows']),'new_sources':len(data['new_sources']),'complete_old_path_and_diagnostic_policy':True,'closed_labels':11,'rust_executed':False,'issue_781':'OPEN_RESIDUAL_DESCENDANTS'},sort_keys=True))
