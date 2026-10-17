"""Bounded Python acquisition/refusal controls only; never invokes the binder run."""
from pathlib import Path
import ast,hashlib,json,tempfile,os,copy,subprocess
D=Path(__file__).resolve().parent;R=D.parents[2];p=D/'bind_current.py';source=p.read_text();tree=ast.parse(source)
# Load definitions without the CLI or run(); all physical controls use a private SSD directory.
body=ast.Module(body=[node for node in tree.body if not isinstance(node,ast.If)],type_ignores=[]);ns={};exec(compile(body,str(p),'exec'),ns)
sha=lambda b:hashlib.sha256(b).hexdigest();data=(D/'binding-descriptor.json').read_bytes();assert len(data)==ns['DESCRIPTOR_LEN'] and sha(data)==ns['DESCRIPTOR_SHA'];d=json.loads(data)
assert d['raw_original_head']=='a82e00cc83f99b16249181c45710d962598577db' and d['cap']==9464242
before=(D/'input-domains.template-7cb.json').read_bytes();assert len(before)==d['template_len'] and sha(before)==d['template_sha256']
assert before==(D/'input-domains.json').read_bytes()
assert len(json.loads(before)['legacy_current'])==149
for row in d['immutable']:
 b=(R/row['path']).read_bytes();assert len(b)==row['length'] and sha(b)==row['sha256']
module=(R/d['module']).read_bytes();assert sha(ns['mask'](module))==d['masked_module_sha256']
count=0
def refuse(fn):
 global count
 try:fn()
 except (ValueError,OSError,AssertionError):count+=1
 else:raise AssertionError('hostile input accepted')
for bad in ('../x','/x','a//b','a/./b','a/../b','') :refuse(lambda bad=bad:ns['parts'](bad))
refuse(lambda:ns['parse'](b'{"a":1,"a":2}'))
refuse(lambda:ns['mask'](module+ b'\nconst MAIN090_MANIFEST_LEN: usize = 1;\n'))
assert sha(ns['mask'](module.replace(b'fn main090_expected(',b'fn foreign_expected(',1)))!=d['masked_module_sha256'];count+=1
with tempfile.TemporaryDirectory(prefix='main090-bind-controls-',dir='/Volumes/Semaprax/runtime-tmp') as folder:
 root=Path(folder);(root/'regular').write_bytes(b'abc');(root/'dir').mkdir();(root/'link').symlink_to(root/'regular');(root/'linked-dir').symlink_to(root/'dir',target_is_directory=True);(root/'dir'/'body').write_bytes(b'abc');os.mkfifo(root/'fifo')
 ns.update(ROOT_FD=os.open(root,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW),R=root,READS={},EXPECTED='a'*40)
 try:
  assert ns['read']('regular',3)[0]==b'abc'
  for name,limit in [('regular',2),('link',3),('dir',3),('fifo',3),('linked-dir/body',3)]:refuse(lambda name=name,limit=limit:ns['read'](name,limit))
  (root/'regular').write_bytes(b'abd');refuse(lambda:ns['read']('regular',3))
  ns['head']=lambda:'b'*40;ns['ROOT_ID']=ns['identity'](os.stat(root));refuse(ns['recheck'])
  # Extra bytes introduced after the pre-buffer size check are refused.
  (root/'growth').write_bytes(b'abc');original_read=os.read;grown=[False]
  def grow(fd,n):
   b=original_read(fd,n)
   if not grown[0]:grown[0]=True;(root/'growth').write_bytes(b'abcd')
   return b
  os.read=grow
  try:refuse(lambda:ns['read']('growth',3))
  finally:os.read=original_read
  # Same-length replacement after EOF changes path identity and fails closed.
  (root/'replace').write_bytes(b'abc');done=[False]
  def replace(fd,n):
   b=original_read(fd,n)
   if n==1 and b==b'' and not done[0]:
    done[0]=True;(root/'replace').unlink();(root/'replace').write_bytes(b'abc')
   return b
  os.read=replace
  try:refuse(lambda:ns['read']('replace',3))
  finally:os.read=original_read
 finally:os.close(ns['ROOT_FD'])
# Exact full prior 33-line recipe remains reconstructible by the receipt.
receipt=json.loads((D/'binding-recipe-inverse.json').read_bytes());assert sha(p.read_bytes())==receipt['after_sha256']
assert receipt['inverse'][0]['after']==source
old=receipt['inverse'][0]['before'].encode();assert old==(D/'bind_current.py.before-7cb.txt').read_bytes() and sha(old)==receipt['before_sha256']
result=dict(status='PASS_SOURCE_ONLY_GUARDED_BINDING_RECIPE',descriptor_sha256=sha(data),immutable_inputs=len(d['immutable']),hostile_refusals=count,original_recipe_lines=len(old.splitlines()),hardened_recipe_lines=len(source.splitlines()),Cargo_or_Rust_runtime=False,actual_final_binding_executed=False)
(D/'binding-recipe-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
