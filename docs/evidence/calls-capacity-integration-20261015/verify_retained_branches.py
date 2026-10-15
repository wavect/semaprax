"""Independently prove retained incoming metadata histories from immutable Git."""
from pathlib import Path
import hashlib,json,subprocess
r=Path.cwd();p=r/'docs/evidence/calls-capacity-integration-20261015';sha=lambda b:hashlib.sha256(b).hexdigest();cache={}
def blob(rev,name):
 k=(rev,name)
 if k not in cache:
  x=subprocess.run(['git','show',rev+':'+name],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL);cache[k]=x.stdout if x.returncode==0 else None
 return cache[k]
reverse={'loan-candidate-inventory-reuse','exit-status-directory-reuse','referenced-edge-ownership-proof','prefix-certified-expression-lookup','receiving-transitive-results-816','exit-prefix-reference-proof','status-fact-borrow-reuse','constructor-inline-failure-825','default-static-error-custody-825','prelude-forecast-286','pin-capture'}
results=[];manifests=0
for f in sorted(p.glob('retained-*.json')):
 for item in json.loads(f.read_text()):
  rev,name=item['incoming_revision'],item['path'];manifests+=1
  for side in ['base','root','incoming']:
   if side+'_sha256' not in item:continue
   b=(p/'conflict-inputs'/rev/side/name).read_bytes();assert sha(b)==item[side+'_sha256'];assert b==blob(item[side+'_revision'] if side!='incoming' else rev,name)
  d=json.loads((p/'conflict-inputs'/rev/'incoming'/name).read_text());assert d==json.loads(blob(rev,name))
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
proof={'status':'PASS_SOURCE_ONLY','full_parallel_metadata_histories':manifests,'independent_incoming_original_rows':len(results),'rows':results,'rust_execution':'UNEXECUTED'};(p/'retained-branches-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({k:v for k,v in proof.items() if k!='rows'}))
