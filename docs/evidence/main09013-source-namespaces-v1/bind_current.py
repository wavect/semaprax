"""Explicit final-source binding only; never regenerates historical namespaces."""
from pathlib import Path
import argparse,hashlib,json,re,subprocess
p=argparse.ArgumentParser();p.add_argument('--root',required=True);p.add_argument('--expected-head',required=True);a=p.parse_args();R=Path(a.root).resolve()
D=R/'docs/evidence/main09013-source-namespaces-v1';sha=lambda b:hashlib.sha256(b).hexdigest()
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R).decode().strip()==a.expected_head
manifest_path=D/'input-domains.json';before=manifest_path.read_bytes();m=json.loads(before)
assert m['table_head']=='8c08e049247582871734136e5dcef0261c947d9b'
assert m['source_body_head']=='09013dbfafbcb37db7c79a0622a56a0707e94e3b'
assert m['cap']==9464242
# Each historical body remains exactly sealed, including all previous MAIN090 tables.
for domain in ('main','expected','legacy_current'):
 for row in m[domain]:
  b=(R/row['archive']).read_bytes();assert len(b)==row['view_len'] and sha(b)==row['view_sha256']
for row in m['prior090_tables']:
 b=(R/row['archive']).read_bytes();assert len(b)==row['length'] and sha(b)==row['sha256']
# Prepare everything before writing; missing/nonregular/symlinked subjects fail closed.
current={}
for row in m['physical']:
 q=R/row['path'];assert q.is_file() and not q.is_symlink(),row['path'];b=q.read_bytes();assert len(b)<=m['cap'];current[row['path']]=b
for domain in ('main','physical'):
 for row in m[domain]:
  b=current[row['path']];row['current_len']=len(b);row['current_sha256']=sha(b)
  if 'binding_status' in row:row['binding_status']='assembled'
m['unassembled']=[];m['bound_final_head']=a.expected_head
new=(json.dumps(m,indent=2)+'\n').encode()
module=R/'src/source_reflows/incoming090_semantic_receivers.rs';text=module.read_text()
text,n=re.subn(r'const MAIN090_MANIFEST_LEN: usize = \d+;',f'const MAIN090_MANIFEST_LEN: usize = {len(new)};',text);assert n==1
text,n=re.subn(r'const MAIN090_MANIFEST_SHA: &str = "[^"]+";',f'const MAIN090_MANIFEST_SHA: &str = "{sha(new)}";',text);assert n==1
(D/('binding-before-'+sha(before)+'.json')).write_bytes(before)
manifest_path.write_bytes(new);module.write_text(text)
print(json.dumps({'head':a.expected_head,'physical_subjects':len(current),'manifest_sha256':sha(new),'status':'BOUND_SOURCE_ONLY_NOT_RUST_QUALIFIED'}))
