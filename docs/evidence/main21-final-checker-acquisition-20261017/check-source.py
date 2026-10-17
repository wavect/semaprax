"""Minimal source-only wrapper controls; never execute its checker or any binder."""
from pathlib import Path
import ast,hashlib,importlib.util,json,os,subprocess,tempfile,copy
D=Path(__file__).resolve().parent;R=D.parents[2]
assert os.environ.get('TMPDIR')=='/Volumes/Semaprax/runtime-tmp'and os.environ.get('PYTHONDONTWRITEBYTECODE')=='1'
spec=importlib.util.spec_from_file_location('closed_main21_wrapper',D/'check-current.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
original=(D/'check-source.before.py.txt').read_bytes();assert original==(R/m.ORIGINAL).read_bytes()
assert original==subprocess.check_output(['git','show','ac1e9f53e995d2844a0d334af70dc1a3836eed62:'+m.ORIGINAL],cwd=R)
program,edits=m.render(original);restored=program
for before,after in reversed(edits):assert restored.count(after)==1;restored=restored.replace(after,before,1)
assert restored.encode()==original and len(edits)==4
ast.parse((D/'check-current.py').read_text())
old=(R/m.OLD_MANIFEST).read_bytes();assert len(old)==m.OLD_MANIFEST_LENGTH and m.sha(old)==m.OLD_MANIFEST_SHA
inverse=(R/m.OLD_INVERSES).read_bytes();assert len(inverse)==m.OLD_INVERSE_LENGTH and m.sha(inverse)==m.OLD_INVERSE_SHA
raw=subprocess.check_output(['git','show','7cb45154f8c83011de82b7d45ebd727e7486f0fa:'+m.NEW_MANIFEST],cwd=R);namespace=m.load(raw)
assert m.sha(m.projection(namespace))==m.NEW_PROJECTION_SHA
old=m.load(old);legacy={x['path']:x for x in namespace['legacy_current']};targets={}
for domain in ['main','ours']:
 for row in old[domain]:
  pair=(row['current_len'],row['current_sha256']);assert row['path']not in targets or targets[row['path']]==pair;targets[row['path']]=pair
assert len(targets)==len(legacy)==149
for path,(length,digest)in targets.items():assert (legacy[path]['view_len'],legacy[path]['view_sha256'])==(length,digest)and length<=m.OLD_CAP
for row in m.load(inverse):assert targets[row['path']][1]==row['after_sha256']
negative=0
def refused(fn):
 global negative
 try:fn()
 except (AssertionError,KeyError,ValueError,OSError):negative+=1
 else:raise AssertionError('hostile source admitted')
checkpoint=b'checkpoint';row={'view_len':len(checkpoint),'view_sha256':m.sha(checkpoint)}
assert m.select_checkpoint(row,len(checkpoint),m.sha(checkpoint),b'actual',checkpoint)==checkpoint
for bad in [b'',checkpoint+b' ',b'foreign']:
 refused(lambda:m.select_checkpoint(row,len(checkpoint),m.sha(checkpoint),b'actual',bad))
refused(lambda:m.select_checkpoint(row,len(checkpoint),'0'*64,b'actual',checkpoint))
refused(lambda:m.select_checkpoint({'view_len':m.OLD_CAP+1,'view_sha256':'0'*64},m.OLD_CAP+1,'0'*64,b'actual',b''))
for changed in [original+b' ',original[:-1],b'foreign']:refused(lambda:m.render(changed))
refused(lambda:m.load(b'{"x":1,"x":2}'))
forged=copy.deepcopy(namespace);forged['legacy_current'][0]['view_sha256']='0'*64
assert m.sha(m.projection(forged))!=m.NEW_PROJECTION_SHA
with tempfile.TemporaryDirectory(prefix='main21-checker-source-',dir=os.environ['TMPDIR'])as tmp:
 path=Path(tmp)/'raw';path.write_bytes(b'exact\n');assert m.physical(path,6,m.sha(b'exact\n'))[0]==b'exact\n'
 for value in [b'exact',b'exact\n ',b'foreign\n']:
  path.write_bytes(value);refused(lambda:m.physical(path,6,m.sha(b'exact\n')))
 link=Path(tmp)/'link';link.symlink_to(path);refused(lambda:m.physical(link))
 directory=Path(tmp)/'directory';directory.mkdir();refused(lambda:m.physical(directory))
result={'status':'PURE_SOURCE_PASS_NOT_RUST_QUALIFIED','original_checker_sha256':m.sha(original),'exact_whole_checker_inverse':True,'finite_acquisition_sites':4,'old_manifest_unchanged':True,'legacy_current_targets':149,'whole_inverse_targets':11,'subsequent_raw3_census_byte_exact':True,'negative_controls':negative,'checker_main_executed':False,'binder_executed':False,'Rust_or_Cargo':False}
print(json.dumps(result))
