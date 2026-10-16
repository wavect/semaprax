#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='b5eca55b96890e1a815f0d8d9e7b42c78d743b07';MAIN='f1f8b5943d784043d24b39daa35bef91aafe02da';get=lambda ref,p:subprocess.check_output(['git','show',ref+':'+p],cwd=R);sha=lambda b:hashlib.sha256(b).hexdigest()
def function(s,name):
 a=s.index('fn '+name+'(');i=s.index('{',a);n=1;j=i+1
 while n:n+=(s[j]=='{')-(s[j]=='}');j+=1
 return s[a:j]
root='src/cleanup_plan/replay.rs';s=(R/root).read_text();assert s.replace('#[cfg(test)]\nmod sequence_prefix;\n','',1).encode()==get(BASE,root)
assert (R/'src/cleanup_plan/replay/path_array/tests.rs').read_bytes()==get(BASE,'src/cleanup_plan/replay/path_array/tests.rs')
for name in ['sequence_skeleton_paths','sequence_call_argument']:
 assert function(s,name)==function(get(BASE,root).decode(),name)
assert 'let observations = prefix.observations;'in function(s,'sequence_skeleton_paths')
assert 'sequence_prefix::try_take'not in function(s,'sequence_skeleton_paths')
current=(R/'src/cleanup_plan/replay/sequence_prefix/main_current.rs').read_text();assert function(current,'sequence_skeleton_paths')==function(get(MAIN,root).decode(),'sequence_skeleton_paths')
mainhelper=get(MAIN,'src/cleanup_plan/replay/sequence_prefix.rs').decode();helper=(R/'src/cleanup_plan/replay/sequence_prefix.rs').read_text()
for name in ['receiving_bytes','minimum_items','try_take']:assert function(helper,name)==function(mainhelper,name)
assert 'use super::*;'not in helper
before=(R/'src/cleanup_plan/replay/sequence_prefix/before.rs').read_text();original=get(MAIN,'src/cleanup_plan/replay/sequence_prefix/before.rs').decode();assert function(before,'sequence_skeleton_paths')==function(original,'sequence_skeleton_paths')
tests=(R/'src/cleanup_plan/replay/sequence_prefix/tests.rs').read_text();original=get(MAIN,'src/cleanup_plan/replay/sequence_prefix/tests.rs').decode()
for line in original.splitlines():
 if line.startswith('fn '):
  name=line.split('fn ',1)[1].split('(',1)[0];assert function(tests,name)==function(original,name),name
assert 'use super::main_current::sequence_skeleton_paths;'in tests
for path in ['src/cleanup_plan/replay/skeleton_work.rs','src/cleanup_plan/replay/physical_budget.rs','src/cleanup_plan/replay/allocation.rs']:
 b=(R/path).read_bytes()
 if path.endswith('physical_budget.rs'):b=b.replace(b'pub(in crate::cleanup_plan::replay) mod authentication;\n',b'',1)
 assert b==get(MAIN,path),path
for name,p in [('receiver-inverse12.json','docs/evidence/receiver-source-custody-v1/inverse.json'),('compact-inverse7.json','docs/evidence/compact-staged-observations-v1/inverse.json')]:assert (D/name).read_bytes()==get(MAIN,p);assert(R/p).read_bytes()==get(BASE,p)
for row in json.loads((D/'new-owners.json').read_text()):
 b=(R/row['source']).read_bytes();assert sha(b)==row['sha256'];assert len(b.decode().splitlines())==row['lines']<=1500
# Original Main helper and tests are entirely closed behind the real test registration.
assert '#[cfg(test)]\nmod sequence_prefix;'in s
for child in ['before','tests','main_current','shipping_tests']:assert '#[cfg(test)]\nmod '+child+';'in helper
print(json.dumps({'status':'PASS_SOURCE_ONLY','shipping_sequence_and_call_sequence_exact':True,'Main_helper_current_and_clone_complete':True,'original_five_assertion_bodies_exact':True,'original_path_array_preflight_exact':True,'same_shared_work_budget_receivers':True,'rust_executed':False}))
