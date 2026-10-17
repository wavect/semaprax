#!/usr/bin/env python3
"""Read-only byte-custody checks. Does not execute any Rust owner."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3]
D=Path(__file__).resolve().parent
h=lambda b:hashlib.sha256(b).hexdigest()
m=json.loads((D/'subjects.json').read_bytes())
rows=m['inputs'];assert len(rows)==13 and len({r['path'] for r in rows})==13
core='src/cleanup_plan/replay/factored/canonical_shared/tests.rs'
helper='src/source_reflows/factored_sources.rs'
s=(R/helper).read_bytes()
for row in rows:
 for domain,head in [('ours',m['ours_head']),('main',m['main_head'])]:
  rec=row[domain]
  if rec is None:
   assert row['path'].endswith('/borrowed_labels.rs') and domain=='main';continue
  b=(D/rec['archive']).read_bytes()
  assert len(b)==rec['length'] and h(b)==rec['sha256']
  assert rec['archive']=='objects/'+h(b)+'.txt'
  assert b==subprocess.check_output(['git','show',head+':'+row['path']],cwd=R)
  assert len(b)<=2097152
 if row['path']!=core:
  b=(R/row['path']).read_bytes();rec=row['current']
  assert len(b)==rec['length'] and h(b)==rec['sha256']
  assert b==(D/rec['archive']).read_bytes()
# Every complete incoming predecessor, ordered table, and inverse stays original.
for row in rows:
 if not row['path'].endswith('/sources.json'):continue
 proof=json.loads((D/row['main']['archive']).read_bytes())
 for record in proof['predecessors']:
  before=(D/next(r for r in rows if r['path']==str(Path(row['path']).parent/record['archive']))['main']['archive']).read_bytes()
  current=(D/next(r for r in rows if r['path']==record['path'])['main']['archive']).read_bytes()
  assert h(before)==record['before_sha256'] and h(current)==record['after_sha256']
  text=current.decode()
  for edit in record['inverse']:
   assert text.count(edit['after'])==1;text=text.replace(edit['after'],edit['before'],1)
  assert text.encode()==before
for family in ['targets','labels']:
 original=(R/('docs/evidence/factored-flat-'+family+'-v1/custody-function.before.txt')).read_bytes()
 assert s.count(original.replace(b'    fn ',b'    pub(crate) fn ',1))==1
main=(D/'main-helper.before.txt').read_bytes()
assert main==subprocess.check_output(['git','show',m['main_head']+':'+helper],cwd=R)
expected=main.replace(b'mod receivers {',b'mod receivers {\n    use super::main_std as std;',1).replace(b'::std::include_str!("../cleanup_plan/replay/factored/canonical_shared/tests.rs")',b'super::MAIN_CORE',1)
assert s.startswith(expected)
# Only private callback spelling and headers change inside the complete OURS methods.
old=(D/'ours-core.before.txt').read_bytes()
for name in ['complete_predecessors_and_new_target_sources_are_bound','complete_outer_label_sources_and_original_producer_remain_exact']:
 start=old.index(('    fn '+name+'(').encode());end=old.index(b'\n    }',start)+6
 body=old[start:end].replace(b'    fn ',b'    pub(crate) fn ',1)
 if name.startswith('complete_predecessors'):
  body=body.replace((name+'()').encode(),(name+'(incoming_input: fn(&str, &str) -> String)').encode(),1).replace(b'super::super::targets::incoming_input',b'incoming_input')
 assert s.count(body)==1,name
inverse=json.loads((D/'source-inverses.json').read_bytes())
for row in inverse['files']:
 b=(R/row['path']).read_bytes();assert len(b)==row['after_length'] and h(b)==row['after_sha256']
 if row['before'] is not None:
  before=row['before'].encode();assert h(before)==row['before_sha256']
  assert before==subprocess.check_output(['git','show',inverse['base']+':'+row['path']],cwd=R)
 assert row['inverse']['remove_sha256']==h(b)
assert len(s.splitlines())<1500
print(json.dumps({'state':'PASS_SOURCE_ONLY','subjects':13,'main_predecessors':7,'whole_ours_receivers':2,'whole_main_receivers':2,'whole_main_gate':1,'rust_execution':'UNEXECUTED'}))
