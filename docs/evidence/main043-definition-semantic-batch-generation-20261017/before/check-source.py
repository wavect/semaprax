"""Finite definition generation proof; no verifier/binder/common main execution."""
from pathlib import Path
import copy,hashlib,json,runpy,shutil,subprocess,tempfile
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;REL=D.relative_to(R);BASE='0a35b31280d0987371b4a7817442713bd0b0ad3e';OLD=R/'docs/evidence/main043-definition-raw3-generation-20261017';G=R/'docs/evidence/main043-independent-custody-20261016';B=R/'docs/evidence/main043-newmain-acquisition-20261017/bind-current.py';sha=lambda b:hashlib.sha256(b).hexdigest();git=lambda p:subprocess.check_output(['git','show',BASE+':'+str(p)],cwd=R)
for p in OLD.iterdir():
 if p.is_file():assert p.read_bytes()==git(p.relative_to(R))
oldseal=json.loads((OLD/'definition-seal.json').read_bytes());seal=json.loads((D/'definition-seal.json').read_bytes());restored=copy.deepcopy(seal);restored['raw_inventories']=oldseal['raw_inventories'];assert restored==oldseal
for a,b in zip(seal['raw_inventories'],oldseal['raw_inventories']):
 assert a['path']==b['path'] and a['exact_target_include_count']==b['exact_target_include_count']==1
 assert len((R/a['path']).read_bytes())==a['length'] and sha((R/a['path']).read_bytes())==a['sha256']
 assert {k:v for k,v in a.items() if k not in ('length','sha256')}=={k:v for k,v in b.items() if k not in ('length','sha256')}
helper=(D/'definition_payload.py').read_text();oldhelper=(OLD/'definition_payload.py').read_text();h=helper.replace(str(REL),'docs/evidence/main043-definition-raw3-generation-20261017',1).replace(sha((D/'definition-seal.json').read_bytes()),sha((OLD/'definition-seal.json').read_bytes()),1);assert h==oldhelper
m=runpy.run_path(str(D/'definition_payload.py'));row=seal['old_row'];expected=(R/seal['old_archive']).read_text();assert m['definition_payload'](R,row,lambda p:None)==expected;assert len(expected.encode())==6160
assert m['definition_payload'](R,{'path':'unrelated'},lambda p:'original read')=='original read'
# Reconstruct the complete original recipe, then the prior public wrapper.
W=G/'verify-or-refresh.py';P=G/'guard-extension-recipe-edits.json';before=(D/'before/verify-or-refresh.py').read_text();recipe=json.loads(P.read_bytes());oldrecipe=json.loads((D/'before/guard-extension-recipe-edits.json').read_bytes());assert recipe[:-1]==oldrecipe and recipe[-1]=={'before':before,'after':W.read_text()};runpy.run_path(str(G/'guard-extensions.py'))['verify_recipe']()
oldb=(D/'before/bind-current.py').read_text();newb=B.read_text();assert newb.replace(sha(W.read_bytes()),sha(before.encode()),1).replace(sha(P.read_bytes()),sha((D/'before/guard-extension-recipe-edits.json').read_bytes()),1)==oldb
assert oldb.encode()==git(B.relative_to(R));assert before.encode()==git(W.relative_to(R));assert (D/'before/guard-extension-recipe-edits.json').read_bytes()==git(P.relative_to(R))
# Execute only the real wrapper bootstrap, stopping before its original verifier exec.
prefix=W.read_text().split("exec(compile(current,str(P),'exec')",1)[0];space={'__file__':str(W)};exec(compile(prefix,str(W),'exec'),space);assert space['m'].definition_payload(R,row,lambda p:None)==expected
refusals=0
def refuse(f):
 global refusals
 try:f()
 except (AssertionError,OSError,ValueError,UnicodeError):refusals+=1;return
 raise AssertionError('hostile admitted')
for key,value in [('bytes',6161),('sha256','0'*64),('path',row['path']+' ' )]:
 forged=dict(row);forged[key]=value
 if key!='path':refuse(lambda:m['definition_payload'](R,forged,lambda p:None))
with tempfile.TemporaryDirectory(prefix='definition-raw3-',dir='/Volumes/Semaprax/runtime-tmp') as t:
 root=Path(t);paths=[str(REL/'definition-seal.json'),seal['path'],seal['current_archive'],seal['old_archive'],*[x['path']for x in seal['raw_inventories']]]
 for p in paths:(root/p).parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/p,root/p)
 assert m['definition_payload'](root,row,lambda p:None)==expected
 for p in paths:
  path=root/p;raw=path.read_bytes()
  for bad in (raw[:-1],raw+b' ',b'foreign'):
   path.write_bytes(bad);refuse(lambda:m['definition_payload'](root,row,lambda p:None))
  path.write_bytes(raw);path.unlink();path.symlink_to(R/p);refuse(lambda:m['definition_payload'](root,row,lambda p:None));path.unlink();path.write_bytes(raw)
 # Each old raw generation refuses, never admitted as an acquisition projection.
 for old in oldseal['raw_inventories']:
  p=root/old['path'];raw=p.read_bytes();p.write_bytes(subprocess.check_output(['git','show','0fa63204bc67d3c35245a8634b90c7ecd4984707:'+old['path']],cwd=R));refuse(lambda:m['definition_payload'](root,row,lambda p:None));p.write_bytes(raw)
 # Pre-import helper authentication must refuse before mutated code executes.
 originaldir=root/'docs/evidence/main043-definition-generation-20261017';originaldir.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/'docs/evidence/main043-definition-generation-20261017/verify-or-refresh.before.py',originaldir/'verify-or-refresh.before.py')
 hp=root/REL/'definition_payload.py';hp.write_text(helper+'\nraise RuntimeError("untrusted helper executed")\n')
 wp=root/W.relative_to(R);wp.parent.mkdir(parents=True,exist_ok=True);refuse(lambda:exec(compile(prefix,str(wp),'exec'),{'__file__':str(wp)}))
result={'status':'PASS_SOURCE_ONLY_NO_VERIFIER_BINDER_COMMON_RUST_EXECUTION','new_raw_scopes':3,'only_seal_changes':'three raw length/hash pairs','old_definition_bytes':6160,'old_definition_sha256':sha(expected.encode()),'old_helper_body_restored':True,'whole_old_recipe_wrapper_binder_recovered':True,'refusals':refusals}
(D/'source-controls.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
