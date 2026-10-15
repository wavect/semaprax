"""Independently prove retained incoming metadata histories from immutable Git."""
from pathlib import Path
import atexit,gzip,hashlib,json,subprocess
r=Path.cwd();p=r/'docs/evidence/calls-capacity-integration-20261015';sha=lambda b:hashlib.sha256(b).hexdigest()
index=json.loads((p/'compressed-storage-index.json').read_text())
storage={row['original_path']:row for row in index['entries']};verified=[];chunk=index['chunk_bytes'];largest_blob=0
# One persistent Git protocol stream; no decoded or raw blob cache exists.
git=subprocess.Popen(['git','cat-file','--batch'],stdin=subprocess.PIPE,stdout=subprocess.PIPE)
@atexit.register
def finish_git():
 git.stdin.close();git.stdout.close();assert git.wait()==0

def begin(query):
 assert '\n' not in query
 git.stdin.write((query+'\n').encode());git.stdin.flush();head=git.stdout.readline().decode().rstrip('\n')
 if head.endswith(' missing'):return None
 oid,kind,size=head.split();assert kind=='blob';return oid,int(size)

def blob(rev,name):
 global largest_blob
 header=begin(rev+':'+name)
 if header is None:return None
 _,size=header;largest_blob=max(largest_blob,size);data=git.stdout.read(size);assert len(data)==size and git.stdout.read(1)==b'\n';return data

def verify_archive(original,revision,name,expected):
 row=storage[original];stored=r/row['stored_path'];compressed=hashlib.sha256();compressed_bytes=0
 with stored.open('rb') as f:
  header=f.read(10);assert header[:3]==b'\x1f\x8b\x08' and header[4:8]==b'\0'*4 and not header[3]&8
  f.seek(0)
  while data:=f.read(chunk):compressed.update(data);compressed_bytes+=len(data)
 assert compressed.hexdigest()==row['compressed_sha256'] and compressed_bytes==row['compressed_bytes']
 header=begin(revision+':'+name);assert header is not None;oid,size=header
 assert oid==row['original_git_blob_id'] and size==row['uncompressed_bytes']
 digest=hashlib.sha256();total=0
 with gzip.open(stored,'rb') as z:
  while data:=z.read(chunk):
   assert git.stdout.read(len(data))==data,(original,'Git/decompressed byte identity');digest.update(data);total+=len(data)
 assert git.stdout.read(1)==b'\n' and total==size
 assert digest.hexdigest()==expected==row['uncompressed_sha256']
 verified.append(original)

