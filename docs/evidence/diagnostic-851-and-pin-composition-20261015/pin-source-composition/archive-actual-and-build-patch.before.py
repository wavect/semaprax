from pathlib import Path
import hashlib,json,shutil,difflib,subprocess,sys
OUT=Path(sys.argv[1]).resolve()
ROOT=Path(json.loads((OUT/'trusted-inputs.json').read_text())['root'])
OLD=Path('/Volumes/Semaprax/provenance/final-a417-actual-pin-capture-20261015')
NEW=Path(sys.argv[2]).resolve()
sha=lambda b:hashlib.sha256(b).hexdigest()
for origin,name in [(OLD,'previous-actual-a417-capture'),(NEW,'actual-current-capture')]:
 assert not OUT.is_relative_to(origin)
 shutil.copytree(origin,OUT/name,dirs_exist_ok=True)
plan=json.loads((OUT/'readonly-plan.json').read_text());meta=json.loads((OUT/'trusted-inputs.json').read_text())
for name in ['owning-build-result.json','prelude-pin-producers-result.json','expected-counts.json']:
 shutil.copyfile(Path(meta['round'])/name,OUT/name)
def changes(a,b,path=''):
 if type(a)!=type(b):return [{'pointer':path,'before':a,'after':b}]
 if isinstance(a,dict):
  assert a.keys()==b.keys();return [r for key in a for r in changes(a[key],b[key],path+'/'+key.replace('~','~0').replace('/','~1'))]
 if isinstance(a,list):
  assert len(a)==len(b);return [r for i,(x,y) in enumerate(zip(a,b)) for r in changes(x,y,path+'/'+str(i))]
 return [] if a==b else [{'pointer':path,'before':a,'after':b}]
wires=[]
for file in sorted((OLD/'wires').glob('*.json')):
 before=file.read_bytes();after=(NEW/'wires'/file.name).read_bytes();edits=[]
 for op,a,b,c,d in difflib.SequenceMatcher(None,before,after,autojunk=True).get_opcodes():
  if op!='equal':edits.append(dict(before_start=a,before_end=b,after_start=c,after_end=d,before_hex=before[a:b].hex(),after_hex=after[c:d].hex()))
 restored=after
 for e in reversed(edits):
  assert restored[e['after_start']:e['after_end']]==bytes.fromhex(e['after_hex']);restored=restored[:e['after_start']]+bytes.fromhex(e['before_hex'])+restored[e['after_end']:]
 assert restored==before
 wires.append(dict(wire=file.stem,before_sha256=sha(before),after_sha256=sha(after),json_changes=changes(json.loads(before),json.loads(after)),exact_byte_inverse=edits))
assert len(wires)==10
(OUT/'whole-wire-exact-inverses.json').write_text(json.dumps(wires,indent=2)+'\n')
before={p:(OUT/'source-before'/p).read_bytes() for p in set(e['path'] for e in plan['edits'])|{'docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json'}};after=before.copy()
for e in plan['edits']:
 a=e['before'].encode();b=e['after'].encode();assert after[e['path']].count(a)==1;after[e['path']]=after[e['path']].replace(a,b,1)
path='docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json';rows=json.loads(before[path])
for e in plan['edits']:
 if e['before']!=e['after']:
  row=next((row for row in rows if row['path']==e['path']),None)
  if row is not None:row['edits'].append({'before':e['before'],'after':e['after']})
for row in rows:
 raw=(after[row['path']] if row['path'] in after else subprocess.check_output(['git','show',meta['capture_head']+':'+row['path']],cwd=ROOT)).decode()
 for e in reversed(row['edits']):
  assert raw.count(e['after'])==1,(row['path'],e['after'][:60]);raw=raw.replace(e['after'],e['before'],1)
 archive=ROOT/'docs/evidence/prelude-forecast-286/pin-capture'/row['archive'];assert sha(raw.encode())==row['sha256'] and raw.encode()==archive.read_bytes()
after[path]=(json.dumps(rows,indent=2)+'\n').encode()
patch=[];inverse=[]
for name in sorted(before):
 file=OUT/'candidate-source-after'/name;file.parent.mkdir(parents=True,exist_ok=True);file.write_bytes(after[name])
 patch+=difflib.unified_diff(before[name].decode().splitlines(True),after[name].decode().splitlines(True),fromfile='a/'+name,tofile='b/'+name)
 inverse+=difflib.unified_diff(after[name].decode().splitlines(True),before[name].decode().splitlines(True),fromfile='a/'+name,tofile='b/'+name)
(OUT/'actual-literals-and-inverse-composition.patch').write_text(''.join(patch));(OUT/'complete-source.inverse.patch').write_text(''.join(inverse))
oldgraph=json.loads((OLD/'wires/public-graph.json').read_text());newgraph=json.loads((NEW/'wires/public-graph.json').read_text());oldamount=oldgraph['budget']['used_builder_bytes'];newamount=newgraph['budget']['used_builder_bytes']
record=dict(state='ACTUAL_FROZEN_SOURCE_STRICT_READONLY_VALIDATED_PATCH_NOT_APPLIED',source_basis=meta['capture_head'],later_root_at_preparation=meta['later_root_head_at_preparation'],pins=11,actual_old_amount=oldamount,actual_new_amount=newamount,delta=newamount-oldamount,all10_full_actual_wire_archives_preserved=True,all10_exact_byte_inverses_verified=True,complete_previous_pin_capture_archives_unchanged=True,existing_pin_capture_fullsource_inverse_composition_verified=True,apply_performed=False,exclusive_causality_or_gain_claim=False,source_before_sha256={p:sha(raw) for p,raw in before.items()},source_after_sha256={p:sha(raw) for p,raw in after.items()})
(OUT/'prepared-patch-record.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
