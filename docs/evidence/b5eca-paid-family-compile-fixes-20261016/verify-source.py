#!/usr/bin/env python3
"""Three exact source corrections, no Rust execution or qualification claim."""
from pathlib import Path
import hashlib,json,subprocess
root=Path(__file__).resolve().parents[3];packet=Path(__file__).resolve().parent
x=json.loads((packet/'source-inverses.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
assert len(x['rows'])==3
for row in x['rows']:
 old=(packet/row['before_archive']).read_bytes();current=(root/row['source']).read_bytes()
 assert old==subprocess.check_output(['git','show',x['base']+':'+row['source']],cwd=root)
 assert sha(old)==row['before_sha256'];assert sha(current)==row['after_sha256']
 a=old.decode();b=current.decode()
 for edit in reversed(row['edits']):
  i=edit['old_offset'];assert a[i:i+len(edit['before'])]==edit['before'];a=a[:i]+edit['after']+a[i+len(edit['before']):]
  i=edit['new_offset'];assert b[i:i+len(edit['after'])]==edit['after'];b=b[:i]+edit['before']+b[i+len(edit['after']):]
 assert a.encode()==current and b.encode()==old
failure=json.loads((packet/'actual-failure.json').read_text());log=(packet/'actual-clippy-production.log').read_bytes();assert sha(log)==failure['log_sha256'];assert sum(failure['diagnostics'].values())==12
assert 'pub struct CleanupSlot' in (root/'src/cleanup_plan.rs').read_text() or 'pub struct CleanupSlot' in (root/'src/cleanup_plan/storage.rs').read_text()
leaves=(root/'src/cleanup_plan/replay/flat_records/paid_leaves.rs').read_text();assert 'slot: &crate::cleanup_plan::CleanupSlot,' in leaves
boundary=(root/'src/cleanup_plan/replay/lexical_boundary/paid.rs').read_text();assert 'let site:&crate::hir::ExpressionId=$site;' in boundary
original=(root/'src/cleanup_plan/replay/lexical_boundary.rs').read_text();assert 'let mut admit = |site: &ExpressionId,' in original
maps=(root/'src/cleanup_plan/replay/structural_maps/paid.rs').read_text();assert '#[cfg(test)]\npub(in crate::cleanup_plan::replay) use index;' in maps
assert 'structural_maps::paid::index!' in (root/'src/cleanup_plan/replay/flat_records/tests/receiving_families.rs').read_text()
# Banks, producer bodies, test assertions and all original evidence inputs are
# identical after exactly reversing the three complete-file source frames.
print(json.dumps({'status':'PASS_SOURCE_ONLY','complete_source_rows':3,'actual_prior_diagnostics':12,'rust_executed':False},sort_keys=True))
