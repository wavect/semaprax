#!/usr/bin/env python3
"""Finite source custody only; no Rust execution or qualification claim."""
import argparse, hashlib, json, subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path.cwd());a=p.parse_args();r=a.repo.resolve()
d=r/'docs/evidence/streaming-instance-comparison-853-next-20261016'
m=json.loads((d/'source-inverses.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
previous={}
for row in m['rows']:
 current=(r/row['path']).read_bytes();old=(d/row['before_archive']).read_bytes()
 assert sha(current)==row['current_sha'],row['path']
 assert sha(old)==row['before_sha'],row['path']
 assert old==subprocess.check_output(['git','show',m['base']+':'+row['path']],cwd=r),row['path']
 at=row['offset'];new=row['current'].encode();prior=row['prior'].encode()
 assert current[at:at+len(new)]==new
 restored=current[:at]+prior+current[at+len(new):]
 assert restored==old,row['path']
 assert old[:at]+new+old[at+len(prior):]==current,row['path']
 previous[row['path']]=old.decode()
for row in m['new']:
 b=(r/row['path']).read_bytes();assert sha(b)==row['sha256'];assert len(b.splitlines())==row['lines']<=1500
# Complete unchanged independent construction oracle, not a substring digest.
def derive(s):
 start=s.index('impl FunctionInstanceId {\n    pub fn derive(')
 return s[start:s.index('\n// Validation compares',start)]
assert derive(previous['src/hir/ids.rs'])==derive((r/'src/hir/ids.rs').read_text())
def key_method(text, signature):
 start=text.index(signature)
 end=text.index('\n    }\n}', start)+6
 return text[start:end]
old_key=key_method(previous['src/hir/nodes.rs'], '    pub fn identity_key(&self) -> String {')
assert old_key==key_method((r/'src/hir/nodes.rs').read_text(), '    pub fn identity_key(&self) -> String {')
old_sink=previous['src/bounded_output.rs']
start=old_sink.index('pub(crate) struct CappedString {');end=old_sink.index('pub(crate) struct CappedVec {',start)
assert old_sink[start:end] in (r/'src/bounded_output.rs').read_text()
# The tracked producer is a complete original traversal. Reduce its two
# nominal alternatives to the unchanged original producer and compare every
# token, retaining quoted literal bytes exactly rather than ignoring contents.
import re
tokens=lambda text:re.findall(r'"(?:\\.|[^"\\])*"|[^\s]',text)
tracked=key_method((r/'src/hir/nodes/instance_key.rs').read_text(),
 '    pub(in crate::hir) fn identity_key_checked(&self, complete: &mut bool) -> String {')
a=old_key.index('                    let mut encoded = crate::bounded_output::CappedString::new();')
z=old_key.index('\n                }',a)
original_nominal=old_key[a:z]
a2=tracked.index('                    if crate::bounded_output::reserve_active_required(')
z2=tracked.index('\n                }',a2)
conditional=tracked[a2:z2]
then=conditional[conditional.index('\n')+1:conditional.index('                    } else {')]
then=then[then.index('                        let mut encoded'):]
then=then.replace('crate::bounded_output::CheckedCappedString::new(&mut *complete)', 'crate::bounded_output::CappedString::new()')
otherwise=conditional.split('                    } else {',1)[1]
otherwise=otherwise.removesuffix('\n                    }').replace('*complete = false;','',1)
assert tokens(then)==tokens(original_nominal)
assert tokens(otherwise)==tokens(original_nominal)
reduced=tracked[:a2]+original_nominal+tracked[z2:]
reduced=reduced.replace('pub(in crate::hir) fn identity_key_checked(&self, complete: &mut bool)', 'pub fn identity_key(&self)',1)
assert tokens(reduced)==tokens(old_key)
replacements={
 'src/hir/validation.rs':[(
 'FunctionInstanceId::derive(&instance.template, &instance.type_arguments)\n                    != instance.id',
 '!super::ids::instance_match::instance_matches!(\n                    &instance.id, &instance.template, &instance.type_arguments\n                )')],
 'src/hir/validation/generic_template.rs':[(
 'FunctionInstanceId::derive(&template.id, &arguments) != *instance_id',
 '!crate::hir::ids::instance_match::instance_matches!(instance_id, &template.id, &arguments)'),(
 'FunctionInstanceId::derive(callee, type_arguments) == *instance',
 'crate::hir::ids::instance_match::instance_matches!(instance, callee, type_arguments)'),(
 'FunctionInstanceId::derive(callee, arguments) != *instance',
 '!crate::hir::ids::instance_match::instance_matches!(instance, callee, arguments)')]
}
for path,pairs in replacements.items():
 expected=previous[path]
 for old,new in pairs:
  assert expected.count(old)==(2 if old=='FunctionInstanceId::derive(callee, arguments) != *instance' else 1)
  expected=expected.replace(old,new)
 assert expected==(r/path).read_text(),path
# The separate before-seen reachability authentication remains exactly its base.
p='src/hir/validation/generic_template/reachability.rs'
assert (r/p).read_bytes()==subprocess.check_output(['git','show',m['base']+':'+p],cwd=r)
s=(r/'src/hir/ids/instance_match.rs').read_text()
assert 'Err(fmt::Error)' not in s and 'Ok(())' in s
assert s.count('argument.identity_key()')==2
assert s.count('argument.identity_key_checked(&mut output.complete)')==1
assert 'output.remaining.is_empty() & !output.failed & output.complete' in ' '.join(s.split())
assert 'reserve_active_required(0)' not in s
assert '#[path = "instance_match/tests.rs"]' in s
assert 'mod instance_match;' in (r/'src/hir/ids.rs').read_text()
tests=(r/'src/hir/ids/instance_match/tests.rs').read_text()
selectors=json.loads((d/'pending-selectors.json').read_text())
assert len(selectors)==3
for row in selectors:
 assert tests.count('fn '+row['selector'].split('::')[-1]+'(')==1
 assert row['status']=='PENDING_NOT_EXECUTED'
print(json.dumps(dict(status='PASS_SOURCE_ONLY',complete_before_inputs=len(m['rows']),new_owners=len(m['new']),changed_predicates=5,unchanged_derive=True,unchanged_reachability=True,pending_bodies=3,unchanged_public_key=True,unchanged_ordinary_sink=True,tracked_key_mechanical_body=True)))
