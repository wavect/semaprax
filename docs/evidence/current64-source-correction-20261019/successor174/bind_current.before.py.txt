"""One-shot, explicit-HEAD, finite historical source binding; no compiler runs."""
from pathlib import Path,PurePosixPath
from collections import Counter
import argparse,copy,hashlib,json,os,re,stat,subprocess,secrets
DESCRIPTOR='docs/evidence/main09013-source-namespaces-v1/binding-descriptor.json'
DESCRIPTOR_LEN=121365
DESCRIPTOR_SHA='c98b31327f8f5fb88d4ee36f1b457d9f18339789abe528efb5aa18d9b652619c'
LIMIT=9464242
sha=lambda b:hashlib.sha256(b).hexdigest()
def require(condition,message):
 if not condition:raise ValueError(message)
def unique(pairs):
 result={}
 for k,v in pairs:require(k not in result,'duplicate JSON key');result[k]=v
 return result
def parse(b):return json.loads(b,object_pairs_hook=unique)
def parts(path):
 p=PurePosixPath(path)
 require(isinstance(path,str) and not p.is_absolute() and p.parts and all(x not in ('','.','..') for x in p.parts) and str(p)==path,'foreign relative path')
 return p.parts
def identity(s):return(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
def directory(path):
 fd=os.dup(ROOT_FD)
 try:
  for part in parts(path):
   new=os.open(part,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC,dir_fd=fd);os.close(fd);fd=new
  return fd
 except BaseException:os.close(fd);raise
def parent(path):
 p=parts(path);return(directory('/'.join(p[:-1])) if len(p)>1 else os.dup(ROOT_FD),p[-1])
READS={}
def read(path,limit=LIMIT,remember=True):
 require(0<=limit<=LIMIT,'foreign read bound');fd,name=parent(path)
 try:
  before=os.stat(name,dir_fd=fd,follow_symlinks=False)
  require(stat.S_ISREG(before.st_mode) and before.st_size<=limit,'nonregular or oversized input: '+path)
  handle=os.open(name,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC|os.O_NONBLOCK,dir_fd=fd)
  try:
   opened=os.fstat(handle);require(identity(before)==identity(opened),'open identity drift: '+path)
   chunks=[];remaining=opened.st_size
   while remaining:
    b=os.read(handle,min(65536,remaining));require(bool(b),'truncated input: '+path);chunks.append(b);remaining-=len(b)
   require(os.read(handle,1)==b'','extra input: '+path)
   after=os.fstat(handle);named=os.stat(name,dir_fd=fd,follow_symlinks=False)
   require(identity(opened)==identity(after)==identity(named),'post-read identity drift: '+path)
   b=b''.join(chunks)
  finally:os.close(handle)
 finally:os.close(fd)
 record=(len(b),sha(b),identity(after))
 if remember:
  require(path not in READS or READS[path]==record,'input changed between reads: '+path);READS[path]=record
 return b,record

def git(*args):return subprocess.check_output(['git',*args],cwd=R)
def head():return git('rev-parse','HEAD').decode().strip()
def git_bytes(path):
 parts(path);subject=EXPECTED+':'+path
 entry=git('ls-tree','-z',EXPECTED,'--',path)
 require(entry.endswith(b'\0') and entry.count(b'\0')==1,'nonunique Git entry')
 header,name=entry[:-1].split(b'\t',1);mode,kind,oid=header.split()
 require(name==path.encode() and mode in (b'100644',b'100755') and kind==b'blob','nonregular Git subject')
 kind=git('cat-file','-t',subject).decode().strip();size=int(git('cat-file','-s',subject))
 require(kind=='blob' and 0<=size<=LIMIT,'foreign Git subject: '+path)
 # Git blobs are immutable. Bound stdout by the size checked before acquisition.
 proc=subprocess.Popen(['git','cat-file','blob',subject],cwd=R,stdout=subprocess.PIPE)
 try:
  data=proc.stdout.read(size+1);require(len(data)==size,'Git blob length');require(proc.wait()==0,'Git blob read')
 finally:
  proc.stdout.close()
  if proc.poll() is None:proc.kill();proc.wait()
 return data

def current(path):
 b,_=read(path);require(b==git_bytes(path),'physical subject differs from explicit Git HEAD: '+path);return b

def mask(body):
 for field,value in (('LEN',b'<MANIFEST_LENGTH>'),('SHA',b'<MANIFEST_SHA256>')):
  pattern=(rb'const MAIN090_MANIFEST_LEN: usize = [0-9]+;' if field=='LEN' else rb'const MAIN090_MANIFEST_SHA: &str = "[0-9a-f]{64}";')
  body,n=re.subn(pattern,value,body);require(n==1,'nonunique namespace pin '+field)
 return body

def fragments(path,current,original):
 # Macro bodies are indivisible. Non-macro characters remain exact anchors;
 # whitespace between anchors can belong to an added run only if removing
 # that complete run retains the exact old whitespace (not a normalized view).
 macro=re.compile(r'include_str!\("[^"\n]+"\),')
 grammar=r'(?:\s*include_str!\("[^"\n]+"\),)+\s*'
 def tokens(text):
  end=0
  for match in macro.finditer(text):
   for character in text[end:match.start()]:yield(False,character)
   yield(True,match.group());end=match.end()
  for character in text[end:]:yield(False,character)
 def anchors(text):
  space=[]
  for token in tokens(text):
   if not token[0] and token[1].isspace():space.append(token[1]);continue
   yield(''.join(space),token);space=[]
  yield(''.join(space),None)
 def addition(before,after):
  if before==after:return
  delta=len(after)-len(before);assert delta>0,('non-additive original guard edit',path)
  # A gap contains only old whitespace and new complete macros. Choose the
  # longest exact retained prefix; every candidate restores the same raw gap.
  # This resolves whitespace ownership without splitting any macro token.
  for prefix in range(len(before),-1,-1):
   new=after[prefix:prefix+delta]
   if after[:prefix]+after[prefix+delta:]==before and re.fullmatch(grammar,new):
    out.append(new);return
  raise AssertionError(('foreign additive fragment or changed original whitespace',path))
 left=iter(anchors(original));space,expected=next(left);out=[];added=[]
 for token in tokens(current):
  if not token[0] and token[1].isspace():added.append(token[1]);continue
  if token==expected:
   addition(space,''.join(added));added=[];space,expected=next(left)
  else:
   assert token[0],('non-additive original guard edit',path)
   added.append(token[1])
 assert expected is None,('missing original guard token',path)
 addition(space,''.join(added));restored=current
 for new in out:
  assert re.fullmatch(grammar,new),('foreign additive fragment',path)
  assert current.count(new)==1,('nonunique additive fragment',path)
  assert restored.count(new)==1,('overlapping additive fragment',path)
  restored=restored.replace(new,'',1)
 assert restored==original,('original raw guard changed',path)
 return out


def raw_guards(descriptor):
 allowed_path=descriptor['allowed_path'];allowed=parse(current(allowed_path));baseline=descriptor['prior_allowed']
 require(set(allowed)==set(baseline),'foreign guard carriers')
 for carrier in descriptor['raw3']:
  path=carrier['path'];raw=current(path).decode();old=git('show',descriptor['raw_original_head']+':'+path)
  require(sha(old)==carrier['original_sha256'],'original raw scope drift');original=old.decode()
  additions=fragments(path,raw,original);base=PurePosixPath(path).parent if path=='tests/hir_module_boundaries.rs' else PurePosixPath(path).parent.parent
  counts=Counter()
  for added in additions:
   for literal in re.findall(r'include_str!\("([^"\n]+)"\)',added):
    joined=os.path.normpath(str(base/literal));parts(joined);counts[joined]+=1
  rule=allowed[path]
  require(set(rule['required'])<=set(rule['maximum']),'required guard subjects lack maxima')
  for kind in ('required','maximum'):
   for k,v in baseline[path][kind].items():require(rule[kind].get(k)==v,'prior guard count changed: '+path+' '+k)
  require(set(counts)<=set(rule['maximum']),'foreign raw include')
  require(all(rule['required'].get(k,0)<=counts[k]<=v for k,v in rule['maximum'].items()),'missing/duplicate raw source')
  owner=descriptor['module'];require(counts[owner]==rule['required'][owner]==rule['maximum'][owner]==1,'namespace must appear exactly once in each original scope')
  if 'ledger_capacity' in path:require(len(raw.splitlines())<=1500,'native source inventory line bound')
  binding_path=descriptor['guard_directory']+'/guard-extension-bindings/'+path.replace('/','__')+'.json'
  expected=dict(source=path,current_sha256=sha(raw.encode()),original_sha256=sha(old),added_fragments=additions)
  require(parse(current(binding_path))==expected,'stale raw scope binding: '+path)

def recheck():
 require(head()==EXPECTED,'Git HEAD drifted before publication')
 require(identity(os.stat(R,follow_symlinks=False))==ROOT_ID,'root identity drifted')
 for path,record in list(READS.items()):
  _,now=read(path,remember=False);require(now==record,'source identity or bytes changed before publication: '+path)
 require(head()==EXPECTED,'Git HEAD drifted during final reads')

def stage(path,data):
 fd,name=parent(path);temp='.'+name+'.main090-'+secrets.token_hex(12)
 handle=None
 try:
  handle=os.open(temp,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW|os.O_CLOEXEC,0o600,dir_fd=fd)
  offset=0
  while offset<len(data):offset+=os.write(handle,data[offset:])
  os.fchmod(handle,0o644);os.fsync(handle);os.close(handle);handle=None
  return fd,name,temp
 except BaseException:
  if handle is not None:os.close(handle)
  try:os.unlink(temp,dir_fd=fd)
  except FileNotFoundError:pass
  os.close(fd);raise

def run():
 descriptor_bytes,_=read(DESCRIPTOR,DESCRIPTOR_LEN)
 require(len(descriptor_bytes)==DESCRIPTOR_LEN and sha(descriptor_bytes)==DESCRIPTOR_SHA,'immutable descriptor drift')
 require(descriptor_bytes==git_bytes(DESCRIPTOR),'descriptor differs from selected HEAD')
 d=parse(descriptor_bytes);require(d['cap']==LIMIT and d['table_head']=='8c08e049247582871734136e5dcef0261c947d9b','historical namespace changed')
 template=current(d['template']);require(len(template)==d['template_len'] and sha(template)==d['template_sha256'],'immutable template drift')
 before=current(d['manifest']);require(before==template,'one-shot binder requires exact original template')
 current(d['binder'])
 m=parse(template);require(m['table_head']==d['table_head'] and m['source_body_head']==d['source_body_head'] and m['cap']==LIMIT,'template namespace mismatch')
 require(len(m['legacy_current'])==149 and len(m['expected'])==3 and len(m['main'])==173,'finite domain cardinality')
 for row in d['immutable']:
  b=current(row['path']);require(len(b)==row['length'] and sha(b)==row['sha256'],'historical full input drift: '+row['path'])
 source=current(d['module']);require(sha(mask(source))==d['masked_module_sha256'],'namespace body changed beyond two pins')
 require(('const MAIN090_MANIFEST_LEN: usize = '+str(len(before))+';').encode() in source and ('const MAIN090_MANIFEST_SHA: &str = "'+sha(before)+'";').encode() in source,'old module pins do not authenticate template')
 raw_guards(d)
 physical={}
 for row in m['physical']:
  require(row['path'] not in physical,'duplicate physical subject');b=current(row['path']);physical[row['path']]=(len(b),sha(b))
 require(set(row['path'] for row in m['main'])<=set(physical),'missing physical domain')
 for domain in ('main','physical'):
  for row in m[domain]:
   row['current_len'],row['current_sha256']=physical[row['path']]
   if 'binding_status' in row:row['binding_status']='assembled'
 m['unassembled']=[];m['bound_final_head']=EXPECTED
 after=(json.dumps(m,indent=2)+'\n').encode();require(len(after)<=LIMIT,'new manifest bound')
 updated,n=re.subn(rb'const MAIN090_MANIFEST_LEN: usize = [0-9]+;',('const MAIN090_MANIFEST_LEN: usize = '+str(len(after))+';').encode(),source);require(n==1,'length pin')
 updated,n=re.subn(rb'const MAIN090_MANIFEST_SHA: &str = "[0-9a-f]{64}";',('const MAIN090_MANIFEST_SHA: &str = "'+sha(after)+'";').encode(),updated);require(n==1 and mask(updated)==mask(source),'only two exact pins may change')
 archive=d['directory']+'/binding-before-'+sha(before)+'.json';fd,name=parent(archive)
 try:
  try:os.stat(name,dir_fd=fd,follow_symlinks=False)
  except FileNotFoundError:pass
  else:raise ValueError('immutable prior-manifest archive already exists')
 finally:os.close(fd)
 # All bounded reads, Git equality, historical seals and scope checks finish first.
 recheck();prepared=[]
 try:
  prepared.append(stage(d['manifest'],after))
  prepared.append(stage(d['module'],updated))
  recheck()
  fd,name=parent(archive)
  try:
   handle=os.open(name,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW|os.O_CLOEXEC,0o644,dir_fd=fd)
   try:
    offset=0
    while offset<len(before):offset+=os.write(handle,before[offset:])
    os.fsync(handle)
   finally:os.close(handle)
  finally:os.close(fd)
  for fd,name,temp in prepared:os.replace(temp,name,src_dir_fd=fd,dst_dir_fd=fd);os.fsync(fd)
 finally:
  for fd,name,temp in prepared:
   try:os.unlink(temp,dir_fd=fd)
   except FileNotFoundError:pass
   os.close(fd)
 print(json.dumps({'status':'BOUND_SOURCE_ONLY_NOT_RUST_QUALIFIED','head':EXPECTED,'physical_subjects':len(physical),'manifest_sha256':sha(after),'previous_manifest_archive':archive,'persistent_mutations':[d['manifest'],d['module'],archive]}))

if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--root',required=True);p.add_argument('--expected-head',required=True);args=p.parse_args()
 require(re.fullmatch(r'[0-9a-f]{40}',args.expected_head) is not None,'expected HEAD must be full 40 lowercase hex digits')
 require(__debug__,'optimized Python disables original fragment assertions')
 R=Path(args.root).absolute();require(R==R.resolve(),'root must have no symlink components');EXPECTED=args.expected_head
 require(hasattr(os,'O_NOFOLLOW') and hasattr(os,'O_DIRECTORY'),'no-follow platform required')
 ROOT_FD=os.open(R,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC);ROOT_ID=identity(os.fstat(ROOT_FD))
 try:require(head()==EXPECTED,'explicit Git HEAD mismatch');run()
 finally:os.close(ROOT_FD)
