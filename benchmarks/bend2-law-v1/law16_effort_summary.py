#!/usr/bin/env python3
import json,pathlib,hashlib
root=pathlib.Path('benchmarks/bend2-law-v1/evidence/law16-bounded-balance-v2/v2-turns-and-replay'); rows=[]
for n in range(1,11):
 for lane,ext in [('bend2','bend'),('semaprax-scalar-v1','spx')]:
  d=root/(lane if n==1 else f'ordinal-{n}/{lane}'); src=(d/f'final-source.{ext}').read_bytes(); ev=[json.loads(x) for x in (d/'events.jsonl').read_text().splitlines()];u=[x['usage'] for x in ev if x.get('type')=='turn.completed'][0]
  rows.append({'ordinal':n,'language':lane,'source_bytes':len(src),'source_sha256':'sha256:'+hashlib.sha256(src).hexdigest(),'input_tokens':u['input_tokens'],'cached_input_tokens':u['cached_input_tokens'],'output_tokens':u['output_tokens'],'chargeable_tokens':u['input_tokens']+u['output_tokens'],'proof_synthesis':'one isolated agent turn; not kernel checking'})
print(json.dumps({'schema':'semaprax.bend2-law-benchmark.law16-effort-summary.v1','rows':rows,'phase_separation':{'kernel_check':'retained local Bend/check/Z3 process samples','cold_isolation':{'status':'unavailable','reason':'no reproducible clean tool/cache reset was retained'},'cost':{'status':'unavailable'}},'nonclaims':['source bytes are not proof quality']},indent=2,sort_keys=True))
