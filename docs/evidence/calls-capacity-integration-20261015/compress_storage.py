"""Lossless deterministic storage of only this integration's new conflict inputs."""
from pathlib import Path
import gzip,hashlib,json,subprocess
r=Path.cwd();p=r/'docs/evidence/calls-capacity-integration-20261015';basis='cb7aeb1a12e11306ebf01ac08abca4008f6fa792';chunk=1024*1024
assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()==basis

def digest(path):
 h=hashlib.sha256();n=0
 with path.open('rb') as f:
  while b:=f.read(chunk):h.update(b);n+=len(b)
 return h.hexdigest(),n

def compress(source,destination,kind):
 name=str(source.relative_to(r));oid=subprocess.check_output(['git','rev-parse',basis+':'+name],text=True).strip()
 destination.parent.mkdir(parents=True,exist_ok=True)
 with source.open('rb') as a,destination.open('wb') as raw:
  with gzip.GzipFile(filename='',mode='wb',fileobj=raw,mtime=0,compresslevel=6) as z:
   while b:=a.read(chunk):z.write(b)
 h=hashlib.sha256();count=0
 with source.open('rb') as a,gzip.open(destination,'rb') as z:
  while b:=a.read(chunk):
   assert z.read(len(b))==b,(name,'roundtrip');h.update(b);count+=len(b)
  assert z.read(1)==b''
 compressed,size=digest(destination)
 return {'original_path':name,'stored_path':str(destination.relative_to(r)),'uncompressed_sha256':h.hexdigest(),'uncompressed_bytes':count,'compressed_sha256':compressed,'compressed_bytes':size,'original_git_blob_id':oid,'original_git_revision':basis,'kind':kind,'gzip_mtime':0,'gzip_filename':''}

rows=[]
for source in sorted((p/'conflict-inputs').rglob('*')):
 if not source.is_file():continue
 assert source.suffix=='.json',source
 row=compress(source,Path(str(source)+'.gz'),'new_conflict_input');rows.append(row);source.unlink()
old=p/'verify_retained_branches.py';rows.append(compress(old,p/'archive-storage-before/verify_retained_branches.py.gz','immediate_prior_verifier'))
index={'status':'LOSSLESS_SOURCE_STORAGE_ONLY','original_revision':basis,'codec':'gzip','compression_level':6,'mtime':0,'filename':'','chunk_bytes':chunk,'entries':rows}
(p/'compressed-storage-index.json').write_text(json.dumps(index,indent=2)+'\n')
print(json.dumps({'retained_payloads':len(rows)-1,'uncompressed_bytes':sum(x['uncompressed_bytes'] for x in rows),'compressed_bytes':sum(x['compressed_bytes'] for x in rows),'all_roundtrips':'PASS'}))
