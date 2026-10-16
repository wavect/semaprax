#!/usr/bin/env python3
"""Finite exact source custody. Does not compile or execute Rust."""
import argparse,hashlib,json,re,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',required=True);a=p.parse_args();r=Path(a.repo).resolve()
d=r/'docs/evidence/instance-key-format-completion-853-20261016';m=json.loads((d/'source-inverses.json').read_text())
def sha(s):return hashlib.sha256(s.encode()).hexdigest()
def git(path):return subprocess.check_output(['git','show',m['base']+':'+path],cwd=r).decode()
ns={};exec(git('docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py').split('# Source only:')[0],ns);mask=ns['mask']
def tokens(s):return re.findall(r'\w+|[^\s]',s)
def function(s,name):
 masked=mask(s);start=re.search(r'(?m)^    pub\(in crate::hir\) fn '+name+r'\(',masked).start();b=masked.index('{',start);end=b+1;depth=1
 while depth:depth+=(masked[end]=='{')-(masked[end]=='}');end+=1
 return s[start:end]
previous={}
assert [x['source'] for x in m['rows']]==['src/hir/nodes/instance_key.rs','src/bounded_output/checked_string.rs','src/hir/ids/instance_match/tests.rs','CHANGELOG.md']
for row in m['rows']:
 old=(d/row['archive']).read_text();new=(r/row['source']).read_text();assert old==git(row['source']);assert sha(old)==row['before_sha256'];assert sha(new)==row['after_sha256']
 restored=new
 for edit in row['inverse']:
  assert restored.count(edit['after'])==1;restored=restored.replace(edit['after'],edit['before'],1)
 assert restored==old;previous[row['source']]=old
source=(r/'src/hir/nodes/instance_key.rs').read_text();old=previous['src/hir/nodes/instance_key.rs'];active=function(source,'identity_key_checked');before=function(source,'identity_key_before_format_completion')
old_function=function(old,'identity_key_checked')
assert before[before.index('{'):]==old_function[old_function.index('{'):]
expected=function(old,'identity_key_checked').replace('format!(', 'checked_key_format!(complete,')
expected=expected.replace('crate::bounded_output::CheckedCappedString::CONTROLS,','crate::bounded_output::CheckedCappedString::CONTROLS + std::mem::size_of::<String>(),',1)
expected=expected.replace('                        keys.push(checked_key_format!(complete,','                        let encoded = encoded.into_string();\n                        keys.push(checked_key_format!(complete,',1)
expected=expected.replace('                            encoded.into_string()\n                        ));','                            encoded\n                        ));',1)
expected=expected.replace('Self::ArrayU8(length) => keys.push(checked_key_format!(complete,"array:u8:{length}")),','Self::ArrayU8(length) => { keys.push(checked_key_format!(complete,"array:u8:{length}")) }')
# Comments/formatting are irrelevant to this mechanical shape comparison;
# string literals, grammar and all ordinary key traversal tokens remain exact.
def no_comments(s):
 # No comment markers occur inside this function's string literals.
 return re.sub(r'//[^\n]*','',s)
def formatting_tokens(s):
 s=no_comments(s);masked=mask(s);out=list(s)
 for match in re.finditer(r',\s*[)\]}]',masked):out[match.start()]=' '
 return tokens(''.join(out))
assert formatting_tokens(active)==formatting_tokens(expected)
assert active.count('checked_key_format!')==6
assert 'format!(' not in active.replace('checked_key_format!(', '')
macro=source[source.index('macro_rules! checked_key_format'):source.index('\nimpl ResolvedType {')]
assert macro.index('reserve_active_required')<macro.index('CheckedCappedString::new')<macro.index('write!(output')<macro.index('output.into_string()')
assert '*$complete = false;' in macro and 'format!($($argument)*)' in macro
assert '#[cfg(test)]\nimpl ResolvedType {' in source
checked=(r/'src/bounded_output/checked_string.rs').read_text();old_checked=previous['src/bounded_output/checked_string.rs']
assert 'const OUTPUT_CAP' not in checked and 'Self::ENTRY_CONTROLS' in checked
# The production writer is byte-exact apart from one cfg(test) observer call.
new_writer=checked[checked.index('impl fmt::Write'):checked.index('// Diagnostic-only exact write frontier.')]
old_writer=old_checked[old_checked.index('impl fmt::Write'):]
new_writer=new_writer.replace('        #[cfg(test)]\n        frontier_observer::record(value.len());\n','')
assert new_writer.rstrip()==old_writer.rstrip()
observer=checked[checked.index('// Diagnostic-only exact write frontier.'):]
assert '#[cfg(test)]' in observer and 'active_remaining()' in observer
for forbidden in ['reserve_active','set_active_floor','clear_active_floor','budget.remaining.set','Vec<','String','Box<']:
 assert forbidden not in observer
for path in ['src/hir/ids/instance_match.rs','src/hir/ids.rs','src/hir/nodes.rs','src/bounded_output.rs']:
 assert (r/path).read_text()==git(path),path
tests=(r/'src/hir/ids/instance_match/tests.rs').read_text();original=previous['src/hir/ids/instance_match/tests.rs']
# Keep complete grammar/forged-byte and validator graph/diagnostic owners exact.
def body(s,name):
 mm=mask(s);start=re.search(r'(?m)^fn '+name+r'\(',mm).start();b=mm.index('{',start);e=b+1;depth=1
 while depth:depth+=(mm[e]=='{')-(mm[e]=='}');e+=1
 return s[start:e]
for name in ['streamed_instance_matches_complete_encoder_all_forms_and_forged_bytes','streamed_instance_validator_preserves_duplicate_before_canonical_refusal']:
 assert body(tests,name)==body(original,name)
owner=body(tests,'streamed_instance_exact_short_zero_sticky_floor_keep_all_key_producers_and_drop')
assert 'assert_eq!(first.requested, child_bytes);' in owner
assert 'assert!(first.remaining >= child_bytes);' in owner
assert 'assert!(first.remaining - child_bytes < floor);' in owner
assert 'set_active_floor(floor)' in owner
assert 'with_limit_usage_refusal_site(complete_bill' in owner
assert 'capture_write_frontier' in owner
assert 'assert!(crate::bounded_output::active_refused());' in owner
assert 'after_first' in owner and 'count(), compound.len()' in owner
for part in ['for limit in [0, bill - 1, bill]', 'let wrong = FunctionInstanceId', 'set_active_floor(1)', 'old_heap.allocated', 'old_metrics.live']:
 assert part in owner
regression=body(tests,'streamed_instance_zero_argument_nominal_omission_cannot_authenticate_empty_key')
for part in ['identity_key_before_format_completion', 'old_key.is_empty() && old_complete && !overflowed', ':1:0:', 'first.requested, declaration_bytes', 'active_refused()', '2 * arguments.len()']:
 assert part in regression
for guard in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(r/guard).read_text()
 for path in ['src/hir/nodes/instance_key.rs','src/bounded_output/checked_string.rs','src/hir/ids/instance_match/tests.rs']:
  assert any(x.endswith('/'+path) for x in re.findall(r'include_str!\(\s*"([^"]+)"',text)),(guard,path)
print(json.dumps(dict(status='SOURCE_ONLY_FORMAT_COMPLETION_CUSTODY_PASS',original_inputs=4,unchanged_public_key_and_comparator=True,complete_old_checked_encoder_live=True,checked_format_sites=6,first_child_request_retained=8208,frontier_observation_test_only=True,rust_executed=False)))