reverse={'loan-candidate-inventory-reuse','exit-status-directory-reuse','referenced-edge-ownership-proof','prefix-certified-expression-lookup','receiving-transitive-results-816','exit-prefix-reference-proof','status-fact-borrow-reuse','constructor-inline-failure-825','default-static-error-custody-825','prelude-forecast-286','pin-capture'}
results=[];manifests=0
revisions=sorted({row['original_path'].split('/conflict-inputs/',1)[1].split('/',1)[0] for row in index['entries'] if row['kind']=='new_conflict_input'})
for f in [p/('retained-'+revision+'.json') for revision in revisions]:
 for item in json.loads(f.read_text()):
  rev,name=item['incoming_revision'],item['path'];manifests+=1
  for side in ['base','root','incoming']:
   if side+'_sha256' not in item:continue
   original=str((p/'conflict-inputs'/rev/side/name).relative_to(r))
   verify_archive(original,item[side+'_revision'] if side!='incoming' else rev,name,item[side+'_sha256'])
  original=str((p/'conflict-inputs'/rev/'incoming'/name).relative_to(r))
  # One manifest at a time, already proved byte-identical to immutable Git.
  with gzip.open(r/storage[original]['stored_path'],'rt',encoding='utf-8') as stream:d=json.load(stream)
  if 'pending-selectors' in name:continue
  nominal=name.endswith('nominal-payload-827/source-inventory-activation-v14.json')
  if nominal:
   bindings={x['source']:x for x in d['rows']};rows=json.loads(blob(rev,'docs/evidence/nominal-payload-827/exact-source-inverses-activation-v14.json'))
  else:rows=d if isinstance(d,list) else d.get('whole_sources',d.get('owners',[d]))
  for row in rows:
   source=row.get('source',row.get('file',row.get('path')));b=blob(rev,source);assert b is not None
   binding=bindings[source] if nominal else row
   h=binding.get('sha256') if nominal else row.get('current_sha256');assert not h or h==sha(b),(rev,name,source,'current binding')
   key='inverse' if 'inverse' in row else 'hooks' if 'hooks' in row else 'edits';edits=row.get(key,[])
   if binding.get('new_file') and not edits:
    results.append({'revision':rev,'manifest':name,'source':source,'new_source_sha256':sha(b)});continue
   text=b.decode()
   for e in reversed(edits) if Path(name).parent.name in reverse else edits:
    nk='current' if 'current' in e else 'after' if 'after' in e else 'new';bk='previous' if 'previous' in e else 'before';assert text.count(e[nk])==e.get('count',1),(rev,name,source,'hook occurrence');text=text.replace(e[nk],e[bk],e.get('count',1))
   expected=binding.get('original_sha256',binding.get('before_sha256',binding.get('old_sha256',binding.get('sha256'))));assert not expected or sha(text.encode())==expected,(rev,name,source,'original hash')
   if isinstance(row.get('original_bytes'),str):assert text==row['original_bytes'],(rev,name,source,'literal original')
   ar=binding.get('archive',binding.get('original'))
   if isinstance(ar,str):
    archived=blob(rev,ar)
    if archived is None:archived=blob(rev,str(Path(name).parent/ar))
    assert archived is not None and text.encode()==archived,(rev,name,source,'complete original archive')
   results.append({'revision':rev,'manifest':name,'source':source,'hooks':len(edits),'original_sha256':sha(text.encode())})
for f in p.glob('source-conflicts-*.json'):
 for row in json.loads(f.read_text()):
  for side in ['base','root','incoming']:
   b=(p/'source-conflicts'/row['incoming_revision']/side/row['path']).read_bytes();assert sha(b)==row[side+'_sha256']
prior=(p/'retained-branches-proof.json').read_bytes();frozen=json.loads(prior)
assert manifests==frozen['full_parallel_metadata_histories'] and results==frozen['rows']
old=next(row for row in index['entries'] if row['kind']=='immediate_prior_verifier')
verify_archive(old['original_path'],old['original_git_revision'],old['original_path'],old['uncompressed_sha256'])
assert len(verified)==len(storage) and len(set(verified))==len(storage)
proof={'status':'PASS_SOURCE_ONLY_LOSSLESS_COMPRESSED_STORAGE','full_parallel_metadata_histories':manifests,'independent_incoming_original_rows':len(results),'all_original_row_identities_equal_frozen_proof':True,'prior_proof_sha256':sha(prior),'storage_index_sha256':sha((p/'compressed-storage-index.json').read_bytes()),'payloads_verified':len(verified),'all_uncompressed_side_SHA_claims_unchanged':True,'all_decompressed_payloads_byte_identical_to_original_Git_blobs':True,'deterministic_gzip_mtime':0,'deterministic_gzip_filename':'','uncompressed_bytes':sum(row['uncompressed_bytes']for row in index['entries']),'compressed_bytes':sum(row['compressed_bytes']for row in index['entries']),'chunk_bytes':chunk,'global_blob_cache':False,'largest_single_decoded_Git_blob_bytes':largest_blob,'largest_single_manifest_payload_bytes':max(row['uncompressed_bytes']for row in index['entries']),'memory_policy':'one JSON manifest plus one source/old-archive blob at a time; byte identity streams in fixed chunks; no global raw/decoded blob cache','rust_execution':'UNEXECUTED'}
(p/'compressed-retained-branches-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
