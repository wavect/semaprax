"""Bind only a distinct FINAL -> frozen787 source edge at an explicit full HEAD.
No compiler, runtime, historical binder or authority tool is executed.
"""
from pathlib import Path
import argparse,difflib,hashlib,json,os,re,stat,subprocess
D=Path(__file__).resolve().parent
CAP=2*1024*1024
SPEC_LENGTH=4107
SPEC_SHA='13dcae708fa9b246b677905eab33da1afac16292312625b2ceff64c3a7bfb6d2'
CHECKPOINT='787d9810480209532fa645b8cb97f2585ce5f426'
KEYS=('src/cleanup_plan/replay.rs','src/source_reflows_merge.rs','src/source_reflows_merge/main67107_inputs.rs','tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs')
sha=lambda b:hashlib.sha256(b).hexdigest()
def load(raw):
 def unique(pairs):
  result={}
  for key,value in pairs:assert key not in result;result[key]=value
  return result
 return json.loads(raw,object_pairs_hook=unique)
def read(path,length=None,digest=None):
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 try:
  first=os.fstat(fd);assert stat.S_ISREG(first.st_mode)and first.st_size<=CAP
  if length is not None:assert first.st_size==length
  with os.fdopen(fd,'rb',closefd=False)as f:raw=f.read(first.st_size);assert len(raw)==first.st_size and not f.read(1)
  last=os.fstat(fd);physical=path.lstat();identity=lambda m:(m.st_dev,m.st_ino,m.st_size,m.st_mtime_ns,m.st_ctime_ns,m.st_mode)
  assert stat.S_ISREG(physical.st_mode)and identity(first)==identity(last)==identity(physical)
  raw.decode('utf8')
  if digest is not None:assert sha(raw)==digest
  return raw
 finally:os.close(fd)
def retain(path,raw,write):
 try:path.lstat()
 except FileNotFoundError:
  assert write,('missing retained source',str(path));path.parent.mkdir(parents=True,exist_ok=True)
  fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
  with os.fdopen(fd,'wb')as f:f.write(raw)
 else:assert read(path)==raw,('immutable archive drift',str(path))
def restore(raw,edits):
 value=raw;last=len(value);assert len(edits)<=512
 for edit in edits:
  start=edit['start_byte'];after=edit['after'].encode();end=start+len(after)
  assert 0<=start<=end<=last and value[start:end]==after
  value=value[:start]+edit['before'].encode()+value[end:];last=start
 value.decode('utf8');return value
def inverse(raw,before):
 left=raw.decode().splitlines(True);right=before.decode().splitlines(True);offsets=[0]
 for line in left:offsets.append(offsets[-1]+len(line.encode()))
 edits=[]
 for tag,i,j,k,l in difflib.SequenceMatcher(a=left,b=right,autojunk=False).get_opcodes():
  if tag=='equal':continue
  now=''.join(left[i:j]);old=''.join(right[k:l]);prefix=0
  while prefix<min(len(now),len(old))and now[prefix]==old[prefix]:prefix+=1
  suffix=0
  while suffix<min(len(now)-prefix,len(old)-prefix)and now[-1-suffix]==old[-1-suffix]:suffix+=1
  end_now=len(now)-suffix if suffix else len(now);end_old=len(old)-suffix if suffix else len(old)
  edits.append({'start_byte':offsets[i]+len(now[:prefix].encode()),'after':now[prefix:end_now],'before':old[prefix:end_old]})
 edits.reverse();assert restore(raw,edits)==before;return edits
def pins(code):
 length=re.findall(r'const FINAL_INGRESS_LENGTH: usize = ([0-9]+);',code)
 digest=re.findall(r'const FINAL_INGRESS_SHA256: &str = "([0-9a-f]{64})";',code)
 assert len(length)==len(digest)==1
 return int(length[0]),digest[0]
def scrub(code):
 code,n=re.subn(r'const FINAL_INGRESS_LENGTH: usize = [0-9]+;','const FINAL_INGRESS_LENGTH: usize = SOURCE_LENGTH;',code);assert n==1
 code,n=re.subn(r'const FINAL_INGRESS_SHA256: &str = "[0-9a-f]{64}";','const FINAL_INGRESS_SHA256: &str = "SOURCE_SHA256";',code);assert n==1
 return code
def custody_source_template(template):
 # This exact new append block is the only extension to the old helper body.
 addition=read(D.parent/'ordinary-plan-macro-custody-14d217-v1/helper-addition.rs.txt', 1046, '18b9d238b9c4393e8260d57916c8b937fc08d4c251b65ed2394e8c0cf566122f').decode()
 assert template.count('pub(super) fn original_root_checkpoint(')==0
 assert addition.count('pub(super) fn original_root_checkpoint(')==1
 return template+addition

