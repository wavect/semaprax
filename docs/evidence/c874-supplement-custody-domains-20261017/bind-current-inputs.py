"""Bind exactly eight new current endpoints; all historical domains stay fixed."""
from pathlib import Path
import argparse,hashlib,json,os,re,stat,subprocess
D=Path(__file__).resolve().parent;R=D.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
SOURCE='src/source_reflows/semantic_receivers.rs'
VIEWS=[('src/cleanup_plan/replay/factored/canonical_shared/before_paid_many.rs','1bae9e8902fe8ba9e59958308dc779f9bb185da9'),('src/cleanup_plan/replay/factored/canonical_shared.rs','1bae9e8902fe8ba9e59958308dc779f9bb185da9'),('src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs','51c1dfc49962e046566ad21ec31f20c6b3a9bdae'),('src/cleanup_plan/replay/factored/canonical_shared/tests.rs','51c1dfc49962e046566ad21ec31f20c6b3a9bdae'),('src/cleanup_plan/replay/factored/moderate_tests.rs','51c1dfc49962e046566ad21ec31f20c6b3a9bdae'),('src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs','51c1dfc49962e046566ad21ec31f20c6b3a9bdae'),('src/cleanup_plan/replay/factored/canonical_shared/inline_weighted/tests.rs','51c1dfc49962e046566ad21ec31f20c6b3a9bdae')]
def whole(path):
 fd=os.open(path,os.O_RDONLY|getattr(os,'O_NOFOLLOW',0)|getattr(os,'O_NONBLOCK',0))
 try:
  m=os.fstat(fd);assert stat.S_ISREG(m.st_mode)and m.st_size<=2097152,'nonregular or overbound source'
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(m.st_size);assert len(b)==m.st_size and f.read(1)==b''
  after=os.fstat(fd);physical=path.lstat();identity=lambda v:(v.st_dev,v.st_ino,v.st_size,v.st_mtime_ns,v.st_ctime_ns)
  assert not stat.S_ISLNK(physical.st_mode)and identity(m)==identity(after)==identity(physical)
  return b
 finally:os.close(fd)
def retain(path,data):
 try:m=path.lstat()
 except FileNotFoundError:
  fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|getattr(os,'O_NOFOLLOW',0),0o600)
  with os.fdopen(fd,'wb')as f:f.write(data)
 else:assert stat.S_ISREG(m.st_mode)and whole(path)==data,'content address collision/nonregular'
def replace(text,pattern,value):
 text,n=re.subn(pattern,lambda m:value,text);assert n==1;return text
