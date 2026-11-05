#!/usr/bin/env python3
"""Verify exact full current-to-already-declared historical source inputs."""
from pathlib import Path
import hashlib,json,re
from collections import Counter
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
origins={
 'docs/evidence/declared-history-inputs-20261105/reviewed-stage-requests.json':'f50ea5ef2733f46fedeae73cba33a915d1e5f03c61ebdcd137d2ce312318338e',
 'docs/evidence/selected-child-refusal-781/exact-source-inverses.json':'2ff3e371c8bfde876ede4f5e7445a26067858e4a0d84610cca94d5b3ccc1240e',
 'docs/evidence/declared-history-inputs-20261105/original-fixture-requests.json':'a42b8940cafe8c79feda172e26c9aabbce396187d6496b477264ced8601e7a73',
 'docs/evidence/main09013-source-namespaces-v1/input-domains.json':'5df894bf662953e70d1014af93aae17ee401ca6f7c771ae614ca30446345516d',
 'docs/evidence/main21-exit-region-integration-20261017/input-domains.json':'44687cd637b9ea867c84f6afccea40259a24b48231da21a2e76ed5ac4e9e740b',
 'docs/evidence/referenced-edge-ownership-proof/exact-source-inverses.json':'37ad317fbf75310b1f4faa58b63ec081da84a3d361e3731fd6c3a53d8de9672a',
 'docs/evidence/exit-prefix-reference-proof/exact-source-inverses.json':'3dad4ad86f620e5293e86fa03ac7fbde28a75e588a0e5dd26416567089008f3b',
 'docs/evidence/status-fact-borrow-reuse/exact-source-inverses.json':'41f45c874531cf091ad059bc6600250a3602d5d57e3e7522bd2510a17a722bec',
}
declared=set();derived={}
for name,digest in origins.items():
 raw=(ROOT/name).read_bytes();assert sha(raw)==digest

 if name.endswith('/original-fixture-requests.json'):
  for request in json.loads(raw)['requests']:
   owner=(ROOT/request['owner_archive']).read_bytes()
   assert len(owner)==request['owner_bytes'] and sha(owner)==request['owner_sha256']
   before=(ROOT/request['archive']).read_bytes()
   assert len(before)==request['view_len'] and sha(before)==request['view_sha256']

 if name.endswith('/reviewed-stage-requests.json'):
  proof=json.loads(raw)
  for req in proof['requests']:
   origin=req['origin'];physical=(ROOT/origin['origin']).read_bytes()
   assert sha(physical)==proof['origins'][origin['origin']]
   obj=json.loads(physical)
   components=origin['trail'].split('/')[1:]
   while components:
    if isinstance(obj,list):obj=obj[int(components.pop(0))];continue
    count=next(n for n in range(len(components),0,-1) if '/'.join(components[:n]) in obj)
    obj=obj['/'.join(components[:count])];components=components[count:]
   assert obj[origin['field']]==req['view_sha256']
   archive=(ROOT/'e'/req['view_sha256']).read_bytes()
   assert len(archive)==req['view_len'] and sha(archive)==req['view_sha256']
 pending=[json.loads(raw)]
 while pending:
  row=pending.pop()
  if isinstance(row,list):pending.extend(row)
  elif isinstance(row,dict):
   source=row.get('path',row.get('source'))
   for version in ['current','view']:
    if isinstance(source,str) and version+'_sha256' in row and version+'_len' in row:
     declared.add((source,row[version+'_len'],row[version+'_sha256']))
   if isinstance(source,str) and isinstance(row.get('edits'),list) and 'archive' in row and 'sha256' in row:
    archive=(ROOT/name).parent/row['archive'];before=archive.read_bytes();assert sha(before)==row['sha256']
    after=before.decode()
    for edit in row['edits']:
     assert edit['before'] and after.count(edit['before'])==1
     after=after.replace(edit['before'],edit['after'],1)
    body=after.encode();declared.add((source,len(body),sha(body)))
    derived.setdefault((source,sha(body)),set()).add((row['sha256'],digest,name))
   if isinstance(source,str) and 'original_bytes' in row and isinstance(row.get('edits'),list):
    after=row['original_bytes']
    for edit in reversed(row['edits']):
     before,new=edit['before'],edit['new']
     if not before:assert not after;after=new
     else:assert after.count(before)==1;after=after.replace(before,new,1)
    body=after.encode();declared.add((source,len(body),sha(body)))
   pending.extend(row.values())
seen=set();expected_slots={};hashes=[];count=0
parts=sorted(HERE.glob('part-*.json'),key=lambda p:int(p.stem.split('-')[-1]))
for part,p in enumerate(parts):
 assert p.name==f'part-{part}.json' and not p.is_symlink()
 raw=p.read_bytes();assert len(raw)<=2097152;hashes.append(sha(raw))
 rows=json.loads(raw)
 for index,row in enumerate(rows):
  key=(row['source'],row['before_sha256']);assert key not in seen;seen.add(key)
  expected_slots[key]=(part,index);hashes.append(row['before_sha256'])
  assert (row['source'],row['before_bytes'],row['before_sha256']) in declared
  assert row['provenance'] in origins
  if 'fixture_originals' in row:assert {(entry['sha256'],entry['table_sha256'],entry['provenance']) for entry in row['fixture_originals']}==derived[key]
  def read(name,length,digest):
   p=ROOT/name;assert p.is_file() and not p.is_symlink()
   body=p.read_bytes();assert len(body)==length and sha(body)==digest;return body
  current=read(row['source'],row['current_bytes'],row['current_sha256'])
  assert read(row['current_archive'],row['current_bytes'],row['current_sha256'])==current
  before=read(row['before_archive'],row['before_bytes'],row['before_sha256'])
  restored=current
  for at,new,old in reversed(row['inverse']):
   new,old=new.encode(),old.encode();assert restored[at:at+len(new)]==new
   restored=restored[:at]+old+restored[at+len(new):]
  assert restored==before
  count+=1
code=(ROOT/'src/source_reflows/declared_history_inputs.rs').read_text()
assert Counter(re.findall(r'"([a-f0-9]{64})"',code))==Counter(hashes)
actual={(source,digest):(int(part),int(index)) for source,digest,part,index in re.findall(r'\(\s*"([^"]+)",\s*"([a-f0-9]{64})",?\s*\)\s*=>\s*Some\(\(\s*(\d+),\s*(\d+)\s*\)\)',code)}
assert actual==expected_slots
assert count==276 and len(parts)==11
print('PASS:',count,'closed declared tuples, complete physical/current archives, exact full inverses and independent predecessor archives')
