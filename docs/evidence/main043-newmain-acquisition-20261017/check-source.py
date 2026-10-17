"""Execute only the exact historical Python source-view acquisition controls."""
from pathlib import Path
import hashlib,json,os,runpy,tempfile
ROOT=Path(__file__).resolve().parents[3]
HELPER='docs/evidence/main043-independent-custody-20261016/semantic-receiver-main34d-view.py'
sha=lambda b:hashlib.sha256(b).hexdigest()
module=runpy.run_path(str(ROOT/HELPER));source=module['SOURCE']
raw=(ROOT/source).read_bytes();text=raw.decode('utf8')
assert sha(module['historical'](ROOT,text).encode())=='651d174d04fad97da5c08ad215a869fd395a7d691d4a78dc901a99e814fc8252'
negative=0
def refuses(f):
 global negative
 try:f()
 except (AssertionError,UnicodeError):negative+=1
 else:raise AssertionError('hostile acquisition admitted')
for bad in ['',text+'\n',text[:-1],'!'+text[1:],text.encode()]:
 refuses(lambda bad=bad:module['historical'](ROOT,bad))
refuses(lambda:module['historical'](ROOT.parent,text))
assert os.environ['TMPDIR']=='/Volumes/Semaprax/runtime-tmp'
with tempfile.TemporaryDirectory(prefix='main043-newmain-controls-',dir=os.environ['TMPDIR']) as temporary:
 root=Path(temporary)
 for path in [*module['PINS'],source,HELPER]:
  target=root/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((ROOT/path).read_bytes())
 local=runpy.run_path(str(root/HELPER))
 assert local['historical'](root,text)==module['historical'](ROOT,text)
 p=root/source
 for changed in [raw.replace(b'\n',b'\r\n'),b'!'+raw[1:],raw[:-1]]:
  p.write_bytes(changed)
  # The real outer caller uses read_text; CRLF must not normalize past admission.
  refuses(lambda:local['historical'](root,p.read_text()))
  p.write_bytes(raw)
 for path in module['PINS']:
  p=root/path;before=p.read_bytes();p.write_bytes(before+b'\n')
  refuses(lambda:local['historical'](root,text));p.write_bytes(before)
 p=root/HELPER;before=p.read_bytes();p.write_bytes(before.replace(b'\n',b'\r\n'))
 refuses(lambda:local['historical'](root,text));p.write_bytes(before)
guard=ROOT/'docs/evidence/main043-independent-custody-20261016'
runpy.run_path(str(guard/'guard-extensions.py'))['verify_recipe']()
D=Path(__file__).resolve().parent
proof=json.loads((D/'full-source-inverse.json').read_bytes())
for row in proof['rows']:
 now=(ROOT/row['path']).read_bytes();before=(ROOT/row['before_archive']).read_bytes()
 assert len(now)==row['current_len'] and sha(now)==row['current_sha256']
 assert len(before)==row['before_len'] and sha(before)==row['before_sha256']
for name,expected in proof['original_view_and_outer_bytes_unchanged'].items():
 assert (guard/name).read_bytes()==(D/'before'/name).read_bytes()
 assert sha((guard/name).read_bytes())==expected
old=json.loads((D/'before/guard-extension-recipe-edits.json').read_bytes())
new=json.loads((guard/'guard-extension-recipe-edits.json').read_bytes())
assert new[:-1]==old and len(new)==len(old)+1
print(json.dumps({'status':'PASS_PYTHON_SOURCE_ACQUISITION_ONLY','positive':2,'explicit_negative':negative,'original_historical_negative_controls_also_executed':True,'whole_old_view_and_outer_unchanged':True,'complete_original_recipe_plus_one_edit':True,'global_refresh_or_Rust_execution':False}))
