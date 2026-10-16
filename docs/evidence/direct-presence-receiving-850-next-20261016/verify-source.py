#!/usr/bin/env python3
"""Finite source-only component custody; no Rust or qualification claims."""
from pathlib import Path
import argparse,hashlib,json,subprocess,re
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);a=p.parse_args();R=a.repo.resolve();D=Path(__file__).resolve().parent
J=json.loads((D/'source-inverses.json').read_text());h=lambda b:hashlib.sha256(b).hexdigest()
old={}
for row in J['rows']:
 b=(D/row['before_archive']).read_bytes();c=(R/row['source']).read_bytes()
 assert h(b)==row['before_sha256'] and h(c)==row['current_sha256']
 assert b==subprocess.check_output(['git','show',J['base']+':'+row['source']],cwd=R)
 s=c.decode()
 for e in row['edits']:
  assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
 assert s.encode()==b;old[row['source']]=b.decode()
for row in J['new_sources']:
 b=(R/row['source']).read_bytes();assert h(b)==row['sha256'];assert len(b.decode().splitlines())==row['lines']<=1500

def function(s,name):
 start=s.index('fn '+name+'(');start=s.rfind('    ',0,start)
 i=s.index('{',start);n=1;j=i+1
 while n:
  if s[j]=='{':n+=1
  elif s[j]=='}':n-=1
  j+=1
 return s[start:j]
mod='src/cleanup_plan/renewal_profile/ordinary/mod.rs';cur=(R/mod).read_text()
for name in ['collect_current_with_presence','collect_impl']:
 assert function(old[mod],name) in cur,'complete predecessor body changed'
record='src/cleanup_plan/renewal_profile/ordinary/record_presence.rs'
assert function(old[record],'get') in (R/record).read_text()
assert function(old[record],'new') in (R/record).read_text()
assert 'Option<&RecordPresence' not in function(cur,'collect_current_shared')
assert 'Some(presence)' not in function(cur,'collect_current_shared')
assert 'consuming::collect_with_presence(' in function(cur,'collect_current_shared')
# The selected collector order and complete shared body remain in the direct copy.
a=function(old[mod],'collect_impl');a=a[a.index('iterator::collect'):a.rindex('            Ok(())')]
x=a.index('                match presence {');y=a.index('                leaf::collect(',x)
a=a[:x]+'                consuming::collect_with_presence(program, function, output, presence, admission)?;\n'+a[y:]
b=function(cur,'collect_current_shared');b=b[b.index('iterator::collect'):b.rindex('            Ok(())') if '            Ok(())' in b else b.rindex('            Ok')]
assert re.sub(r'\s+','',a)==re.sub(r'\s+','',b),'collector tokens/order changed'
module='src/loop_calls/byte_writer/module_index/ordinary.rs';s=(R/module).read_text()
assert s.index('.reserve(RecordPresence::direct_caller_controls())')<s.index('Directory::collect_current_shared')<s.index('&record_presence')
test='src/cleanup_plan/renewal_profile/ordinary/record_presence/tests.rs'
front='        let direct_previous = super::super::direct_presence_tests::previous(path, current);\n        let current = direct_previous.as_str();\n'
assert (R/test).read_text().replace(front,'',1)==old[test],'original assertions/body changed'
# All original consuming tests and full consuming implementation remain exact.
for path in ['src/cleanup_plan/renewal_profile/ordinary/consuming.rs','src/cleanup_plan/renewal_profile/ordinary/consuming/tests.rs','src/cleanup_plan/renewal_profile/ordinary/consuming/old.rs']:
 assert (R/path).read_bytes()==subprocess.check_output(['git','show',J['base']+':'+path],cwd=R)
runtime=json.loads((D/'runtime-inputs.json').read_text());assert set(runtime)==set(J['rows'][i]['source']for i in range(3))
for path,row in runtime.items():
 s=(R/path).read_text();assert h(s.encode())==row['current_sha256']
 for e in row['edits']:assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
 assert s==old[path]
assert len(json.loads((D/'pending-selectors.json').read_text()))==2
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_before_rows':len(J['rows']),'runtime_rows':len(runtime),'new_rust_owners':len(J['new_sources']),'old_some_impl_get_constructor_and_all_original_assertions_preserved':True,'rust_executed':False}))