def main():
 p=argparse.ArgumentParser();p.add_argument('--expected-head',required=True);p.add_argument('--write',action='store_true');a=p.parse_args()
 assert re.fullmatch('[0-9a-f]{40}',a.expected_head)
 git=lambda path:subprocess.check_output(['git','show',a.expected_head+':'+path],cwd=R)
 head=lambda:subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()
 assert head()==a.expected_head
 writes={};receiver=json.loads(whole(D/'receiver-input.json'));assert receiver['source']==SOURCE
 assert whole(D/receiver['stage_archive'])==subprocess.check_output(['git','show','49219d230734e000e2884b5b4fb71e61cdf0eb0b:'+SOURCE],cwd=R)
 held=whole(D/receiver['current_archive']);assert len(held)==receiver['current_length']and sha(held)==receiver['current_sha256']
 assert receiver['stage_sha256']=='d19607ffcbca7e4d676a774eb2f21d81293430f6c5985a7afd6409d9bb7589df'
 assert whole(D/receiver['format_archive'])==subprocess.check_output(['git','show','c8740045e33126a4375aadd1dc524a25be9245fc:'+SOURCE],cwd=R)
 assert receiver['format_sha256']=='ba2ddbf8486039ba24fb0f36a90dea66910e931cbb71115b4d2eab114906430b'
 raw=whole(R/SOURCE);assert raw==git(SOURCE)and len(raw)<=65536
 name='receiver.raw-'+sha(raw)+'.rs.txt';writes[D/name]=raw;receiver.update(current_length=len(raw),current_sha256=sha(raw),current_archive=name)
 paid=json.loads(whole(D/'paid-inputs.json'));assert[(x['path'],x['view']['head'])for x in paid]==VIEWS
 for i,row in enumerate(paid):
  held=whole(D/row['current']['archive']);assert len(held)==row['current']['length']and sha(held)==row['current']['sha256']
  view=whole(D/row['view']['archive']);assert len(view)==row['view']['length']and sha(view)==row['view']['sha256'];assert view==subprocess.check_output(['git','show',row['view']['head']+':'+row['path']],cwd=R)
  raw=whole(R/row['path']);assert raw==git(row['path'])and len(raw)<=2097152
  name='paid-current-'+str(i)+'-'+sha(raw)+'.txt';writes[D/name]=raw;row['current']={'archive':name,'length':len(raw),'sha256':sha(raw)}
 rb=(json.dumps(receiver,indent=2)+'\n').encode();pb=(json.dumps(paid,indent=2)+'\n').encode()
 fixed={D/'receiver-input.json':rb,D/'paid-inputs.json':pb}
 path=R/'src/source_reflows_merge/main67107_inputs.rs';s=whole(path).decode();s=replace(s,r'const SUPPLEMENT_RECEIVER_LENGTH: usize = [0-9]+;',f'const SUPPLEMENT_RECEIVER_LENGTH: usize = {len(rb)};');s=replace(s,r'const SUPPLEMENT_RECEIVER_SHA: &str = "[0-9a-f]{64}";',f'const SUPPLEMENT_RECEIVER_SHA: &str = "{sha(rb)}";');fixed[path]=s.encode()
 path=R/'src/cleanup_plan/replay/factored/canonical_shared/paid_many/source.rs';s=whole(path).decode();s=replace(s,r'const PAID_INPUT_LENGTH: usize = [0-9]+;',f'const PAID_INPUT_LENGTH: usize = {len(pb)};');s=replace(s,r'const PAID_INPUT_SHA: &str = "[0-9a-f]{64}";',f'const PAID_INPUT_SHA: &str = "{sha(pb)}";');fixed[path]=s.encode()
 path=D/'format-namespace.py';s=whole(path).decode();s=replace(s,r'META_LENGTH=[0-9]+',f'META_LENGTH={len(rb)}');s=replace(s,r"META_SHA='[0-9a-f]{64}'",f"META_SHA='{sha(rb)}'");fixed[path]=s.encode()
 path=R/'docs/evidence/mainbcc-ordinary-source-domain-20261017/check-source.py';s=whole(path).decode();s=replace(s,r"FORMAT_NAMESPACE_SHA='[0-9a-f]{64}'",f"FORMAT_NAMESPACE_SHA='{sha(fixed[D/'format-namespace.py'])}'");fixed[path]=s.encode()
 path2=R/'docs/evidence/main34d-three-source-domains-20261017/check-source.py';s=whole(path2).decode();pattern=r"assert sha\(bcc_checker.read_bytes\(\)\) == '[0-9a-f]{64}'";s=replace(s,pattern,f"assert sha(bcc_checker.read_bytes()) == '{sha(fixed[path])}'");fixed[path2]=s.encode()
 assert head()==a.expected_head
 if a.write:
  for path,data in writes.items():retain(path,data)
  for path,data in fixed.items():
   if whole(path)!=data:path.write_bytes(data)
 else:
  for path,data in{**writes,**fixed}.items():assert whole(path)==data,('stale eight-input endpoint',str(path))
 assert head()==a.expected_head
 print(json.dumps({'state':'BOUND_EIGHT_SOURCE_ENDPOINTS_ONLY'if a.write else'PASS_EIGHT_SOURCE_ENDPOINTS_ONLY','head':a.expected_head,'runtime':'UNEXECUTED','source_inputs':8,'historical_views_unchanged':True}))
if __name__=='__main__':main()
