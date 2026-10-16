from pathlib import Path
import hashlib,json,shutil,difflib,subprocess,sys
OUT=Path(sys.argv[1]).resolve()
ROOT=Path(json.loads((OUT/'trusted-inputs.json').read_text())['root'])
OLD=Path('/Volumes/Semaprax/provenance/actual35cc-pin-capture-20261015')
NEW=Path(sys.argv[2]).resolve()
sha=lambda b:hashlib.sha256(b).hexdigest()
for origin,name in [(OLD,'previous-actual-35cc-capture'),(NEW,'actual-current-capture')]:
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
# Additional complete independently observed364d wires; published BEFORE stays35cc.
OBSERVED=Path(__file__).resolve().parent/'historical-observations/observed364d-managed'
shutil.copytree(OBSERVED,OUT/'independent-observed364d-capture')
observed_wires=[]
observed_record=json.loads((Path(__file__).resolve().parent/'skeleton-record.json').read_text())
assert {p.name for p in (OBSERVED/'wires').glob('*.json')}==set(observed_record['observed364d_managed_all10_wire_sha256'])
for file in sorted((OBSERVED/'wires').glob('*.json')):
 assert hashlib.sha256(file.read_bytes()).hexdigest()==observed_record['observed364d_managed_all10_wire_sha256'][file.name]
 prior=file.read_bytes();actual=(NEW/'wires'/file.name).read_bytes();inverse=[]
 for op,a,b,c,d in difflib.SequenceMatcher(None,prior,actual,autojunk=True).get_opcodes():
  if op!='equal':inverse.append(dict(before_start=a,before_end=b,after_start=c,after_end=d,before_hex=prior[a:b].hex(),after_hex=actual[c:d].hex()))
 restored=actual
 for e in reversed(inverse):
  assert restored[e['after_start']:e['after_end']]==bytes.fromhex(e['after_hex']);restored=restored[:e['after_start']]+bytes.fromhex(e['before_hex'])+restored[e['after_end']:]
 assert restored==prior
 observed_wires.append(dict(wire=file.stem,before_sha256=sha(prior),after_sha256=sha(actual),json_changes=changes(json.loads(prior),json.loads(actual)),exact_byte_inverse=inverse))
assert len(observed_wires)==10 and json.loads((OBSERVED/'wires/public-graph.json').read_text())['budget']['used_builder_bytes']==1103165
(OUT/'independent-observed364d-whole-wire-inverses.json').write_text(json.dumps(observed_wires,indent=2)+'\n')
before={p:(OUT/'source-before'/p).read_bytes() for p in set(e['path'] for e in plan['edits'])|{'docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json'}};after=before.copy()
for e in plan['edits']:
 a=e['before'].encode();b=e['after'].encode();assert after[e['path']].count(a)==1;after[e['path']]=after[e['path']].replace(a,b,1)
# Explicit immutable legacy-input domain; never infer it from a live checkout.
from source_inverse_composition import compose,blob
assert plan['capture_head']==meta['capture_head']
for name,raw in before.items():
 assert raw==blob(ROOT,meta['capture_head'],name),'complete captured source-before drift'
for name,expected in plan['source_sha256'].items():
 assert 'sha256:'+sha(before[name])==expected
for name,expected in plan['after_sha256'].items():
 assert 'sha256:'+sha(after[name])==expected
checkpoint_head=subprocess.check_output(['git','rev-parse',sys.argv[3]+'^{commit}'],cwd=ROOT,text=True).strip()
path='docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json'
projection,legacy_table,legacy_archives=compose(ROOT,meta['capture_head'],checkpoint_head,path,after)
assert legacy_table==before[path],'complete frozen legacy table must remain exact'
assert after[path]==before[path],'do not rewrite legacy manifest or append guessed hooks'
(OUT/'complete-source-projections.json').write_text(json.dumps(projection,indent=2)+'\n')
legacy_dir=OUT/'immutable-legacy';legacy_dir.mkdir(exist_ok=True)
for name,raw in {path:legacy_table,**legacy_archives}.items():
 target=legacy_dir/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(raw)
# Candidate literal sources remain hypothetical: no application or current-head
# qualification is asserted by this external source-composition stage.
patch=[];inverse=[]
for name in sorted(before):
 file=OUT/'candidate-source-after'/name;file.parent.mkdir(parents=True,exist_ok=True);file.write_bytes(after[name])
 patch+=difflib.unified_diff(before[name].decode().splitlines(True),after[name].decode().splitlines(True),fromfile='a/'+name,tofile='b/'+name)
 inverse+=difflib.unified_diff(after[name].decode().splitlines(True),before[name].decode().splitlines(True),fromfile='a/'+name,tofile='b/'+name)
(OUT/'actual-literals-and-inverse-composition.patch').write_text(''.join(patch));(OUT/'complete-source.inverse.patch').write_text(''.join(inverse))
oldgraph=json.loads((OLD/'wires/public-graph.json').read_text());newgraph=json.loads((NEW/'wires/public-graph.json').read_text());oldamount=oldgraph['budget']['used_builder_bytes'];newamount=newgraph['budget']['used_builder_bytes']
record=dict(state='ACTUAL_FROZEN_SOURCE_STRICT_READONLY_VALIDATED_PATCH_NOT_APPLIED',source_basis=meta['capture_head'],later_root_at_preparation=meta['later_root_head_at_preparation'],pins=11,actual_old_amount=oldamount,actual_new_amount=newamount,delta=newamount-oldamount,all10_full_actual_wire_archives_preserved=True,all10_exact_byte_inverses_verified=True,complete_previous_pin_capture_archives_unchanged=True,existing_pin_capture_fullsource_inverse_composition_verified=True,apply_performed=False,exclusive_causality_or_gain_claim=False,source_before_sha256={p:sha(raw) for p,raw in before.items()},source_after_sha256={p:sha(raw) for p,raw in after.items()})
(OUT/'prepared-patch-record.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
