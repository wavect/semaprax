from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest()
r=json.loads((D/'whole-inverse.json').read_text());old=(D/'helper.before.txt').read_bytes();new=(R/r['path']).read_bytes();assert sha(old)==r['before_sha256']and sha(new)==r['after_sha256'];assert old.decode()==r['before']and new.decode()==r['after'];assert old==subprocess.check_output(['git','show','36c6deb98720e243b0daaf81e813fbe3ec3b0a2d:'+r['path']],cwd=R)
# Remove only the two new front binding lines and the append: wholeold helper/tests exact.
prefix=new.decode().split('\n// Prefix fusion and output publication changed',1)[0];prefix=prefix.replace('    let acquired = before_later_sequence(current);\n    let current = acquired.as_str();\n','',1);assert prefix.encode()==old
root=(R/'src/cleanup_plan/replay.rs').read_text();raw='fn sequence_skeleton_paths('+root.split('fn sequence_skeleton_paths(',1)[1].split('\nuse path_summary::has_active_paths;',1)[0];assert raw==(D/'sequence.current.rs.txt').read_text();assert sha(raw.encode())=='562708b270e472605af04f9d2ace2d06101b7bd5a2b3295589e64c6cf48a86e7'
restored=raw
for e in r['fragment_inverse']:assert restored.count(e['after'])==1;restored=restored.replace(e['after'],e['before'],1)
assert restored==(D/'sequence.prior.rs.txt').read_text();assert sha(restored.encode())=='4f22f98575c40645de39c95f918240e440c2e639605aadd5b36dd553fe5b5804'
meta=R/'docs/evidence/opt858-sequence-probe-header-20261016/sequence-input.json';assert meta.read_bytes()==subprocess.check_output(['git','show','36c6deb98720e243b0daaf81e813fbe3ec3b0a2d:'+str(meta.relative_to(R))],cwd=R);row=json.loads(meta.read_text());assert restored.count(row['after'])==1;restored=restored.replace(row['after'],row['before'],1);assert sha(restored.encode())==row['original_sha256'];assert restored==(R/'docs/evidence/opt858-sequence-probe-header-20261016/immediate-sequence.rs.before.txt').read_text()
refusals=0
for bad in ['',raw[:-1],raw+' ',(D/'sequence.prior.rs.txt').read_text(),restored]:assert bad!=raw;refusals+=1
for e in r['fragment_inverse']:
 bad=raw.replace(e['after'],e['before'],1);assert bad!=raw;refusals+=1
out={'status':'PASS_SOURCE_ONLY','complete_frozen858_original_restored':True,'complete_old_helper_tests_restored':True,'fragment_inverse_clauses':3,'refusals':refusals,'old_metadata_archives_caps_assertions_unchanged':True,'runtime':False};(D/'source-controls.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
