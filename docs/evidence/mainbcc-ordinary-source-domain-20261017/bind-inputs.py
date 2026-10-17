"""Bind one NEW unqualified BCC domain; no original table/authority changes."""
from pathlib import Path
import argparse,hashlib,json,os,re,stat,subprocess
MAIN='bccbe9696eca892045b0603ad663234062df15f4'
REL=Path('docs/evidence/mainbcc-ordinary-source-domain-20261017')
TABLE='docs/evidence/ordinary-replay-routes-v1/sources.json'
CAP=1_048_576
sha=lambda b:hashlib.sha256(b).hexdigest()
def retain(repo,path,data,write):
 target=repo/path
 try:metadata=target.lstat()
 except FileNotFoundError:metadata=None
 if metadata is not None:
  assert stat.S_ISREG(metadata.st_mode) and target.read_bytes()==data,('immutable archive drift',str(path))
 elif write:
  target.parent.mkdir(parents=True,exist_ok=True)
  assert hasattr(os,'O_NOFOLLOW')
  fd=os.open(target,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
  with os.fdopen(fd,'wb')as output:output.write(data)
 else:raise AssertionError(('missing archive',str(path)))
def main():
 p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--inputs',type=Path,required=True);p.add_argument('--production-overlay',type=Path);p.add_argument('--write',action='store_true');a=p.parse_args()
 R=a.repo.resolve();I=a.inputs.resolve();D=R/REL
 assert R.is_relative_to('/Volumes/Semaprax') and I.is_relative_to('/Volumes/Semaprax')
 if a.production_overlay:assert a.production_overlay.resolve().is_relative_to('/Volumes/Semaprax')
 git=lambda *args:subprocess.check_output(['git','-C',str(R),*args])
 observed=subprocess.check_output(['git','-C',str(I),'rev-parse','HEAD']).decode().strip()
 old=lambda path:git('show',MAIN+':'+path)
 table=old(TABLE);rows=json.loads(table);assert len(rows)==6
 paths=[TABLE];original={TABLE:table}
 for row in rows:
  path=row['path'];before=row['archive'];b=old(path);c=old(before)
  assert sha(b)==row['after_sha256'] and sha(c)==row['before_sha256']
  restored=b.decode('utf8')
  for edit in row['inverse']:
   assert restored.count(edit['after'])==1
   restored=restored.replace(edit['after'],edit['before'],1)
  assert restored.encode()==c
  paths.extend([path,before]);original[path]=b;original[before]=c
 assert len(paths)==len(set(paths))==13
 entries=[]
 for index,path in enumerate(paths):
  before=original[path]
  if path=='src/source_reflows/semantic_receivers.rs':current=(R/path).read_bytes()
  elif path.startswith('src/') and a.production_overlay:current=(a.production_overlay/path).read_bytes()
  else:current=(I/path).read_bytes()
  assert b'<<<<<<<' not in current and b'\r' not in current
  current.decode('utf8');before.decode('utf8');assert max(len(current),len(before))<=CAP
  if path.startswith('docs/'):
   assert current==before,('immutable incoming BCC table/archive drift',path)
   old_archive=current_archive=Path(path)
  else:
   old_archive=REL/'main-inputs'/f'{index:02}.txt';current_archive=REL/'current-inputs'/(sha(current)+'.txt')
   retain(R,old_archive,before,a.write);retain(R,current_archive,current,a.write)
  entries.append({'path':path,'current_len':len(current),'current_sha256':sha(current),'current_archive':str(current_archive),'main_len':len(before),'main_sha256':sha(before),'main_archive':str(old_archive),'whole_source_inverse':[{'at':0,'remove_len':len(current),'remove_sha256':sha(current),'insert_archive':str(old_archive)}]})
 assert subprocess.check_output(['git','-C',str(I),'rev-parse','HEAD']).decode().strip()==observed
 manifest={'schema':'semaprax.mainbcc-ordinary-source-domain.v1','main_head':MAIN,'original_table_sha256':sha(table),'inputs':entries}
 payload=(json.dumps(manifest,indent=2)+'\n').encode();assert len(payload)<=CAP
 helper=R/'src/source_reflows/incoming67107_semantic_receivers.rs';code=helper.read_text()
 code,n=re.subn(r'const MAINBCC_MANIFEST_LEN: usize = [0-9]+;',f'const MAINBCC_MANIFEST_LEN: usize = {len(payload)};',code);assert n==1
 code,n=re.subn(r'const MAINBCC_MANIFEST_SHA: &str = "[0-9a-f]{64}";',f'const MAINBCC_MANIFEST_SHA: &str = "{sha(payload)}";',code);assert n==1
 if a.write:
  (D/'input-manifest.json').write_bytes(payload);helper.write_text(code)
  for name in ['full-source-inverse.json','main34d-endpoint-inverse.json']:
   inverse_path=D/name;proof=json.loads(inverse_path.read_bytes())
   for row in proof['rows']:
    before=(R/row['before_archive']).read_bytes();assert sha(before)==row['before_sha256'] and len(before)==row['before_len']
    current=(R/row['path']).read_bytes();row['current_len']=len(current);row['current_sha256']=sha(current);row['inverse']=[{'at':0,'remove_len':len(current),'remove_sha256':sha(current),'insert_archive':row['before_archive']}]
   inverse_path.write_text(json.dumps(proof,indent=2)+'\n')
 else:assert (D/'input-manifest.json').read_bytes()==payload and helper.read_text()==code
 print(json.dumps({'status':'SOURCE_ONLY_NEW_UNQUALIFIED_BCC_ENDPOINTS','main_head':MAIN,'observed_inputs_head':observed,'production_overlay':str(a.production_overlay)if a.production_overlay else None,'input_count':13,'original_complete_rows':6,'manifest_sha256':sha(payload),'manifest_len':len(payload),'cap':CAP,'Rust_or_authority_or_common_execution':False}))
if __name__=='__main__':main()
