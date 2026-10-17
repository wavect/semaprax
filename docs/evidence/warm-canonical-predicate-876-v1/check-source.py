"""Pure source/inverse proof only; never builds or executes Rust."""
from pathlib import Path
import hashlib,json,subprocess
E=Path(__file__).resolve().parent; R=E.parents[2]
d=json.loads((E/'whole-source-inverses.json').read_text())
for row in d['files']:
 current=(R/row['source']).read_bytes();before=row['edits'][0]['old'].encode()
 assert current==row['edits'][0]['new'].encode()
 assert len(current)==row['after_length'] and hashlib.sha256(current).hexdigest()==row['after_sha256']
 assert len(before)==row['before_length'] and hashlib.sha256(before).hexdigest()==row['before_sha256']
 assert before==subprocess.check_output(['git','-C',str(R),'show',d['base']+':'+row['source']])
 assert current.count(b'\n')<=1500
 if row['source'].endswith('arena_tests.rs'):assert current.startswith(before)
 else:
  prefix=b'        // Existing received owner place: no copied tag, bool or reference.\n        // scratch_paid=true makes cold_dead_node false by its own definition.\n        if let Self { scratch_paid: false, .. } = self {\n'
  assert current.count(prefix)==1
  restored=current.replace(prefix,b'',1)
  marker=b'            return self.cold_dead_id(function, root, budget);\n        }\n        }\n'
  assert restored.count(marker)==1
  assert restored.replace(marker,b'            return self.cold_dead_id(function, root, budget);\n        }\n',1)==before
print('PASS three whole inverses, binding-free dispatch only, unchanged original test prefix and module limits; no Rust execution')
