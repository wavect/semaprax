#!/usr/bin/env python3
"""Verify exact full current-to-already-declared historical source inputs."""
from pathlib import Path
import hashlib,json,re
from collections import Counter
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
origins={
 'docs/evidence/prelude-forecast-286/exact-source-inverses.json':'9583da7f424d9e962b60023a62443e3239e5aaa7015e5f3026fdbabc46ef5610',

 'docs/evidence/maina3-independent-custody-20261016/selected-session-inverse19.json':'7b12f6b0806a8a8daa7089fc385f122dec7c7beaade797e143ca19f8074886ff',
 'docs/evidence/main8a-independent-custody-20261016/selected-session-inverse19.json':'cbe7d0c63ec9390379e0a967ca777beef308ff4e47529fb178204f32eff9c78c',
 'docs/evidence/maina3-independent-custody-20261016/legacy-nominal-inverse18.json':'16ded8556f9460661a4056b1d8b8a3a113ca5c495b9efeed95f6a404d04a0059',

 'docs/evidence/selected-session-static-refusal-781/exact-source-inverses.json':'5080ec6ca01d0d3165427f06fe3a2d8413e171d2725fbf8c8f15c07de91d5cce',
 'docs/evidence/legacy-nominal-sharing-827/exact-source-inverses.json':'98335b571361f43b4d199270a9eb9c5918126e1a4b46a7ceefdaec5e63648e7f',
 'docs/evidence/paid-generic-constructor-graph-824/exact-source-inverses.json':'7f52d853afc872e686614e632e3ec90bfd5beaa17c4fa7d5d5f19fe772c2840d',
 'docs/evidence/lifecycle-inline-active-path/whole-source-inverses.json':'de34a7d186ed6068a4685b63960f8d7c36da5547682f26486ca6567887a49ca3',
 'docs/evidence/loan-candidate-inventory-reuse/exact-source-inverses.json':'94cc7f70e8cb2cb200526edfa0f5b4550b6c2c8b1d6b12361ddbce8dd7115d65',
 'docs/evidence/paid-nul-audit-824/exact-source-inverses.json':'a4795c4a596231aad97d70f0e3c603c4a8fa81b03980d05ffec804d36db7b14a',
 'docs/evidence/public-boundary-fixture-repair/exact-source-inverses.json':'7ee650cb29763d81de4449302b92abbba77b3a426b3c48852b4a2299d6b609ff',
 'docs/evidence/paid-cfg-child-scratch/exact-source-inverses.json':'e333d2031edb8f13c61d7df30d06eb4209944cab6c0154715b963a178e35ff50',
 'docs/evidence/ordinary-lexical-return-operands-832/exact-source-inverses.json':'cbf4b77b009ab67d5cfbb070081e3883842dca83b4457f6db046d64124dfb1f3',

 'docs/evidence/declared-history-inputs-20261105/reviewed-stage-requests.json':'f50ea5ef2733f46fedeae73cba33a915d1e5f03c61ebdcd137d2ce312318338e',
 'docs/evidence/selected-child-refusal-781/exact-source-inverses.json':'2ff3e371c8bfde876ede4f5e7445a26067858e4a0d84610cca94d5b3ccc1240e',
 'docs/evidence/declared-history-inputs-20261105/original-fixture-requests.json':'b90822c4a4855a0aff61218bcf186b1d21c83248071159c9a85a60c91624c4be',
 'docs/evidence/main09013-source-namespaces-v1/input-domains.json':'5df894bf662953e70d1014af93aae17ee401ca6f7c771ae614ca30446345516d',
 'docs/evidence/main21-exit-region-integration-20261017/input-domains.json':'44687cd637b9ea867c84f6afccea40259a24b48231da21a2e76ed5ac4e9e740b',
 'docs/evidence/referenced-edge-ownership-proof/exact-source-inverses.json':'37ad317fbf75310b1f4faa58b63ec081da84a3d361e3731fd6c3a53d8de9672a',
 'docs/evidence/exit-prefix-reference-proof/exact-source-inverses.json':'3dad4ad86f620e5293e86fa03ac7fbde28a75e588a0e5dd26416567089008f3b',
 'docs/evidence/status-fact-borrow-reuse/exact-source-inverses.json':'41f45c874531cf091ad059bc6600250a3602d5d57e3e7522bd2510a17a722bec',
}
GUARD_ORIGINS={'docs/evidence/maina3-independent-custody-20261016/legacy-nominal-inverse18.json', 'docs/evidence/loan-candidate-inventory-reuse/exact-source-inverses.json', 'docs/evidence/lifecycle-inline-active-path/whole-source-inverses.json', 'docs/evidence/paid-generic-constructor-graph-824/exact-source-inverses.json', 'docs/evidence/legacy-nominal-sharing-827/exact-source-inverses.json', 'docs/evidence/paid-nul-audit-824/exact-source-inverses.json', 'docs/evidence/paid-cfg-child-scratch/exact-source-inverses.json', 'docs/evidence/selected-session-static-refusal-781/exact-source-inverses.json', 'docs/evidence/public-boundary-fixture-repair/exact-source-inverses.json', 'docs/evidence/prelude-forecast-286/exact-source-inverses.json', 'docs/evidence/maina3-independent-custody-20261016/selected-session-inverse19.json', 'docs/evidence/ordinary-lexical-return-operands-832/exact-source-inverses.json', 'docs/evidence/main8a-independent-custody-20261016/selected-session-inverse19.json'}
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

 if name in GUARD_ORIGINS:
  for row in json.loads(raw):
   source=row.get('source',row.get('path'))
   original_sha=row.get('original_sha256',row.get('before_sha256',row.get('sha256')))
   if 'original_bytes' in row:original=row['original_bytes'].encode()
   else:
    archive=row.get('archive',row.get('original'))
    if archive is None:original=b''
    else:
     p=ROOT/archive
     if not p.is_file():p=(ROOT/name).parent/archive
     if not p.is_file():p=ROOT/{'docs/evidence/maina3-independent-custody-20261016/selected-session-inverse19.json': 'docs/evidence/selected-session-static-refusal-781', 'docs/evidence/main8a-independent-custody-20261016/selected-session-inverse19.json': 'docs/evidence/selected-session-static-refusal-781', 'docs/evidence/maina3-independent-custody-20261016/legacy-nominal-inverse18.json': 'docs/evidence/legacy-nominal-sharing-827'}[name]/archive
     original=p.read_bytes()
   assert sha(original)==original_sha
   answers=[]
   for edits in [row['edits'],list(reversed(row['edits']))]:
    after=original.decode();valid=True
    for edit in edits:
     before=edit['before'];new=edit.get('after',edit.get('new'))
     if not before:
      if after:valid=False;break
      after=new
     elif after.count(before)!=1:valid=False;break
     else:after=after.replace(before,new,1)
    if valid:answers.append(after)
   answers=list(dict.fromkeys(answers))
   if 'current_sha256' in row:answers=[a for a in answers if sha(a.encode())==row['current_sha256']]
   if len(answers)!=1:
    candidates=list(dict.fromkeys(e.get('after',e.get('new')) for e in row['edits']))
    for index,edit in enumerate(row['edits']):
     candidate=edit.get('after',edit.get('new'));valid=True
     for follow in row['edits'][index+1:]:
      before=follow['before'];new=follow.get('after',follow.get('new'))
      if not before or candidate.count(before)!=1:valid=False;break
      candidate=candidate.replace(before,new,1)
     if valid:candidates.append(candidate)
    for candidate in dict.fromkeys(candidates):
     for inverse in [row['edits'],list(reversed(row['edits']))]:
      restored=candidate;valid=True
      for edit in inverse:
       new=edit.get('after',edit.get('new'));before=edit['before']
       if not new or restored.count(new)!=1:valid=False;break
       restored=restored.replace(new,before,1)
      if valid and restored.encode()==original:answers.append(candidate)
    answers=list(dict.fromkeys(answers))
    if 'current_sha256' in row:answers=[a for a in answers if sha(a.encode())==row['current_sha256']]
   if len(answers)!=1:continue
   body=answers[0].encode();declared.add((source,len(body),sha(body)))
   derived.setdefault((source,sha(body)),set()).add((original_sha,digest,name))
 pending=[] if name in GUARD_ORIGINS else [json.loads(raw)]
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
assert count==423 and len(parts)==27
print('PASS:',count,'closed declared tuples, complete physical/current archives, exact full inverses and independent predecessor archives')
