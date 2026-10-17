"""Bind only actual-current acquisition endpoints; immutable views never change."""
import argparse, hashlib, json, re, subprocess
from pathlib import Path
D=Path(__file__).resolve().parent
R=D.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--expected-head',required=True);p.add_argument('--write',action='store_true');a=p.parse_args()
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==a.expected_head
 path=D/'input-domains.json';m=json.loads(path.read_bytes())
 assert m['ours_head']=='e031060ce138243b20792779a7f0a0ee4c7d2acb'
 assert m['main_head']=='21abcda9bc82864834914dc5b89c91bc5a9c94a5'
 assert len(m['main'])==148 and len(m['ours'])==9
 changed=[]
 for domain in ['main','ours']:
  seen=set()
  for row in m[domain]:
   assert row['path'] not in seen;seen.add(row['path'])
   assert not Path(row['path']).is_absolute() and '..' not in Path(row['path']).parts
   view=(R/row['archive']).read_bytes()
   assert len(view)==row['view_len'] and sha(view)==row['view_sha256']
   assert view==subprocess.check_output(['git','show',m['main_head' if domain=='main' else 'ours_head']+':'+row['path']],cwd=R)
   raw=(R/row['path']).read_bytes();assert len(raw)<=9382389
   if (row['current_len'],row['current_sha256'])!=(len(raw),sha(raw)):changed.append(row['path'])
   row['current_len']=len(raw);row['current_sha256']=sha(raw)
 data=(json.dumps(m,indent=2)+'\n').encode()
 source=R/'src/source_reflows/incoming21_semantic_receivers.rs';old=source.read_text()
 new,n=re.subn(r'const MAIN21_MANIFEST_LEN: usize = \d+;',f'const MAIN21_MANIFEST_LEN: usize = {len(data)};',old);assert n==1
 new,n=re.subn(r'const MAIN21_MANIFEST_SHA: &str = "[0-9a-f]{64}";',f'const MAIN21_MANIFEST_SHA: &str = "{sha(data)}";',new);assert n==1
 if a.write:path.write_bytes(data);source.write_text(new)
 else:assert path.read_bytes()==data and old==new,'stale current acquisition endpoints'
 print(json.dumps({'status':'BOUND_CURRENT_SOURCE_ONLY' if a.write else 'PASS_CURRENT_SOURCE_ONLY','changed':changed,'manifest_sha256':sha(data),'no_runtime':True}))
if __name__=='__main__':main()
