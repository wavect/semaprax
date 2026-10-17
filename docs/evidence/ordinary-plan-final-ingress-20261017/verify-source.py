"""Pure source controls for the distinct FINAL -> frozen787 edge. No binder main."""
from pathlib import Path
from collections import Counter
import argparse,hashlib,importlib.util,json,os,re,runpy,subprocess,tempfile,copy
D=Path(__file__).resolve().parent;R=D.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
def refused(fn):
 try:fn()
 except (AssertionError,KeyError,ValueError,IndexError,OSError):return True
 return False
def main():
 p=argparse.ArgumentParser();p.add_argument('--expected-head',required=True);p.add_argument('--output',type=Path);a=p.parse_args()
 assert os.environ.get('TMPDIR')=='/Volumes/Semaprax/runtime-tmp'and os.environ.get('PYTHONDONTWRITEBYTECODE')=='1'
 assert re.fullmatch('[0-9a-f]{40}',a.expected_head)
 git=lambda *args:subprocess.check_output(['git',*args],cwd=R)
 assert git('rev-parse','HEAD').decode().strip()==a.expected_head
 path=D/'bind-current-inputs.py';spec=importlib.util.spec_from_file_location('closed_final_ingress_binder',path);b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
 recipe=b.load(b.read(D/'recipe.json',b.SPEC_LENGTH,b.SPEC_SHA));assert tuple(recipe['keys'])==b.KEYS
 frozen_bytes={}
 for item in recipe['immutable']:
  raw=b.read(R/item['path'],item['length'],item['sha256']);assert raw==git('show',b.CHECKPOINT+':'+item['path']);frozen_bytes[item['path']]=raw
 rows=b.load(b.read(D/'current-inputs.json'));frozen=b.load(frozen_bytes['docs/evidence/ordinary-plan-integrated-e24-v1/stages.json']);legacy=b.load(frozen_bytes['docs/evidence/ordinary-plan-history-c8740045-v1/inputs.json'])
 helper=b.read(R/recipe['helper']).decode();template=b.read(R/recipe['helper_template'],digest=recipe['helper_template_sha256']).decode();assert b.scrub(helper)==b.scrub(template)
 length,digest=b.pins(helper);b.read(D/'current-inputs.json',length,digest)
 original=b.read(D/'ordinary_plan_inputs.before787.rs.txt');assert original==git('show',b.CHECKPOINT+':'+recipe['helper'])
 marker='\n\n// Separate FINAL -> sealed787 edge.';assert helper.count(marker)==1
 restored=helper.split(marker)[0];hook='    let (acquired_physical, acquired_current) = final_ingress_checkpoint(source, expected, current);\n    let current = acquired_current.as_str();\n    let physical = acquired_physical.as_str();'
 old='    let physical = CURRENT.iter().find(|(path, _)| *path == source).unwrap().1;'
 assert restored.count(hook)==1;restored=restored.replace(hook,old,1);assert restored.encode()==original
 assert len(helper.splitlines())<=1500
 assert len(rows)==len(frozen)==len(legacy)==len(recipe['checkpoints'])==6
 negatives=0;summary=[]
 def acquire(index,expected,supplied,physical_override=None):
  row=rows[index];fixed=frozen[index];old=legacy[index]
  if expected!=old['before_sha256']:return supplied
  selected=None
  for j,(entry,checkpoint)in enumerate(zip(rows,recipe['checkpoints'])):
   assert entry['path']==checkpoint['path']==b.KEYS[j]
   assert entry['checkpoint_sha256']==checkpoint['sha256']==frozen[j]['current_sha256']
   assert entry['expected_sha256']==legacy[j]['before_sha256']
   physical=b.read(R/entry['path'])if physical_override is None else physical_override.get(entry['path'],b.read(R/entry['path']))
   assert len(physical)==entry['current_length']and sha(physical)==entry['current_sha256']
   assert physical==b.read(R/entry['path'])
   retained=b.read(R/entry['current_archive'],entry['current_length'],entry['current_sha256']);assert retained==physical
   before=b.read(R/checkpoint['archive'],checkpoint['length'],checkpoint['sha256']);assert before==git('show',b.CHECKPOINT+':'+entry['path'])
   assert b.restore(physical,entry['inverse'])==before
   if j==index:
    assert supplied in [physical,before,fixed['e24'].encode(),fixed['legacy'].encode(),old['before'].encode()]
    selected=before if supplied==physical else supplied
  assert selected is not None;return selected
 for i,(row,fixed,old,item)in enumerate(zip(rows,frozen,legacy,recipe['checkpoints'])):
  raw=b.read(R/row['path']);assert raw==git('show',a.expected_head+':'+row['path'])
  checkpoint=b.read(R/item['archive'],item['length'],item['sha256']);assert checkpoint==git('show',b.CHECKPOINT+':'+row['path'])
  for value in [raw,checkpoint,fixed['e24'].encode(),fixed['legacy'].encode(),old['before'].encode()]:assert acquire(i,old['before_sha256'],value)==(checkpoint if value==raw else value)
  for expected,value in [(sha(raw),raw),('older expected namespace',b'older bytes')]:assert acquire(i,expected,value)==value
  for bad in [b'',raw+b' ',raw+raw,b'foreign']:
   assert refused(lambda:acquire(i,old['before_sha256'],bad));negatives+=1
  for victim in rows:
   assert refused(lambda:acquire(i,old['before_sha256'],raw,{victim['path']:b.read(R/victim['path'])+b' '}));negatives+=1
  # Exercise a genuinely nonidentity UTF-8 final edge without touching a source.
  synthetic=b'// synthetic final \xce\xbb\n'+raw+b'\n// final end\n';edits=b.inverse(synthetic,checkpoint);assert edits and b.restore(synthetic,edits)==checkpoint
  def reject_changed_final():assert b.restore(synthetic+b' ',edits)==checkpoint
  assert refused(reject_changed_final);negatives+=1
  summary.append({'path':row['path'],'raw_sha256':sha(raw),'checkpoint_sha256':sha(checkpoint),'synthetic_nonidentity_edits':len(edits)})
 # Preserve original raw scope anchors and every frozen required/maximum count.
 guard_path=R/'docs/evidence/main043-independent-custody-20261016/guard-extensions.py';guard=runpy.run_path(str(guard_path));allowed_path='docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json'
 allowed=b.load(b.read(R/allowed_path));prior=b.load(git('show',b.CHECKPOINT+':'+allowed_path));raw_scope=[]
 for path,rule in allowed.items():
  raw=b.read(R/path).decode();before=git('show','a82e00cc8:'+path).decode();parts=guard['fragments'](path,raw,before)
  guard['verify_guard'](path,raw,before,False)
  for kind in ['required','maximum']:
   for key,value in prior[path][kind].items():assert rule[kind][key]==value
  if 'ledger_capacity'in path:assert len(raw.splitlines())==1500
  raw_scope.append({'path':path,'actual_raw_sha256':sha(raw.encode()),'original_a82e_sha256':sha(before.encode()),'raw_projected':False})
  assert refused(lambda:guard['verify_guard'](path,raw+'include_str!("/foreign.rs"),',before,False));negatives+=1
 owners=b.load(b.read(D/'current-manifest.json'));assert len(owners['files'])==9
 for owner in owners['files']:
  raw=b.read(R/owner['path']);assert sha(raw)==owner['sha256']and len(raw.splitlines())==owner['lines']<=1500
 assert refused(lambda:b.load(b'{"x":1,"x":2}'));negatives+=1
 with tempfile.TemporaryDirectory(prefix='final-plan-physical-',dir=os.environ['TMPDIR'])as tmp:
  file=Path(tmp)/'regular';file.write_bytes(b'exact\n');assert b.read(file,6,sha(b'exact\n'))==b'exact\n'
  for value in [b'exact',b'exact\n ',b'foreign\n']:
   file.write_bytes(value);assert refused(lambda:b.read(file,6,sha(b'exact\n')));negatives+=1
  link=Path(tmp)/'link';link.symlink_to(file);assert refused(lambda:b.read(link));negatives+=1
  directory=Path(tmp)/'directory';directory.mkdir();assert refused(lambda:b.read(directory));negatives+=1
 assert git('rev-parse','HEAD').decode().strip()==a.expected_head
 result={'status':'PURE_SOURCE_PASS_NOT_RUST_QUALIFIED','head':a.expected_head,'checkpoint':b.CHECKPOINT,'complete_final_sources':summary,'actual_raw_scopes':raw_scope,'negative_controls':negatives,'original_legacy_checkpoint_restored':'BYTE_EXACT','immutable_packets':'BYTE_EXACT_GIT787','binder_main_executed':False,'Cargo':False,'Rust_tests':False,'runtime':'UNEXECUTED'}
 if a.output:
  assert a.output.is_absolute()and a.output.is_relative_to('/Volumes/Semaprax/provenance');a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps({k:v for k,v in result.items()if k not in ['complete_final_sources','actual_raw_scopes']}))
if __name__=='__main__':main()
