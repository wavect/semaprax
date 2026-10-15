#!/usr/bin/env python3
"""Copy the actual completed round only; never executes tests or publishes Git/GitHub."""
import argparse,hashlib,json,shutil
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--destination',type=Path,required=True);a=p.parse_args()
r=Path('/Volumes/Semaprax/lane-prompts/final-7a87-145-qualification-20261015');d=a.destination
assert not d.exists(),'new archive destination required'
receipt=json.loads((r/'owning-selected-results.json').read_text());assert receipt['complete'] and len(receipt['results'])==145
assert receipt['source_subject']=='7a87b33115c0cd45b14cac64d7b37e5742503159'
files=sorted(x for x in r.iterdir() if x.is_file())+sorted(x for x in (r/'drivers').rglob('*') if x.is_file() and '__pycache__' not in x.parts and x.suffix!='.pyc')
rows=[]
for src in files:
 b=src.read_bytes();rel=src.relative_to(r);dest=d/rel;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(b)
 digest=hashlib.sha256(b).hexdigest();assert hashlib.sha256(src.read_bytes()).hexdigest()==digest,'source changed during archive'
 rows.append({'source':str(src),'archive':str(rel),'size':len(b),'sha256':digest})
(d/'archive-manifest.json').write_text(json.dumps({'source_subject':receipt['source_subject'],'scope':'completed current round files and controllers only; no runtime-tmp/cache or unrelated prior histories','files':rows},indent=2)+'\n')
print(json.dumps({'files':len(rows),'bytes':sum(x['size'] for x in rows),'destination':str(d)}))
