from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda s:hashlib.sha256(s.encode()).hexdigest()
rows=json.loads((D/'inputs.json').read_text());assert len(rows)==3;refusals=0
for r in rows:
 for field in ['current','previous','physical']:
  raw=(D/r[field+'_archive']).read_text();assert len(raw.encode())==r[field+'_length'] and sha(raw)==r[field+'_sha256']
 assert (R/r['path']).read_text()==(D/r['physical_archive']).read_text()
 raw=(D/r['current_archive']).read_text()
 for bad in ['',raw[:-1],raw+' ',(D/r['previous_archive']).read_text()]:assert bad!=raw;refusals+=1
for r in json.loads((D/'whole-inverses.json').read_text()):
 assert (R/r['path']).read_text()==r['after'] and sha(r['after'])==r['after_sha256']
 assert (R/r['before_archive']).read_text()==r['before'] and sha(r['before'])==r['before_sha256']
 assert r['before'].encode()==subprocess.check_output(['git','show','36c6deb98720e243b0daaf81e813fbe3ec3b0a2d:'+r['path']],cwd=R)
# The three frozen historical domains and every original inverse remain exact.
for path in ['docs/evidence/main-eb2-source-composition-20261016/inputs.json','docs/evidence/main7f8-source-fronts-20261017/inputs.json','docs/evidence/main7f8-presence-composition-20261017/compact-checkpoints.json','docs/evidence/compact-child-cursor-v1/sources.json']:
 assert (R/path).read_bytes()==subprocess.check_output(['git','show','36c6deb98720e243b0daaf81e813fbe3ec3b0a2d:'+path],cwd=R)
def original_restore(text,rec):
 for e in reversed(rec['edits']):
  assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
 return text
for folder,domain in [('scalar-clone-cursor-v1','eb2-incoming'),('checked-cache-attempt-fit-v1','cache-fit')]:
 records=json.loads((R/f'docs/evidence/{folder}/sources.json').read_text())
 front=json.loads((R/('docs/evidence/main-eb2-source-composition-20261016/inputs.json'if domain=='eb2-incoming'else'docs/evidence/main7f8-source-fronts-20261017/inputs.json')).read_text())
 for rec in records:
  p=rec['path'];raw=(R/p).read_text();outer=next((r for r in rows if r['domain']==domain and r['path']==p),None)
  if outer:assert sha(raw)==outer['current_sha256'];raw=(D/outer['previous_archive']).read_text()
  f=next(r for r in front if r['domain']==('incoming'if domain=='eb2-incoming'else'cache-fit')and r['path']==p);assert sha(raw)==f['current_sha256']
  if domain=='eb2-incoming':
   restored=b'';at=0;raw=raw.encode()
   for e in f['edits']:assert raw[e['start']:e['end']]==e['after'].encode();restored+=raw[at:e['start']]+e['before'].encode();at=e['end']
   raw=(restored+raw[at:]).decode()
  else:raw=f['incoming']
  assert original_restore(raw,rec)==(R/f'docs/evidence/{folder}'/rec['archive']).read_text()
compact=json.loads((R/'docs/evidence/compact-child-cursor-v1/sources.json').read_text());r=rows[2];rec=next(x for x in compact if x['path']==r['path']);assert original_restore((D/r['previous_archive']).read_text(),rec)==(R/'docs/evidence/compact-child-cursor-v1'/rec['archive']).read_text()
result={'status':'PASS_SOURCE_ONLY','finite_stages':3,'refusals':refusals,'all_scalar_and_cache_fit_original_whole_restorations':True,'compact_metadata_original_whole_restoration':True,'old_tables_byte_exact':True,'runtime':False};(D/'source-controls.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
