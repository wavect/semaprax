"""Bind only the unqualified NEWMAIN outer current source after final joins."""
from pathlib import Path
import argparse,hashlib,json,os,re,runpy,stat,subprocess
REL=Path('docs/evidence/main043-newmain-acquisition-20261017')
GUARD=Path('docs/evidence/main043-independent-custody-20261016')
sha=lambda b:hashlib.sha256(b).hexdigest()
def retain(path,data,write):
 try:m=path.lstat()
 except FileNotFoundError:m=None
 if m is not None:assert stat.S_ISREG(m.st_mode) and path.read_bytes()==data,('immutable source archive drift',str(path))
 elif write:
  path.parent.mkdir(parents=True,exist_ok=True);assert hasattr(os,'O_NOFOLLOW')
  fd=os.open(path,os.O_CREAT|os.O_EXCL|os.O_WRONLY|os.O_NOFOLLOW,0o600)
  with os.fdopen(fd,'wb')as output:output.write(data)
 else:raise AssertionError(('missing bound archive',str(path)))
def main():
 p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--expected-head',required=True);p.add_argument('--write',action='store_true');a=p.parse_args();R=a.repo.resolve();assert R.is_relative_to('/Volumes/Semaprax')
 head=lambda:subprocess.check_output(['git','-C',str(R),'rev-parse','HEAD']).decode().strip()
 assert re.fullmatch('[0-9a-f]{40}',a.expected_head) and head()==a.expected_head
 helper=R/GUARD/'semantic-receiver-main34d-view.py';before=helper.read_bytes();m=runpy.run_path(str(helper))
 for path,expected in m['PINS'].items():assert sha((R/path).read_bytes())==expected
 raw=(R/m['SOURCE']).read_bytes();assert len(raw)<=65536 and b'\r' not in raw and b'<<<<<<<' not in raw;raw.decode('utf8')
 archive=REL/'current-inputs'/(sha(raw)+'.txt');retain(R/archive,raw,a.write)
 pins=dict(m['PINS']);pins[str(archive)]=sha(raw);assert len(pins)<=64
 code=before.decode('utf8')
 for pattern,replacement in [(r"CURRENT_SHA='[0-9a-f]{64}'","CURRENT_SHA='"+sha(raw)+"'"),(r'CURRENT_LEN=[0-9]+','CURRENT_LEN='+str(len(raw))),(r"CURRENT_ARCHIVE='[^'\n]+'","CURRENT_ARCHIVE='"+str(archive)+"'"),(r'PINS=\{[^\n]*\}', 'PINS='+repr(pins))]:
  code,n=re.subn(pattern,lambda _:replacement,code);assert n==1
  # All remaining old code/assertions and fixed historical pins are untouched.
 verifier=R/GUARD/'verify-or-refresh.py';recipe=R/GUARD/'guard-extension-recipe-edits.json'
 outputs={helper:code.encode()}
 # A fixed definition generation owns this exact verifier/recipe pair.
 # An unchanged semantic helper requires no substitution; a changed helper
 # cannot retarget this generation's complete immutable prior verifier.
 if sha(verifier.read_bytes())=='34c12ca372f466ee79ac7770e8136e7a9bf472a005edb34d38e3c3d712c61a51':
  assert sha(recipe.read_bytes())=='98f11e71724d0ffeb948fca4c300addf9c52e938507b1709ab0cd29ae63ca342'
  assert sha(before)==sha(code.encode()),'new semantic helper needs an explicit definition generation'
  outputs[verifier]=verifier.read_bytes();outputs[recipe]=recipe.read_bytes()
 else:
  for path in [verifier,recipe]:
   text=path.read_text();old=sha(before);new=sha(code.encode());assert text.count(old)==1
   outputs[path]=text.replace(old,new,1).encode()
 assert head()==a.expected_head and (R/m['SOURCE']).read_bytes()==raw
 if a.write:
  for path,data in outputs.items():path.write_bytes(data)
  inverse_path=R/REL/'full-source-inverse.json';inverse=json.loads(inverse_path.read_bytes())
  for row in inverse['rows']:
   old=(R/row['before_archive']).read_bytes();assert sha(old)==row['before_sha256'] and len(old)==row['before_len']
   current=(R/row['path']).read_bytes();row['current_len']=len(current);row['current_sha256']=sha(current);row['inverse']=[{'at':0,'remove_len':len(current),'remove_sha256':sha(current),'insert_archive':row['before_archive']}]
  inverse_path.write_text(json.dumps(inverse,indent=2)+'\n')
 else:
  for path,data in outputs.items():assert path.read_bytes()==data,('NEWMAIN043 binding stale',str(path))
 print(json.dumps({'status':'SOURCE_ONLY_NEW_UNQUALIFIED_043_CURRENT_ENDPOINT','head':a.expected_head,'raw_source_sha256':sha(raw),'raw_source_len':len(raw),'immutable_previous':m['OLD_SHA'],'wrapper_sha256':sha(code.encode()),'old_view_outer_and_assertions_unchanged':True,'Rust_or_common_execution':False}))
if __name__=='__main__':main()
