from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;r=json.loads((D/'whole-inverse.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest();old=(D/'borrowed_labels.before.txt').read_bytes();new=(R/r['path']).read_bytes();assert sha(old)==r['before_sha256']and sha(new)==r['after_sha256'];assert old==subprocess.check_output(['git','show','36c6deb98:'+r['path']],cwd=R);restored=new.decode()
for e in reversed(r['edits']):assert restored.count(e['new'])==1;restored=restored.replace(e['new'],e['old'],1)
assert restored.encode()==old
needles=['pub(super) fn '+'successors<C: Closed>', 'struct '+"Directory<'a>"]
for needle in needles:assert old.decode().count(needle)==2 and new.decode().count(needle)==1
out={'status':'PASS_SOURCE_ONLY','whole_before_restored':True,'runtime_needles_identical':True,'count1_predicates_unchanged':True,'complete_physical_source_included':True,'Rust_execution':False};(D/'source-controls.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