def main():
 p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=D.parents[2]);p.add_argument('--expected-head',required=True);p.add_argument('--write',action='store_true');a=p.parse_args()
 R=a.repo.resolve();assert R==D.parents[2]and R.is_relative_to('/Volumes/Semaprax')
 assert os.environ.get('TMPDIR')=='/Volumes/Semaprax/runtime-tmp'and os.environ.get('PYTHONDONTWRITEBYTECODE')=='1'
 assert re.fullmatch('[0-9a-f]{40}',a.expected_head)
 git=lambda *args:subprocess.check_output(['git',*args],cwd=R)
 head=lambda:git('rev-parse','HEAD').decode().strip()
 assert head()==a.expected_head
 spec=load(read(D/'recipe.json',SPEC_LENGTH,SPEC_SHA));assert spec['checkpoint']==CHECKPOINT and tuple(spec['keys'])==KEYS
 template=read(R/spec['helper_template'],digest=spec['helper_template_sha256']).decode()
 template=custody_source_template(template)
 helper_path=R/spec['helper'];helper=read(helper_path).decode();assert scrub(helper)==scrub(template),'unexpected source helper drift'
 immutable={}
 for item in spec['immutable']:
  held=read(R/item['path'],item['length'],item['sha256']);assert held==git('show',CHECKPOINT+':'+item['path']);immutable[item['path']]=held
 frozen=load(immutable['docs/evidence/ordinary-plan-integrated-e24-v1/stages.json']);assert tuple(x['path']for x in frozen)==KEYS
 old=load(immutable['docs/evidence/ordinary-plan-history-c8740045-v1/inputs.json']);assert tuple(x['path']for x in old)==KEYS
 old_length,old_sha=pins(helper);old_packet=read(D/'current-inputs.json',old_length,old_sha);prior=load(old_packet)
 assert len(prior)==len(spec['checkpoints'])==6
 snapshots={};new=[];source_fingerprints={}
 for i,(item,previous,stage,legacy)in enumerate(zip(spec['checkpoints'],prior,frozen,old)):
  path=KEYS[i];assert item['path']==previous['path']==stage['path']==legacy['path']==path
  assert item['expected_sha256']==previous['expected_sha256']==stage['expected_sha256']==legacy['before_sha256']
  checkpoint=read(R/item['archive'],item['length'],item['sha256']);assert checkpoint==git('show',CHECKPOINT+':'+path)
  assert previous['checkpoint_archive']==item['archive']and previous['checkpoint_length']==item['length']and previous['checkpoint_sha256']==item['sha256']
  prior_raw=read(R/previous['current_archive'],previous['current_length'],previous['current_sha256']);assert restore(prior_raw,previous['inverse'])==checkpoint
  raw=read(R/path);assert raw==git('show',a.expected_head+':'+path),('dirty authorized source',path)
  source_fingerprints[path]=sha(raw)
  archive='docs/evidence/ordinary-plan-final-ingress-20261017/current/'+sha(raw)+'.txt';snapshots[R/archive]=raw
  new.append({'path':path,'expected_sha256':item['expected_sha256'],'current_length':len(raw),'current_sha256':sha(raw),'current_archive':archive,'checkpoint_length':item['length'],'checkpoint_sha256':item['sha256'],'checkpoint_archive':item['archive'],'inverse':inverse(raw,checkpoint)})
 packet=(json.dumps(new,indent=2)+'\n').encode();assert len(packet)<=CAP
 code,n=re.subn(r'const FINAL_INGRESS_LENGTH: usize = [0-9]+;',f'const FINAL_INGRESS_LENGTH: usize = {len(packet)};',helper);assert n==1
 code,n=re.subn(r'const FINAL_INGRESS_SHA256: &str = "[0-9a-f]{64}";',f'const FINAL_INGRESS_SHA256: &str = "{sha(packet)}";',code);assert n==1
 assert scrub(code)==scrub(template)and len(code.splitlines())<=1500
 owners=load(immutable['docs/evidence/ordinary-plan-integrated-e24-v1/current-manifest.json']);assert len(owners['files'])==9
 for row in owners['files']:
  raw=code.encode()if row['path']==spec['helper']else read(R/row['path'])
  if row['path']!=spec['helper']:assert raw==git('show',a.expected_head+':'+row['path']),('dirty PLAN owner',row['path'])
  row.update(sha256=sha(raw),lines=len(raw.splitlines()));assert row['lines']<=1500
 owners.update(base=CHECKPOINT,runtime_verification='UNEXECUTED_SOURCE_ONLY_FINAL_INGRESS')
 manifest=(json.dumps(owners,indent=2)+'\n').encode();assert len(manifest)<=CAP
 assert head()==a.expected_head
 for path,digest in source_fingerprints.items():assert sha(read(R/path))==digest,('source raced',path)
 receipt={'status':'BOUND_SOURCE_ONLY_UNEXECUTED_FINAL_INGRESS'if a.write else'PASS_SOURCE_ONLY_UNEXECUTED_FINAL_INGRESS','head':a.expected_head,'checkpoint':CHECKPOINT,'sources':6,'packet_length':len(packet),'packet_sha256':sha(packet),'runtime':'UNEXECUTED','old_packets_unchanged':True,'authority_refresh_required_after_source_pins':True}
 if a.write:
  snapshots[D/'retained-packets'/('packet-'+sha(old_packet)+'.json')]=old_packet
  for path,raw in snapshots.items():retain(path,raw,True)
  (D/'current-inputs.json').write_bytes(packet);helper_path.write_text(code);(D/'current-manifest.json').write_bytes(manifest);(D/'binding-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
 else:
  assert read(D/'current-inputs.json')==packet and read(helper_path)==code.encode(),'stale final source endpoint'
  assert read(D/'current-manifest.json')==manifest,'stale final owner manifest'
  for path,raw in snapshots.items():retain(path,raw,False)
 assert head()==a.expected_head
 for item in spec['immutable']:read(R/item['path'],item['length'],item['sha256'])
 print(json.dumps(receipt))
if __name__=='__main__':main()
