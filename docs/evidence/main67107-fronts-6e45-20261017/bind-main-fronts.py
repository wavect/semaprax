#!/usr/bin/env python3
"""Bind closed MAIN front subjects to an explicit actual source HEAD; no semantic pins."""
import argparse,difflib,hashlib,json,os,subprocess
from pathlib import Path
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);ap.add_argument('--source-repo');ap.add_argument('--expected-head',required=True);ap.add_argument('--write',action='store_true');args=ap.parse_args()
assert os.environ.get('TMPDIR')=='/Volumes/Semaprax/runtime-tmp'
R=Path(args.repo).resolve();S=Path(args.source_repo).resolve() if args.source_repo else R
assert R.is_relative_to('/Volumes/Semaprax') and S.is_relative_to('/Volumes/Semaprax')
E=R/'docs/evidence/main67107-fronts-6e45-20261017';sha=lambda b:hashlib.sha256(b).hexdigest()
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=S).decode().strip()==args.expected_head
def inverse(before,current):
    old,new=before.splitlines(keepends=True),current.splitlines(keepends=True);edits=[]
    for tag,i,j,a,z in difflib.SequenceMatcher(None,old,new,autojunk=False).get_opcodes():
        if tag=='equal':continue
        present=''.join(new[a:z]);previous=''.join(old[i:j])
        if not present or current.count(present)!=1:return [{'after':current,'before':before}]
        edits.append({'after':present,'before':previous})
    restored=current
    for e in edits:
        if restored.count(e['after'])!=1:return [{'after':current,'before':before}]
        restored=restored.replace(e['after'],e['before'],1)
    return edits if restored==before else [{'after':current,'before':before}]
raw=(E/'front-spec.json').read_bytes();assert sha(raw)=='c75d7e8ddd7045a5863e291cfa2aaab094f3349f85ae69494c622b9b89aebffc'
spec=json.loads(raw);assert len(spec)==7
bindings={};outputs={};targets={}
for entry in spec:
    original=(R/entry['original']).read_bytes();assert sha(original)==entry['original_sha256']
    data=json.loads(original);rows=data['predecessors'] if isinstance(data,dict) else data
    assert len(rows)==len(entry['rows'])
    assert [r['path'] for r in rows]==[r['path'] for r in entry['rows']]
    for row, target in zip(rows,entry['rows']):
        old=(R/target['archive']).read_bytes();assert sha(old)==target['before_sha256']==row['before_sha256']
        before=old.decode();current=(S/row['path']).read_bytes();text=current.decode();assert text and text.count(text)==1
        bindings[row['path']]=sha(current);targets[target['archive']]=sha(old)
        if row['after_sha256']!=sha(current):
            row['after_sha256']=sha(current);row['inverse']=inverse(before,text)
        restored=text
        for e in row['inverse']:
            assert restored.count(e['after'])==1
            restored=restored.replace(e['after'],e['before'],1)
        assert restored==before
    if isinstance(data,dict):
        for row in data['new_files']:
            b=(S/row['path']).read_bytes();bindings[row['path']]=sha(b);row['sha256']=sha(b)
        child='src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs'
        assert all(r['path']!=child for r in data['new_files'])
        b=(S/child).read_bytes();bindings[child]=sha(b);data['new_files'].append({'path':child,'sha256':sha(b)})
    outputs[entry['table']]=(json.dumps(data,indent=2)+'\n').encode()
for p,h in bindings.items():assert sha((S/p).read_bytes())==h,('source raced',p)
for p,h in targets.items():assert sha((R/p).read_bytes())==h
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=S).decode().strip()==args.expected_head
receipt={'status':'NEW_UNQUALIFIED_MAIN_FRONT_BINDINGS_ONLY','source_head':args.expected_head,'source_repo':str(S),'raw_inputs':bindings,'immutable_archives':targets,'outputs':{p:sha(b) for p,b in outputs.items()},'no_cargo_or_runtime':True}
if args.write:
    for p,b in outputs.items():(R/p).write_bytes(b)
    (E/'front-binding-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
else:
    for p,b in outputs.items():assert (R/p).read_bytes()==b,('front needs binding',p)
print(json.dumps(receipt,indent=2))
