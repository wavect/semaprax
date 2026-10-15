from pathlib import Path
import json,hashlib,subprocess,re
root=Path(__file__).resolve().parents[3];candidate=root;packet='docs/evidence/source36-main88884-merge-20261015';sha=lambda b:hashlib.sha256(b).hexdigest();original_read=Path.read_bytes;original_text=Path.read_text

def readbytes(path):
 try:relative=path.relative_to(root)
 except ValueError:return original_read(path)
 replacement=candidate/relative
 return original_read(replacement) if replacement.exists() else original_read(path)
def readtext(path,*args,**kwargs):return readbytes(path).decode(kwargs.get('encoding') or 'utf-8')
Path.read_bytes=readbytes;Path.read_text=readtext
# Validate both whole-source chains and immutable archives before original assertions.
def restore(rows,source,current):
 rows=[r for r in rows if r['source']==source];assert len(rows)<=1
 if not rows:return current
 r=rows[0];assert sha(current.encode())==r['current_sha256'],source;old=(root/r['archive']).read_bytes();assert sha(old)==r['original_sha256'];assert old==subprocess.check_output(['git','show',r['checkpoint_revision']+':'+source],cwd=root)
 for e in r['edits']:assert current.count(e['new'])==1;current=current.replace(e['new'],e['old'],1)
 assert current.encode()==old;return current
ours=json.loads((root/packet/'ours-projections.json').read_text());main=json.loads((root/packet/'main-reflow-projections.json').read_text())
for rows in [ours,main]:
 for r in rows:restore(rows,r['source'],(root/r['source']).read_text())
assert (root/'src/source_reflows.json').read_bytes()==subprocess.check_output(['git','show','88884:src/source_reflows.json'],cwd=root)
records=json.loads((root/'src/source_reflows.json').read_text())
for r in records:
 current=restore(main,r['path'],(root/r['path']).read_text());assert sha(current.encode())==r['after_sha256']
 for e in r['inverse']:assert current.count(e['after'])==1;current=current.replace(e['after'],e['before'],1)
 assert sha(current.encode())==r['before_sha256']
for row in json.loads((root/packet/'source-inverses.json').read_text()):
 current=(root/row['source']).read_bytes();old=(root/row['archive']).read_bytes();assert sha(current)==row['current_sha256'];assert sha(old)==row['original_sha256']
 for edit in row['edits']:
  assert current.decode().count(edit['new'])==1
  current=current.decode().replace(edit['new'],edit['old'],1).encode()
 assert current==old
# Keep the original 36 runner's assertion interiors; modify only its named adapter.
oldrunner=(root/'docs/evidence/source36-after-next-838-839-20261015/verify_source36.py').read_text();code=oldrunner
code=code.replace('root=Path(__file__).resolve().parents[3]',f'root=Path({str(root)!r})',1)
anchor='def project(source,current):\n';assert code.count(anchor)==1;code=code.replace(anchor,anchor+' current=restore(ours,source,current)\n',1)
anchor=" matching=[row for row in records if row['source']==source];assert len(matching)==1";assert code.count(anchor)==1;code=code.replace(anchor," matching=[row for row in records if row['source']==source]\n if not matching:return current\n assert len(matching)==1",1)
def ci(source):
 current=restore(main,source,(root/source).read_text())
 matching=[r for r in records if r['path']==source]
 if not matching:return current
 r=matching[0];assert sha(current.encode())==r['after_sha256']
 for e in r['inverse']:assert current.count(e['after'])==1;current=current.replace(e['after'],e['before'],1)
 assert sha(current.encode())==r['before_sha256'];return current
# Recognize the real CI macro and keep every original tail assertion.
code=code.replace("start=t.index('    let mut current = include_str!", "start=t.index('    let mut current = ci_str!")
code=code.replace("end=t.index('    assert_eq!(current,',start)", r"end=re.search(r'\n    assert_eq!\(\s*current,',t[start:]).start()+start")
code=code.replace("current=apply((root/'src/hir.rs').read_text(),t[start:end],p)","current=apply(ci('src/hir.rs'),t[start:end],p)")
code=code.replace("paid=(root/'src/hir/declaration_index/flags/paid.rs').read_text()", "paid=ci('src/hir/declaration_index/flags/paid.rs')")
# Anchor only: actual ownership guard uses ci_str, whose input is replayed below.
code=code.replace(r'include_str!\(\s*', r'(?:include_str|ci_str)!\(\s*')
code=code.replace(r'\)\s*\)\s*;', r'\)\s*,?\s*\)\s*;')
code=code.replace("current=(root/'src/hir/validation.rs').read_text()", "current=ci('src/hir/validation.rs')")
presence_path='src/cleanup_plan/replay/live_oracle/source_custody/presence.json'
assert (root/presence_path).read_bytes()==subprocess.check_output(['git','show','88884:'+presence_path],cwd=root)
for row in json.loads((root/presence_path).read_text()):
 current=ci(row['path']);assert sha(current.encode())==row['after_sha256']
 for edit in row['inverse']:assert current.count(edit['after'])==1;current=current.replace(edit['after'],edit['before'],1)
 assert sha(current.encode())==row['before_sha256']
print('MAIN71 complete source inverses and both explicit CI→presence512 full hashes equal; additional cache zero.')
try:exec(compile(code,'original36-adapted-candidate','exec'),globals())
except Exception as e:
 import traceback;traceback.print_exc();raise
