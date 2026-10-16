"""Named finite whole-input composition; raw guards are audited by their owners."""
from pathlib import Path
import json,hashlib
E=Path(__file__).resolve().parent
SHA='0b9948cc2277684540d55e6e5886e6e5010a30e47c826ec646e9146ebdbff2a3';LENGTH=101692
sha=lambda b:hashlib.sha256(b).hexdigest()
def compose(domain,path,raw):
 data=(E/'extensions.json').read_bytes();assert len(data)==LENGTH and sha(data)==SHA
 rows=[r for r in json.loads(data)['rows']if r['domain']==domain and r['path']==path]
 if not rows:return raw # unchanged original verifier predicates remain in force
 assert len(rows)==1;row=rows[0]
 if sha(raw)==row['target_sha256']:return raw # preserve the exact prior choice
 assert sha(raw)==row['current_sha256'],(domain,path,'unlisted whole input')
 text=raw.decode();out='';at=0
 for e in row['edits']:
  assert at<=e['start']<=e['end']<=len(text) and text[e['start']:e['end']]==e['after']
  out+=text[at:e['start']]+e['before'];at=e['end']
 target=(out+text[at:]).encode();assert sha(target)==row['target_sha256'];return target
