"""Bind only three immutable MAIN source-view domains; no Rust/build/authority run."""
from pathlib import Path
import argparse,hashlib,json,subprocess,re
MAIN='34d53510a3374537874c89a0e46be4d68ad9fe61'
BASE='49219d230734e000e2884b5b4fb71e61cdf0eb0b'
REL=Path('docs/evidence/main34d-three-source-domains-20261017')
CAP=9_317_546
DOMAINS=[('held-status','held-status-prefix-v1',3,6),('held-continuation','held-continuation-growth-v1',2,10),('receiving-lookup','paid-expression-lookup-state-v1',4,10)]
META=['docs/evidence/compact-staged-observations-v1/inverse.json','docs/evidence/ordinary-replay-setup-v1/sources.json','docs/evidence/prefix-certified-expression-lookup/exact-source-inverses.json','docs/evidence/receiver-source-custody-v1/inverse.json']
sha=lambda b:hashlib.sha256(b).hexdigest()
def unique(pairs):
 d={}
 for k,v in pairs:assert k not in d,('duplicate field',k);d[k]=v
 return d
def load(b):return json.loads(b,object_pairs_hook=unique)
def git(repo,*a):return subprocess.check_output(['git','-C',str(repo),*a])
def queue_current(files,repo,current):
 # Content addressing preserves each admitted endpoint; never overwrite an archive.
 archive=REL/'current-inputs'/('meta-'+sha(current)+'.txt')
 if (repo/archive).exists():assert (repo/archive).read_bytes()==current,('reused current archive differs',str(archive))
 if archive in files:assert files[archive]==current
 files[archive]=current
 return archive
