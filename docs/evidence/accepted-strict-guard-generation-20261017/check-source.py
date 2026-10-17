"""Only finite strict controls; no common/verifier/Rust execution."""
from pathlib import Path
import runpy,json,subprocess,hashlib,os,stat
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
def bounded(path,length,digest):
 assert 0<=length<=262144 and hasattr(os,'O_NOFOLLOW')
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 identity=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  first=os.fstat(fd);assert stat.S_ISREG(first.st_mode) and first.st_size==length
  with os.fdopen(fd,'rb',closefd=False)as stream:
   raw=stream.read(length);assert len(raw)==length and not stream.read(1)
  after=os.fstat(fd);physical=os.lstat(path)
  assert stat.S_ISREG(physical.st_mode) and identity(first)==identity(after)==identity(physical)
  assert hashlib.sha256(raw).hexdigest()==digest
  raw.decode('utf8');return raw
 finally:os.close(fd)
helper=R/'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';helper_raw=bounded(helper,19974,'b00eedd6a4aa651b02141cbff8d2e1a33ffe8f594406058b61b764f733b5194f')
m={'__file__':str(helper),'_AUTHENTICATED_PROGRAM':helper_raw};exec(compile(helper_raw,str(helper),'exec'),m)
new=m['accepted_guard_extensions'](R)
original_path=R/'docs/evidence/main043-independent-custody-20261016/guard-extensions.py'
original_raw=bounded(original_path,5113,'ccfb62c8b30e5564446ddc6eb2a31f5a7502e5ed43d3d5077a8c0105c42ef95a')
old_source=original_raw.decode();old_read="(D/'verify-or-refresh.py').read_text()";new_read='(_next_public_previous())'
assert old_source.count(old_read)==1;old_current=old_source.replace(old_read,new_read,1)
assert old_current.replace(new_read,old_read,1).encode()==original_raw
old={'__file__':str(original_path),'_next_public_previous':new['public_previous']};exec(compile(old_current,str(original_path),'exec'),old)
allowed=json.loads((D/'current-allowed.json').read_text());count=0
old['verify_recipe']()
def refuses(f):
 global count
 try:f()
 except (AssertionError,ValueError):count+=1;return
 raise AssertionError('hostile guard admitted')
for path in allowed:
 raw=(R/path).read_text();original=subprocess.check_output(['git','show','a82e00cc8:'+path],cwd=R).decode()
 new['verify_guard'](path,raw,original)
 before=subprocess.check_output(['git','show','5f5808c6cf89a29388f25056baee2e7bd5f0dd08:'+path],cwd=R).decode()
 old['verify_guard'](path,before,original)
 archive=D/'before'/(path.replace('/','__')+'.txt');assert archive.read_text()==before
 refuses(lambda:old['verify_guard'](path,raw,original))
 record=new['verify_guard'](path,raw,original);fragment=record['added_fragments'][0]
 for bad in [raw.replace(fragment,'',1),raw+fragment,raw+'include_str!("/foreign.rs"),']:
  refuses(lambda:new['verify_guard'](path,bad,original))
 if path==m['CARRIER']:
  for name in m['SCRATCH']:
   literal='include_str!("../../'+name+'")'
   for bad in [raw+literal+',',raw.replace(literal+',','',1),raw.replace(literal+',','',1)+literal+',']:
    refuses(lambda:new['verify_guard'](path,bad,original))
   first=raw.index(literal);second=raw.index(literal,first+len(literal))
   for at in (first,second):
    missing=raw[:at]+raw[at+len(literal)+1:]
    refuses(lambda:new['verify_guard'](path,missing,original))
    moved=missing.replace('(capacity) => { audit_source_join!(', '(capacity) => { audit_source_join!('+literal+',',1)
    refuses(lambda:new['verify_guard'](path,moved,original))
  previous=json.loads((D/'before/guard-extension-allowed.json').read_text())[path]['maximum']
  added=set(allowed[path]['maximum'])-set(previous)
  for name in added:
   literal='include_str!("../../'+name+'")'
   refuses(lambda:new['verify_guard'](path,raw+literal+',',original))
W=R/'docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py'
s=W.read_text();prefix=s.split("exec(compile(current,str(P),'exec')",1)[0];space={'__file__':str(W)};exec(compile(prefix,str(W),'exec'),space)
assert space['current'].replace(space['guard_new'],space['guard_old']).replace(space['new'],space['old'],1).encode()==space['original']
result={'status':'PASS_SOURCE_ONLY_STRICT_AND_BOOTSTRAP','complete_current_carriers':3,'complete_original_guard_carriers':3,'old_maximums_unchanged':True,'refusals':count,'original_verifier_executed':False,'rust_executed':False}
(D/'source-controls.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