def main():
 p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--inputs',type=Path,required=True);p.add_argument('--production-overlay',type=Path);p.add_argument('--write',action='store_true');a=p.parse_args();R=a.repo.resolve();I=a.inputs.resolve();D=R/REL
 assert R.is_relative_to('/Volumes/Semaprax') and I.is_relative_to('/Volumes/Semaprax')
 if a.production_overlay:assert a.production_overlay.resolve().is_relative_to('/Volumes/Semaprax')
 observed_head=git(I,'rev-parse','HEAD')
 def old(path):return git(R,'show',MAIN+':'+path)
 original={};domains=[];ordered=[]
 for name,directory,nv,nw in DOMAINS:
  reads=[]
  for table,count in [('views.json',nv),('sources.json',nw)]:
   path='docs/evidence/'+directory+'/'+table;b=old(path);assert (I/path).read_bytes()==b,('immutable MAIN table changed',path);rows=load(b);assert len(rows)==count;original[path]=b;reads.append(path)
   for row in rows:
    path=row['path'];b=old(path);assert sha(b)==row['after_sha256'];original[path]=b;reads.append(path)
    archive=row['archive'];b=old(archive);assert sha(b)==row['before_sha256'];assert (I/archive).read_bytes()==b,('immutable MAIN archive changed',archive);original[archive]=b;reads.append(archive)
  reads=list(dict.fromkeys(reads));domains.append({'name':name,'reads':reads});ordered.extend(reads)
 # Reading the exact original MAIN14 domain is separate from OURS5.
 proofs=[]
 for path,count in [('docs/evidence/receiver-source-custody-v1/inverse.json',12),('docs/evidence/live-source-audit-custody-v1/inverse.json',2)]:
  b=old(path);assert len(load(b))==count;proofs.append((path,b))
 (D/'main-inputs').mkdir(exist_ok=True);(D/'current-inputs').mkdir(exist_ok=True)
 files={};entries=[]
 allowed_overlay={path for path in ordered if path.startswith('src/')}
 for index,path in enumerate(dict.fromkeys(ordered)):
  expected=original[path]
  if path=='src/source_reflows/semantic_receivers.rs':current=(R/path).read_bytes()
  elif a.production_overlay and path in allowed_overlay and (a.production_overlay/path).is_file():current=(a.production_overlay/path).read_bytes()
  else:current=(I/path).read_bytes()
  assert b'<<<<<<<' not in current,('unresolved input',path)
  assert len(current)<=CAP and len(expected)<=CAP,('fixture hardcap',path)
  current.decode();expected.decode()
  if path in META:
   stem=path.replace('/','__');parent=Path('docs/evidence/main34d535-integration-20261017')
   main_archive=parent/(stem+'.main34d.json');current_archive=parent/(stem+'.ours-before.json')
   assert (I/main_archive).read_bytes()==expected
   snapshot=(I/current_archive).read_bytes()
   assert snapshot==git(R,'show',BASE+':'+path),('immutable merge snapshot changed',path)
   if current!=snapshot:current_archive=queue_current(files,R,current)
  elif path.startswith('docs/evidence/') and current==expected:
   main_archive=current_archive=Path(path)
  else:
   main_archive=REL/'main-inputs'/f'{index:02}.txt';current_archive=REL/'current-inputs'/f'{index:02}.txt';files[main_archive]=expected;files[current_archive]=current
  entry={'path':path,'current_len':len(current),'current_sha256':sha(current),'current_archive':str(current_archive),'main_len':len(expected),'main_sha256':sha(expected),'main_archive':str(main_archive),'whole_source_inverse':[{'at':0,'remove_len':len(current),'remove_sha256':sha(current),'insert_archive':str(main_archive)}]}
  entries.append(entry)
 manifest={'schema':'semaprax.main34d-three-source-domains.v1','main_head':MAIN,'domains':domains,'inputs':entries}
 for name,(path,b) in zip(['main_receiver_proofs','main_live_audits'],proofs):
  if path in META:archive=Path('docs/evidence/main34d535-integration-20261017')/(path.replace('/','__')+'.main34d.json');assert (I/archive).read_bytes()==b
  else:archive=Path(path);assert (I/archive).read_bytes()==b
  manifest[name]={'archive':str(archive),'len':len(b),'sha256':sha(b)}
 assert git(I,'rev-parse','HEAD')==observed_head,'input HEAD changed during binding'
 data=(json.dumps(manifest,indent=2)+'\n').encode();assert len(data)<65536
 receipt={'status':'BOUND_SOURCE_ONLY_UNQUALIFIED_UNTIL_FINAL_REGISTERED_BUILD','inputs_observed_head':git(I,'rev-parse','HEAD').decode().strip(),'main_head':MAIN,'manifest_sha256':sha(data),'manifest_bytes':len(data),'unique_inputs':len(entries),'domain_counts':[(x[0],x[2],x[3])for x in DOMAINS],'archive_writes':len(files),'all_original_MAIN_tables_and_archive_hashes_verified':True,'original_domain_order_and_duplicate_source_rows_unchanged':True,'whole_source_inverse':'One closed whole-input replacement, with complete current/predecessor preimages and both full lengths/SHA256. No old table/assertion retargeting.','Rust_or_Cargo_or_authority_or_common_refresh':False}
 if a.write:
  for path,b in files.items():
   if path.name.startswith('meta-') and (R/path).exists():assert (R/path).read_bytes()==b,('reused current archive differs',str(path))
   (R/path).write_bytes(b)
  (D/'input-manifest.json').write_bytes(data);(D/'binding-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
  helper=R/'src/source_reflows/incoming67107_semantic_receivers.rs';text=helper.read_text()
  text,n=re.subn(r'const MAIN34D_MANIFEST_LEN: usize = [0-9]+;',f'const MAIN34D_MANIFEST_LEN: usize = {len(data)};',text);assert n==1
  text,n=re.subn(r'const MAIN34D_MANIFEST_SHA: &str = \"[0-9a-f]{64}\";',f'const MAIN34D_MANIFEST_SHA: &str = \"{sha(data)}\";',text);assert n==1
  helper.write_text(text)
  # Rebind only the authored outer endpoint; immutable complete original programs stay fixed.
  inverse_path=D/'source-inverse.json';inverse=load(inverse_path.read_bytes())
  for row in inverse:
   before=(R/row['original_archive']).read_bytes();assert len(before)==row['original_len'] and sha(before)==row['original_sha256']
   current=(R/row['path']).read_bytes();row['current_len']=len(current);row['current_sha256']=sha(current)
   row['inverse']=[{'at':0,'remove_len':len(current),'remove_sha256':sha(current),'insert_archive':row['original_archive']}]
  inverse_path.write_text(json.dumps(inverse,indent=2)+'\n')
 else:
  assert (D/'input-manifest.json').read_bytes()==data,'endpoint binding is stale'
  for path,b in files.items():assert (R/path).read_bytes()==b
 print(json.dumps(receipt))
if __name__=='__main__':main()
